use crate::{
    connection::{Message, MessageSink, MessageStream, PeerConnection, open_message_stream},
    peers::PeersDatabasePort,
    protocol::StreamHeader,
};
use anyhow::Context;
use futures::{SinkExt as _, StreamExt as _};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};
use stellar_graph::{
    database::Database,
    entity::{AuthorId, EntityId, RelationKind, Timestamp, Version},
    store::{EntityData, EntityMetadataValue},
};
use stellar_resources::audio::{AUDIO_RESOURCE_ENTITY, AudioResource, AudioResourceLocation};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
    sync::oneshot,
};

const CHUNK_SIZE: usize = 64 * 1024;

#[derive(Debug, Serialize, Deserialize)]
pub enum ResourceClientMessage {
    Audio { audio_resource: EntityId },
}

impl Message for ResourceClientMessage {}

#[derive(Debug, Serialize, Deserialize)]
pub enum ResourceServerMessage {
    Data { data: Vec<u8> },
    Done,
    Error(String),
}

impl Message for ResourceServerMessage {}

/// Handle for a peer resource client task
pub struct PeerResourceClientTask {}

impl PeerResourceClientTask {
    pub fn spawn(
        database: Arc<dyn PeersDatabasePort>,
        resources: Arc<dyn PeersResourcesPort>,
        connection: Arc<dyn PeerConnection>,
        audio_resource: EntityId,
        parent: EntityId,
        relation: RelationKind,
        result_tx: oneshot::Sender<Result<PathBuf, anyhow::Error>>,
    ) -> Self {
        tokio::spawn({
            async move {
                let result = Self::run(
                    database,
                    resources,
                    connection,
                    audio_resource,
                    parent,
                    relation,
                )
                .await;

                if let Err(error) = &result {
                    tracing::error!("Peer resource client task errored: {error}");
                } else {
                    tracing::debug!("Peer resource client task finished");
                }

                let _ = result_tx.send(result);
            }
        });

        Self {}
    }

    async fn run(
        database: Arc<dyn PeersDatabasePort>,
        resources: Arc<dyn PeersResourcesPort>,
        connection: Arc<dyn PeerConnection>,
        audio_resource: EntityId,
        parent: EntityId,
        relation: RelationKind,
    ) -> Result<PathBuf, anyhow::Error> {
        tracing::debug!("PeerResourceClient starting");

        let source_data = database
            .get_entities_by_id(HashSet::from([audio_resource]))?
            .remove(&audio_resource)
            .ok_or_else(|| {
                anyhow::anyhow!("Audio resource entity {audio_resource} not found locally")
            })?;
        let source = AudioResource::try_from_attributes(&source_data.attributes)?;

        let (mut tx, mut rx) = open_message_stream::<ResourceClientMessage, ResourceServerMessage>(
            connection.as_ref(),
            StreamHeader::Resource,
        )
        .await?;

        tx.send(ResourceClientMessage::Audio { audio_resource })
            .await
            .context("Failed to send ResourceClientMessage::Audio")?;

        let audio_dir = resources.cache_dir().join("audio");
        tokio::fs::create_dir_all(&audio_dir)
            .await
            .context("Failed to create audio cache directory")?;

        let extension = codec_to_extension(&source.codec);
        let tmp_path = audio_dir.join(format!("{audio_resource}.{extension}.tmp"));
        let final_path = audio_dir.join(format!("{audio_resource}.{extension}"));

        let mut tmp_file = File::create(&tmp_path)
            .await
            .context("Failed to create temporary audio cache file")?;

        loop {
            let message = rx
                .next()
                .await
                .ok_or_else(|| anyhow::anyhow!("ResourceServerMessage stream closed"))??;

            match message {
                ResourceServerMessage::Data { data } => {
                    tmp_file
                        .write_all(&data)
                        .await
                        .context("Failed to write to temporary audio cache file")?;
                }
                ResourceServerMessage::Done => break,
                ResourceServerMessage::Error(message) => {
                    drop(tmp_file);
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    anyhow::bail!("Peer failed to send audio resource: {message}");
                }
            }
        }

        tmp_file
            .flush()
            .await
            .context("Failed to flush temporary audio cache file")?;
        drop(tmp_file);

        tokio::fs::rename(&tmp_path, &final_path)
            .await
            .context("Failed to move temporary audio cache file into place")?;

        resources.create_local_audio_resource(&source, final_path.clone(), parent, relation)?;

        tracing::debug!("PeerResourceClient finished");

        Ok(final_path)
    }
}

/// Handle for a peer resource server task
pub struct PeerResourceServerTask {}

impl PeerResourceServerTask {
    pub fn spawn(
        resources: Arc<dyn PeersResourcesPort>,
        tx: MessageSink<ResourceServerMessage>,
        rx: MessageStream<ResourceClientMessage>,
    ) -> Self {
        tokio::spawn({
            async move {
                let result = Self::run(resources, tx, rx).await;

                if let Err(error) = result {
                    tracing::error!("Peer resource server task errored: {error}");
                } else {
                    tracing::debug!("Peer resource server task finished");
                }
            }
        });

        Self {}
    }

