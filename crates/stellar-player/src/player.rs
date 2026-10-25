use crate::{
    backend::CpalBackendTask,
    ports::{PlayerPeersPort, PlayerResourcesPort, RelatedAudioResource},
};
use anyhow::Context;
use std::{path::PathBuf, sync::Arc};
use stellar_graph::entity::{AuthorId, EntityId, RelationDirection, RelationKind};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};

pub trait Backend: Send + Sync {
    fn play(&self, path: PathBuf) -> Result<(), anyhow::Error>;
}

/// Handle to the player task.
pub struct PlayerTask {
    cancellation_token: CancellationToken,
    message_tx: mpsc::UnboundedSender<PlayerMessage>,
}

/// Messages to the player task.
enum PlayerMessage {
    Play { entity: EntityId },
}

impl PlayerTask {
    pub fn spawn(
        cancellation_token: CancellationToken,
        resources_port: Arc<dyn PlayerResourcesPort>,
        peers_port: Arc<dyn PlayerPeersPort>,
    ) -> Result<Self, anyhow::Error> {
        let backend = CpalBackendTask::spawn(cancellation_token.child_token())
            .context("Failed to start playback backend")?;

        Ok(Self::spawn_with_backend(
            cancellation_token,
            Arc::new(backend),
            resources_port,
            peers_port,
        ))
    }

    fn spawn_with_backend(
        cancellation_token: CancellationToken,
        backend: Arc<dyn Backend>,
        resources_port: Arc<dyn PlayerResourcesPort>,
        peers_port: Arc<dyn PlayerPeersPort>,
    ) -> Self {
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        tokio::spawn({
            let cancellation_token = cancellation_token.clone();
            async move {
                debug!("Player task starting");

                let mut player = Player {
                    backend,
                    resources_port,
                    peers_port,
                    message_rx,
                };

                player.run(cancellation_token).await;

                debug!("Player task finished");
            }
        });

        Self {
            cancellation_token,
            message_tx,
        }
    }

    pub fn cancel(&self) {
        debug!("Cancelling player task");
        self.cancellation_token.cancel();
    }

    /// Plays the audio related to `entity`.
    pub fn play(&self, entity: EntityId) -> Result<(), anyhow::Error> {
        self.message_tx
            .send(PlayerMessage::Play { entity })
            .map_err(|_| anyhow::anyhow!("Failed to send, player task has been dropped"))?;

        Ok(())
    }
}

/// Owned state for the player task
struct Player {
    backend: Arc<dyn Backend>,
    resources_port: Arc<dyn PlayerResourcesPort>,
    peers_port: Arc<dyn PlayerPeersPort>,
    message_rx: mpsc::UnboundedReceiver<PlayerMessage>,
}

impl Player {
    async fn run(&mut self, cancellation_token: CancellationToken) {
        loop {
            tokio::select! {
                Some(message) = self.message_rx.recv() => {
                    tokio::select! {
                        result = self.handle_message(message) => {
                            if let Err(error) = result {
                                error!(?error, "Failed to handle message");
                            }
                        }
                        _ = cancellation_token.cancelled() => {
                            debug!("Cancelled");
                            break;
                        }
                    }
                }

                _ = cancellation_token.cancelled() => {
                    debug!("Cancelled");
                    break;
                }
            }
        }
    }

    async fn handle_message(&self, message: PlayerMessage) -> Result<(), anyhow::Error> {
        match message {
            PlayerMessage::Play { entity } => self.play(entity).await,
        }
    }

