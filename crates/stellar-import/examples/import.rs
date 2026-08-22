use std::{collections::HashMap, sync::Arc, time::Duration};
use stellar_graph::{
    entity::{AttributeKind, EntityKind, RelationKind, ValueKind},
    schema::{AttributeSchema, EntitySchema, RelationSchema, Schema},
};
use stellar_import::{
    import::{ImportDatabasePort, ImportEventHandler, ImportEventScannedFile, ImportTask},
    rules::{AttributeRule, RelationRule, RelationRuleDirection, Rule, Rules, TagKind},
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

    let album = EntityKind::random();
    let album_title = AttributeKind::random();
    let album_schema = EntitySchema {
        name: "Album".to_string(),
        attributes: HashMap::from([(
            album_title,
            AttributeSchema {
                name: "Title".to_string(),
                value: ValueKind::Text,
            },
        )]),
    };

    let artist = EntityKind::random();
    let artist_name = AttributeKind::random();
    let artist_schema = EntitySchema {
        name: "Artist".to_string(),
        attributes: HashMap::from([(
            artist_name,
            AttributeSchema {
                name: "Name".to_string(),
                value: ValueKind::Text,
            },
        )]),
    };

    let album_song = RelationKind::random();
    let album_track_number = AttributeKind::random();
    let album_song_schema = RelationSchema {
        name: "Track".to_string(),
        source: album,
        target: song,
        attributes: HashMap::from([(
            album_track_number,
            AttributeSchema {
                name: "Track Number".to_string(),
                value: ValueKind::Number,
            },
        )]),
    };

    let album_artist = RelationKind::random();
    let album_artist_schema = RelationSchema {
        name: "Album Artist".to_string(),
        source: album,
        target: artist,
        attributes: HashMap::new(),
    };

    let song_artist = RelationKind::random();
    let song_artist_schema = RelationSchema {
        name: "Song Artist".to_string(),
        source: song,
        target: artist,
        attributes: HashMap::new(),
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
            (album, album_schema),
            (artist, artist_schema),
            (AUDIO_RESOURCE_ENTITY, audio_resource_schema()),
        ]),
        relations: HashMap::from([
            (album_song, album_song_schema),
            (album_artist, album_artist_schema),
            (song_artist, song_artist_schema),
            (song_audio_resource, song_audio_resource_schema),
        ]),
    };

    let rules = Rules {
        rule: Rule {
            attributes: vec![AttributeRule {
                attribute: song_title,
                value: ValueKind::Text,
                tag: TagKind::TrackTitle,
            }],
            relations: vec![
                RelationRule {
                    relation: album_song,
                    other: album,
                    direction: RelationRuleDirection::Incoming,
                    relation_key_attributes: vec![AttributeRule {
                        attribute: album_track_number,
                        value: ValueKind::Number,
                        tag: TagKind::TrackNumber,
                    }],
                    relation_extra_attributes: vec![],
                    other_key_attributes: vec![AttributeRule {
                        attribute: album_title,
                        value: ValueKind::Text,
                        tag: TagKind::AlbumTitle,
                    }],
                    other_extra_attributes: vec![],
                    nested_relations: vec![RelationRule {
                        relation: album_artist,
                        other: artist,
                        direction: RelationRuleDirection::Outgoing,
                        relation_key_attributes: vec![],
                        relation_extra_attributes: vec![],
                        other_key_attributes: vec![AttributeRule {
                            attribute: artist_name,
                            value: ValueKind::Text,
                            tag: TagKind::AlbumArtist,
                        }],
                        other_extra_attributes: vec![],
                        nested_relations: vec![],
                    }],
                },
                RelationRule {
                    relation: song_artist,
                    other: artist,
                    direction: RelationRuleDirection::Outgoing,
                    relation_key_attributes: vec![],
                    relation_extra_attributes: vec![],
                    other_key_attributes: vec![AttributeRule {
                        attribute: artist_name,
                        value: ValueKind::Text,
                        tag: TagKind::TrackArtist,
                    }],
                    other_extra_attributes: vec![],
                    nested_relations: vec![],
                },
            ],
        },
    };

    dbg!(&schema);
    dbg!(&rules);

    let event_handler = Arc::new(ExampleImportEventHandler::default());

    let cancellation_token = CancellationToken::new();
    let task = ImportTask::spawn(
        cancellation_token,
        Arc::new(ExampleDatabasePort),
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

struct ExampleDatabasePort;

impl ImportDatabasePort for ExampleDatabasePort {
    fn find_entity(
        &self,
        kind: AttributeKind,
        attributes: HashMap<AttributeKind, stellar_graph::entity::Value>,
    ) -> Option<stellar_graph::entity::EntityId> {
        None
    }

    fn find_relation(
        &self,
        kind: RelationKind,
        source: stellar_graph::entity::EntityId,
        target: stellar_graph::entity::EntityId,
        attributes: HashMap<AttributeKind, stellar_graph::entity::Value>,
    ) -> Option<stellar_graph::entity::RelationId> {
        None
    }
}
