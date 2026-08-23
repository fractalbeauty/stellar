use std::{collections::HashMap, sync::Arc, time::Duration};
use stellar_graph::{
    entity::{AttributeKind, AuthorId, EntityKind, RelationKind, ValueKind},
    schema::{AttributeSchema, EntitySchema, GraphSchema, RelationSchema},
};
use stellar_import::{
    import::{ImportEventHandler, ImportEventScannedFile, ImportTask},
    ports::{ImportDatabasePort, ImportSchemaPort},
    rules::{AttributeRule, RelationRule, RelationRuleDirection, Rule, Rules, TagKind},
};
use stellar_resources::audio::{AUDIO_RESOURCE_ENTITY, audio_resource_schema};
use tokio::sync::{Notify, watch};
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

    let event_handler = Arc::new(ExampleImportEventHandler::default());

    let cancellation_token = CancellationToken::new();
    let task = ImportTask::spawn(
        cancellation_token,
        Arc::new(ExampleImportDatabaseAdapter),
        Arc::new(ExampleImportSchemaAdapter::new()),
        event_handler.clone(),
        vec![dir.into()],
        AuthorId::from_bytes([0u8; 32]),
    )?;

    event_handler.scan_finished.notified().await;
    tracing::info!("Scan finished");

    task.import();

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

struct ExampleImportDatabaseAdapter;

impl ImportDatabasePort for ExampleImportDatabaseAdapter {
    fn get_entities_by_kind(
        &self,
        _kind: EntityKind,
    ) -> Result<
        HashMap<stellar_graph::entity::EntityId, stellar_graph::store::EntityData>,
        anyhow::Error,
    > {
        Ok(HashMap::new())
    }

    fn apply_changes(
        &self,
        _changes: stellar_import::evaluator::Changes,
        _author: AuthorId,
    ) -> Result<(), anyhow::Error> {
        todo!()
    }
}

struct ExampleImportSchemaAdapter {
    watch_rx: watch::Receiver<Option<(GraphSchema, Rules)>>,
}

impl ExampleImportSchemaAdapter {
    fn new() -> Self {
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

        let graph = GraphSchema {
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
                        relation_attributes: vec![AttributeRule {
                            attribute: album_track_number,
                            value: ValueKind::Number,
                            tag: TagKind::TrackNumber,
                        }],
                        other_attributes: vec![AttributeRule {
                            attribute: album_title,
                            value: ValueKind::Text,
                            tag: TagKind::AlbumTitle,
                        }],
                        nested_relations: vec![RelationRule {
                            relation: album_artist,
                            other: artist,
                            direction: RelationRuleDirection::Outgoing,
                            relation_attributes: vec![],
                            other_attributes: vec![AttributeRule {
                                attribute: artist_name,
                                value: ValueKind::Text,
                                tag: TagKind::AlbumArtist,
                            }],
                            nested_relations: vec![],
                        }],
                    },
                    RelationRule {
                        relation: song_artist,
                        other: artist,
                        direction: RelationRuleDirection::Outgoing,
                        relation_attributes: vec![],
                        other_attributes: vec![AttributeRule {
                            attribute: artist_name,
                            value: ValueKind::Text,
                            tag: TagKind::TrackArtist,
                        }],
                        nested_relations: vec![],
                    },
                ],
            },
            entity_key_attributes: HashMap::from([
                (album, vec![album_title]),
                (artist, vec![artist_name]),
            ]),
            relation_key_attributes: HashMap::new(),
            song_entity: song,
        };

        dbg!(&graph);
        dbg!(&rules);

        let (_watch_tx, watch_rx) = watch::channel(Some((graph, rules)));

        Self { watch_rx }
    }
}

impl ImportSchemaPort for ExampleImportSchemaAdapter {
    fn watch_schema(&self) -> watch::Receiver<Option<(GraphSchema, Rules)>> {
        self.watch_rx.clone()
    }
}