    async fn play(&self, entity: EntityId) -> Result<(), anyhow::Error> {
        let related_audio_resources = self.resources_port.list_related_audio_resources(entity)?;

        let source = choose_audio_resource(
            self.resources_port.local_author_id(),
            &related_audio_resources,
        )
        .ok_or_else(|| anyhow::anyhow!("No audio available for {entity:?}"))?;

        let path = match source {
            ChooseAudioResourceOutput::Local { path } => path,
            ChooseAudioResourceOutput::Remote {
                device,
                audio_resource,
                relation,
                direction,
            } => self
                .peers_port
                .download_audio_resource(device, audio_resource, entity, relation, direction)
                .await
                .with_context(|| format!("Failed to download audio for {entity:?}"))?,
        };

        self.backend.play(path)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChooseAudioResourceOutput {
    Local {
        path: PathBuf,
    },
    Remote {
        device: AuthorId,
        audio_resource: EntityId,
        relation: RelationKind,
        direction: RelationDirection,
    },
}

/// Chooses where to play an entity from, given the audio resources related to it:
///
/// 1. Returns the first local file if one exists
/// 2. Returns the first remote file if one exists
///
/// Returns `None` if there are no audio resources.
pub fn choose_audio_resource(
    local_author_id: AuthorId,
    related_audio_resources: &[RelatedAudioResource],
) -> Option<ChooseAudioResourceOutput> {
    if let Some(local_audio_resource) = related_audio_resources
        .iter()
        .find(|related| related.audio_resource.location.device == local_author_id)
    {
        return Some(ChooseAudioResourceOutput::Local {
            path: local_audio_resource.audio_resource.location.path.clone(),
        });
    }

    related_audio_resources
        .first()
        .map(|remote_audio_resource| ChooseAudioResourceOutput::Remote {
            device: remote_audio_resource.audio_resource.location.device,
            audio_resource: remote_audio_resource.entity,
            relation: remote_audio_resource.relation,
            direction: remote_audio_resource.relation_direction,
        })
}

#[cfg(test)]
mod test {
    use super::{Backend, PlayerTask};
    use crate::ports::{PlayerPeersPort, PlayerResourcesPort, RelatedAudioResource};
    use async_trait::async_trait;
    use std::{collections::HashMap, path::PathBuf, sync::Arc, sync::Mutex, time::Duration};
    use stellar_graph::entity::{
        AuthorId, EntityId, EntityKind, RelationDirection, RelationKind, Timestamp, Version,
    };
    use stellar_resources::audio::{AUDIO_RESOURCE_ENTITY, AudioResource, AudioResourceLocation};
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    fn local_device() -> AuthorId {
        AuthorId::from_bytes([1u8; 32])
    }
    fn remote_device() -> AuthorId {
        AuthorId::from_bytes([2u8; 32])
    }

    struct FakeBackend {
        played_tx: mpsc::UnboundedSender<PathBuf>,
    }

    impl Backend for FakeBackend {
        fn play(&self, path: PathBuf) -> Result<(), anyhow::Error> {
            self.played_tx.send(path)?;
            Ok(())
        }
    }

    struct FakeResources {
        related: HashMap<EntityId, Vec<RelatedAudioResource>>,
    }

    impl PlayerResourcesPort for FakeResources {
        fn local_author_id(&self) -> AuthorId {
            local_device()
        }

        fn list_related_audio_resources(
            &self,
            entity: EntityId,
        ) -> Result<Vec<RelatedAudioResource>, anyhow::Error> {
            Ok(self.related.get(&entity).cloned().unwrap_or_default())
        }
    }

    #[derive(Debug, PartialEq)]
    struct Download {
        device: AuthorId,
        audio_resource: EntityId,
        parent: EntityId,
        relation: RelationKind,
        relation_direction: RelationDirection,
    }

    struct FakePeers {
        downloaded_path: PathBuf,
        downloads: Mutex<Vec<Download>>,
    }

    #[async_trait]
    impl PlayerPeersPort for FakePeers {
        async fn download_audio_resource(
            &self,
            device: AuthorId,
            audio_resource: EntityId,
            parent: EntityId,
            relation: RelationKind,
            relation_direction: RelationDirection,
        ) -> Result<PathBuf, anyhow::Error> {
            self.downloads.lock().unwrap().push(Download {
                device,
                audio_resource,
                parent,
                relation,
                relation_direction,
            });
            Ok(self.downloaded_path.clone())
        }
    }

    fn make_related_audio_resource(device: AuthorId, path: &str) -> RelatedAudioResource {
        let version = Version::new(Timestamp::new(0), device);

        RelatedAudioResource {
            entity: EntityId::random(AUDIO_RESOURCE_ENTITY),
            audio_resource: AudioResource {
                location: AudioResourceLocation {
                    device,
                    path: PathBuf::from(path),
                },
                size: 0,
                duration: 0.0,
                hash: None,
                codec: None,
                bitrate: None,
                sample_rate: None,
                bit_depth: None,
                channels: None,

                location_version: version,
                size_version: version,
                duration_version: version,
                hash_version: None,
                codec_version: None,
                bitrate_version: None,
                sample_rate_version: None,
                bit_depth_version: None,
                channels_version: None,
            },
            relation: RelationKind::random(),
            relation_direction: RelationDirection::Outgoing,
        }
    }

    struct Test {
        player: PlayerTask,
        peers: Arc<FakePeers>,
        played_rx: mpsc::UnboundedReceiver<PathBuf>,
    }

    fn spawn(related: HashMap<EntityId, Vec<RelatedAudioResource>>) -> Test {
        let (played_tx, played_rx) = mpsc::unbounded_channel();
        let peers = Arc::new(FakePeers {
            downloaded_path: PathBuf::from("/cache/downloaded.mp3"),
            downloads: Mutex::new(Vec::new()),
        });

        let player = PlayerTask::spawn_with_backend(
            CancellationToken::new(),
            Arc::new(FakeBackend { played_tx }),
            Arc::new(FakeResources { related }),
            peers.clone(),
        );

        Test {
            player,
            peers,
            played_rx,
        }
    }

    async fn wait_for_played(played_rx: &mut mpsc::UnboundedReceiver<PathBuf>) -> PathBuf {
        tokio::time::timeout(Duration::from_secs(10), played_rx.recv())
            .await
            .expect("Timed out waiting for playback")
            .expect("Backend dropped")
    }

    #[tokio::test]
    async fn plays_local_resource_without_downloading() {
        let entity = EntityId::random(EntityKind::random());
        let local = make_related_audio_resource(local_device(), "/music/local.mp3");
        let remote = make_related_audio_resource(remote_device(), "/music/remote.mp3");
        let mut test = spawn(HashMap::from([(entity, vec![remote, local])]));

        test.player.play(entity).unwrap();

        assert_eq!(
            wait_for_played(&mut test.played_rx).await,
            PathBuf::from("/music/local.mp3")
        );
        assert!(test.peers.downloads.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn downloads_remote_resource_then_plays() {
        let entity = EntityId::random(EntityKind::random());
        let remote = make_related_audio_resource(remote_device(), "/music/remote.mp3");
        let expected = Download {
            device: remote_device(),
            audio_resource: remote.entity,
            parent: entity,
            relation: remote.relation,
            relation_direction: remote.relation_direction,
        };
        let mut test = spawn(HashMap::from([(entity, vec![remote])]));

        test.player.play(entity).unwrap();

        assert_eq!(
            wait_for_played(&mut test.played_rx).await,
            PathBuf::from("/cache/downloaded.mp3")
        );
        assert_eq!(*test.peers.downloads.lock().unwrap(), vec![expected]);
    }

    #[tokio::test]
    async fn plays_after_missing() {
        let missing = EntityId::random(EntityKind::random());
        let entity = EntityId::random(EntityKind::random());
        let local = make_related_audio_resource(local_device(), "/music/local.mp3");
        let mut test = spawn(HashMap::from([(entity, vec![local])]));

        test.player.play(missing).unwrap();
        test.player.play(entity).unwrap();

        assert_eq!(
            wait_for_played(&mut test.played_rx).await,
            PathBuf::from("/music/local.mp3")
        );
    }
}
