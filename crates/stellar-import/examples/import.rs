use std::{collections::HashMap, sync::Arc, time::Duration};
use stellar_graph::{
    entity::{AttributeKind, EntityKind, RelationKind, ValueKind},
    schema::{AttributeSchema, EntitySchema, RelationSchema, Schema},
};
use stellar_import::{
    import::{ImportEventHandler, ImportEventScannedFile, ImportTask},
    rules::{Rule, Rules, TagKind, TagRule},
};
use stellar_resources::audio::{AUDIO_RESOURCE_ENTITY, audio_resource_schema};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let dir = std::env::args().nth(1).expect("expected directory to scan");

    tracing::info!("Scanning {}", dir);

    let song = EntityKind::random();
    let song_title = AttributeKind::random();
    let song_schema = EntitySchema {
        name: "Song".to_string(),
        attributes: HashMap::from([(
            song_title,
            AttributeSchema {
                name: "Title".to_string(),
                value: ValueKind::Text,
            },
        )]),
    };

    let song_audio_resource = RelationKind::random();
    let song_audio_resource_schema = RelationSchema {
        name: "Song Audio Resource".to_string(),
        source: song,
        target: AUDIO_RESOURCE_ENTITY,
        attributes: HashMap::new(),
    };

    let schema = Schema {
        entities: HashMap::from([
            (song, song_schema),
            (AUDIO_RESOURCE_ENTITY, audio_resource_schema()),
        ]),
        relations: HashMap::from([(song_audio_resource, song_audio_resource_schema)]),
    };

    let rules = Rules {
        rules: vec![Rule::TagRule(TagRule {
            attribute: song_title,
            value: ValueKind::Text,
            tag: TagKind::TrackTitle,
        })],
    };

    dbg!(&schema);
    dbg!(&rules);

    let event_handler = Arc::new(ExampleImportEventHandler::default());

    let cancellation_token = CancellationToken::new();
    let task = ImportTask::spawn(
        cancellation_token,
        event_handler.clone(),
        vec![dir.into()],
        schema,
        song,
    )?;

    event_handler.scan_finished.notified().await;
    tracing::info!("Scan finished");

    task.import(rules);

    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down");

    task.cancel();

    tokio::time::sleep(Duration::from_millis(100)).await;

    Ok(())
}

#[derive(Default)]
struct ExampleImportEventHandler {
    scan_finished: Notify,
}

impl ImportEventHandler for ExampleImportEventHandler {
    fn on_pending_file(&self, path: String) {
        println!("pending: {path}");
    }

    fn on_scanned_file(&self, file: ImportEventScannedFile) {
        println!("scanned {} -> {:?}", file.path, file.tags)
    }

    fn on_scan_finished(&self) {
        self.scan_finished.notify_one();
    }
}