    async fn run(
        resources: Arc<dyn PeersResourcesPort>,
        mut tx: MessageSink<ResourceServerMessage>,
        mut rx: MessageStream<ResourceClientMessage>,
    ) -> Result<(), anyhow::Error> {
        tracing::debug!("PeerResourceServer starting");

        let ResourceClientMessage::Audio { audio_resource } = rx
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("ResourceClientMessage stream closed"))??;

        let path = match resources.resolve_audio_resource_path(audio_resource) {
            Ok(Some(path)) => path,
            Ok(None) => {
                tx.send(ResourceServerMessage::Error(format!(
                    "No local audio resource file for entity {audio_resource}"
                )))
                .await?;
                return Ok(());
            }
            Err(error) => {
                tx.send(ResourceServerMessage::Error(format!("{error:#}")))
                    .await?;
                return Ok(());
            }
        };

        let mut file = match File::open(&path).await {
            Ok(file) => file,
            Err(error) => {
                tx.send(ResourceServerMessage::Error(format!(
                    "Failed to open audio resource file: {error}"
                )))
                .await?;
                return Ok(());
            }
        };

        let mut buffer = vec![0u8; CHUNK_SIZE];
        loop {
            let n = file
                .read(&mut buffer)
                .await
                .context("Failed to read audio resource file")?;
            if n == 0 {
                break;
            }

            tx.send(ResourceServerMessage::Data {
                data: buffer[..n].to_vec(),
            })
            .await?;
        }

        tx.send(ResourceServerMessage::Done).await?;

        tracing::debug!("PeerResourceServer finished");

        Ok(())
    }
}

fn codec_to_extension(codec: &str) -> String {
    // TODO
    return "mp3".to_string();
}

pub trait PeersResourcesPort: Send + Sync {
    /// The directory downloaded resources are cached under.
    fn cache_dir(&self) -> &Path;

    /// Resolves `entity` to a local file path. Used by the server task.
    fn resolve_audio_resource_path(
        &self,
        entity: EntityId,
    ) -> Result<Option<PathBuf>, anyhow::Error>;

    /// Creates an audio resource entity for a downloaded resource from the `source` resource,
    /// pointing to the same `parent` entity via `relation`.
    fn create_local_audio_resource(
        &self,
        source: &AudioResource,
        local_path: PathBuf,
        parent: EntityId,
        relation: RelationKind,
    ) -> Result<EntityId, anyhow::Error>;
}

pub struct PeersResourcesAdapter {
    database: Database,
    author_id: AuthorId,
    cache_dir: PathBuf,
}

impl PeersResourcesAdapter {
    pub fn new(database: Database, author_id: AuthorId, cache_dir: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            database,
            author_id,
            cache_dir,
        })
    }
}

impl PeersResourcesPort for PeersResourcesAdapter {
    fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    fn resolve_audio_resource_path(
        &self,
        entity: EntityId,
    ) -> Result<Option<PathBuf>, anyhow::Error> {
        // TODO: optimize this
        let Some(data) = self.database.get_entities()?.remove(&entity) else {
            return Ok(None);
        };

        let resource = AudioResource::try_from_attributes(&data.attributes)?;
        if resource.location.device != self.author_id {
            return Ok(None);
        }

        Ok(Some(resource.location.path))
    }

