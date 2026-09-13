use std::{path::PathBuf, sync::Arc, time::Duration};
use stellar_graph::entity::{AuthorId, EntityId, Timestamp, Version};
use stellar_player::backend::{CpalBackendTask, PlayerResourcesPort};
use stellar_resources::audio::{AUDIO_RESOURCE_ENTITY, AudioResource, AudioResourceLocation};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")),
        )
        .init();

    let path = PathBuf::from(std::env::args().nth(1).expect("Usage: player <path>"));
    let resources_port = Arc::new(ExamplePlayerResourcesAdapter::new(path));

    let backend = CpalBackendTask::spawn(CancellationToken::new(), resources_port).unwrap();

    tracing::info!("Initialized backend");

    backend
        .play(EntityId::random(AUDIO_RESOURCE_ENTITY))
        .unwrap();

    tokio::time::sleep(Duration::from_secs(30)).await;
}

struct ExamplePlayerResourcesAdapter {
    author_id: AuthorId,
    resource: AudioResource,
}

impl ExamplePlayerResourcesAdapter {
    fn new(path: PathBuf) -> Self {
        let author_id = AuthorId::from_bytes([0u8; 32]);
        let version = Version::new(Timestamp::now(), author_id);

        let resource = AudioResource {
            location: AudioResourceLocation {
                device: author_id,
                path,
            },
            hash: [0u8; 32],
            size: 0,
            duration: 0.0,
            codec: String::new(),
            bitrate: 0,
            sample_rate: 0,
            bit_depth: 0,
            channels: 0,

            location_version: version,
            hash_version: version,
            size_version: version,
            duration_version: version,
            codec_version: version,
            bitrate_version: version,
            sample_rate_version: version,
            bit_depth_version: version,
            channels_version: version,
        };

        Self {
            author_id,
            resource,
        }
    }
}

impl PlayerResourcesPort for ExamplePlayerResourcesAdapter {
    fn local_author_id(&self) -> AuthorId {
        self.author_id
    }

    fn list_related_audio_resources(
        &self,
        _entity: EntityId,
    ) -> Result<Vec<AudioResource>, anyhow::Error> {
        Ok(vec![self.resource.clone()])
    }
}