    fn create_local_audio_resource(
        &self,
        source: &AudioResource,
        local_path: PathBuf,
        parent: EntityId,
        relation: RelationKind,
    ) -> Result<EntityId, anyhow::Error> {
        let version = Version::new(Timestamp::now(), self.author_id);

        let resource = AudioResource {
            location: AudioResourceLocation {
                device: self.author_id,
                path: local_path,
            },
            location_version: version,
            hash: source.hash,
            hash_version: version,
            size: source.size,
            size_version: version,
            duration: source.duration,
            duration_version: version,
            codec: source.codec.clone(),
            codec_version: version,
            bitrate: source.bitrate,
            bitrate_version: version,
            sample_rate: source.sample_rate,
            sample_rate_version: version,
            bit_depth: source.bit_depth,
            bit_depth_version: version,
            channels: source.channels,
            channels_version: version,
        };

        let entity = EntityId::random(AUDIO_RESOURCE_ENTITY);
        self.database.apply_local_entity(
            entity,
            EntityData {
                metadata: EntityMetadataValue {
                    deleted: false,
                    deleted_version: version,
                },
                attributes: resource.to_attributes(),
            },
        )?;

        self.database
            .create_relation(relation, parent, entity, version)?;

        Ok(entity)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{connection::MemoryPeerConnection, peers::PeersDatabaseAdapter};
    use std::{collections::HashMap, sync::Mutex};
    use stellar_graph::database::Database;

    #[tokio::test]
    async fn download_resource() {
        let author = AuthorId::from_slice(&[1u8; 32]);
        let dir = testdir::testdir!();

        let source_path = dir.join("source.mp3");
        let source_bytes: Vec<u8> = (0..(CHUNK_SIZE * 3)).map(|i| (i % 251) as u8).collect();
        std::fs::write(&source_path, &source_bytes).unwrap();

        let database = Database::in_memory();
        let (audio_resource, source) = insert_audio_resource(
            &database,
            author,
            source_path.clone(),
            source_bytes.len() as u64,
        );
        let database = PeersDatabaseAdapter::new(database);

        let client_resources = FakeResources::new(dir.join("client-cache"), HashMap::new());
        let server_resources = FakeResources::new(
            dir.join("server-cache"),
            HashMap::from([(audio_resource, source_path)]),
        );

        let (client_connection, server_connection) = MemoryPeerConnection::new();
        tokio::spawn(accept_and_spawn_resource_server(
            server_connection,
            server_resources,
        ));

        let parent = EntityId::random(AUDIO_RESOURCE_ENTITY);
        let relation = RelationKind::random();
        let (result_tx, result_rx) = oneshot::channel();
        PeerResourceClientTask::spawn(
            database,
            client_resources.clone(),
            client_connection,
            audio_resource,
            parent,
            relation,
            result_tx,
        );

        let downloaded_path = result_rx.await.unwrap().unwrap();
        assert_eq!(
            tokio::fs::read(&downloaded_path).await.unwrap(),
            source_bytes
        );

        let created = client_resources.created.lock().unwrap();
        let (created_source, created_path, created_parent, created_relation) = created
            .first()
            .expect("create_local_audio_resource was not called");
        assert_eq!(*created_source, source);
        assert_eq!(*created_path, downloaded_path);
        assert_eq!(*created_parent, parent);
        assert_eq!(*created_relation, relation);
    }

    struct FakeResources {
        cache_dir: PathBuf,
        files: HashMap<EntityId, PathBuf>,
        created: Mutex<Vec<(AudioResource, PathBuf, EntityId, RelationKind)>>,
    }

    impl FakeResources {
        fn new(cache_dir: PathBuf, files: HashMap<EntityId, PathBuf>) -> Arc<Self> {
            Arc::new(Self {
                cache_dir,
                files,
                created: Mutex::new(Vec::new()),
            })
        }
    }

    impl PeersResourcesPort for FakeResources {
        fn cache_dir(&self) -> &Path {
            &self.cache_dir
        }

        fn resolve_audio_resource_path(
            &self,
            entity: EntityId,
        ) -> Result<Option<PathBuf>, anyhow::Error> {
            Ok(self.files.get(&entity).cloned())
        }

        fn create_local_audio_resource(
            &self,
            source: &AudioResource,
            local_path: PathBuf,
            parent: EntityId,
            relation: RelationKind,
        ) -> Result<EntityId, anyhow::Error> {
            self.created
                .lock()
                .unwrap()
                .push((source.clone(), local_path, parent, relation));
            Ok(EntityId::random(AUDIO_RESOURCE_ENTITY))
        }
    }

    fn insert_audio_resource(
        database: &Database,
        author: AuthorId,
        path: PathBuf,
        size: u64,
    ) -> (EntityId, AudioResource) {
        let version = Version::new(Timestamp::now(), author);
        let source = AudioResource {
            location: AudioResourceLocation {
                device: author,
                path,
            },
            location_version: version,
            hash: [0u8; 32],
            hash_version: version,
            size,
            size_version: version,
            duration: 1.0,
            duration_version: version,
            codec: "mp3".to_string(),
            codec_version: version,
            bitrate: 128_000,
            bitrate_version: version,
            sample_rate: 44_100,
            sample_rate_version: version,
            bit_depth: 16,
            bit_depth_version: version,
            channels: 2,
            channels_version: version,
        };

        let entity = EntityId::random(AUDIO_RESOURCE_ENTITY);
        database
            .apply_local_entity(
                entity,
                EntityData {
                    metadata: EntityMetadataValue {
                        deleted: false,
                        deleted_version: version,
                    },
                    attributes: source.to_attributes(),
                },
            )
            .unwrap();

        (entity, source)
    }

    async fn accept_and_spawn_resource_server(
        connection: Arc<dyn PeerConnection>,
        resources: Arc<dyn PeersResourcesPort>,
    ) {
        let (tx, mut rx) = connection.accept_bi().await.unwrap();

        let header = StreamHeader::decode(&rx.next().await.unwrap().unwrap()).unwrap();
        assert!(matches!(header, StreamHeader::Resource));

        PeerResourceServerTask::spawn(resources, MessageSink::new(tx), MessageStream::new(rx));
    }
}
