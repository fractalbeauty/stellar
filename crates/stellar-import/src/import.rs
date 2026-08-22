use lofty::{
    file::TaggedFileExt,
    tag::{ItemKey, Tag},
};
use rayon::iter::{ParallelBridge, ParallelIterator};
use std::{
    collections::{HashMap, hash_map::Entry},
    path::PathBuf,
    sync::Arc,
};
use stellar_graph::{
    entity::{AttributeKind, EntityId, EntityKind, RelationId, RelationKind, Value, ValueKind},
    schema::Schema,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::rules::{AttributeRule, RelationRule, RelationRuleDirection, Rules};

/// Foreign trait for receiving import events.
#[uniffi::export(with_foreign)]
pub trait ImportEventHandler: Send + Sync {
    fn on_pending_file(&self, path: String);
    fn on_scanned_file(&self, file: ImportEventScannedFile);
    fn on_scan_finished(&self);
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ImportEventScannedFile {
    pub path: String,
    pub tags: HashMap<String, String>,
}

/// Handle to an import task.
#[derive(Debug, Clone)]
pub struct ImportTask {
    cancellation_token: CancellationToken,
    message_tx: mpsc::UnboundedSender<ImportMessage>,
}

impl ImportTask {
    pub fn spawn(
        cancellation_token: CancellationToken,
        event_handler: Arc<dyn ImportEventHandler>,
        roots: Vec<PathBuf>,
        schema: Schema,
        song_entity: EntityKind,
    ) -> Result<Self, anyhow::Error> {
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        tokio::spawn({
            let cancellation_token = cancellation_token.clone();
            let message_tx = message_tx.clone();
            async move {
                let mut import = match Import::init(schema, song_entity) {
                    Ok(import) => import,
                    Err(e) => {
                        tracing::error!("Import task failed to init: {e:?}");
                        return;
                    }
                };

                let result = import
                    .run(
                        cancellation_token,
                        event_handler,
                        roots,
                        message_tx,
                        message_rx,
                    )
                    .await;

                if let Err(error) = result {
                    tracing::error!("Import task errored: {error:?}");
                } else {
                    tracing::debug!("Import task finished");
                }
            }
        });

        Ok(Self {
            cancellation_token,
            message_tx,
        })
    }

    // Cancel the import task. This must be called when the UI is done with the import,
    // or the import task will leak.
    //
    // We can't really do this automatically on drop because the import task holds a reference
    // to the host object via ImportEventHandler.
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    /// Import with the configured settings.
    pub fn import(&self, rules: Rules) {
        let _ = self.message_tx.send(ImportMessage::Import(rules));
    }
}

struct Import {
    schema: Schema,
    song_entity: EntityKind,

    files: Vec<ImportMessageScannedFile>,
}

impl Import {
    fn init(schema: Schema, song_entity: EntityKind) -> Result<Self, anyhow::Error> {
        Ok(Self {
            schema,
            song_entity,

            files: Vec::new(),
        })
    }

    async fn run(
        &mut self,
        cancellation_token: CancellationToken,
        event_handler: Arc<dyn ImportEventHandler>,
        roots: Vec<PathBuf>,
        message_tx: mpsc::UnboundedSender<ImportMessage>,
        mut message_rx: mpsc::UnboundedReceiver<ImportMessage>,
    ) -> Result<(), anyhow::Error> {
        let (path_tx, path_rx) = std::sync::mpsc::channel();

        tokio::task::spawn_blocking({
            let cancellation_token = cancellation_token.clone();
            let event_handler = event_handler.clone();
            move || {
                dua_core::walk_roots(
                    roots.into_iter().enumerate(),
                    // TODO
                    4,
                    dua_core::Order::Completion,
                    move |_, _| {
                        // Stop descending if cancelled
                        !cancellation_token.is_cancelled()
                    },
                )
                .filter_map(|(_, entry)| {
                    let path = match entry {
                        dua_core::RootEvent::Entry(entry) => match entry {
                            Ok(entry) => {
                                if !entry.file_type.is_file() {
                                    return None;
                                }
                                entry.path()
                            }
                            Err(e) => {
                                // TODO: report error to UI
                                tracing::error!("Error while walking : {e:?}");
                                return None;
                            }
                        },
                        dua_core::RootEvent::Finished => return None,
                    };

                    // Skip if the file has no extension or the extension is not in Lofty's list of common audio extensions
                    if path
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_none_or(|extension| !lofty::file::EXTENSIONS.contains(&extension))
                    {
                        return None;
                    }

                    // TODO: batch?
                    event_handler.on_pending_file(path.to_string_lossy().to_string());

                    Some(path)
                })
                .for_each(|path| {
                    let _ = path_tx.send(path);
                });

                tracing::info!("Import walk finished");
            }
        });

        tokio::task::spawn_blocking({
            let cancellation_token = cancellation_token.clone();
            let message_tx = message_tx.clone();
            move || {
                path_rx.into_iter().par_bridge().for_each(|path| {
                    // Stop processing if cancelled
                    if cancellation_token.is_cancelled() {
                        return;
                    }

                    let file = match lofty::read_from_path(&path) {
                        Ok(file) => file,
                        Err(e) => {
                            // TODO: report error to UI
                            tracing::error!(?path, "Failed to read file: {e:#}");
                            return;
                        }
                    };

                    let tags = file.primary_tag().or_else(|| file.first_tag());

                    let event_tags = match tags {
                        Some(tag) => {
                            let mut event_tags = HashMap::new();

                            for item in tag.items() {
                                let key = format!("{:?}", item.key());
                                let value = item
                                    .value()
                                    // TODO
                                    .clone()
                                    .into_string()
                                    .unwrap_or_else(|| "<bytes>".to_string());

                                event_tags
                                    .entry(key)
                                    .and_modify(|existing: &mut String| {
                                        existing.push_str(", ");
                                        existing.push_str(&value);
                                    })
                                    .or_insert(value);
                            }

                            event_tags
                        }
                        None => {
                            tracing::debug!(?path, "File has no tags");
                            HashMap::new()
                        }
                    };

                    event_handler.on_scanned_file(ImportEventScannedFile {
                        path: path.to_string_lossy().to_string(),
                        tags: event_tags,
                    });

                    let _ = message_tx.send(ImportMessage::ScannedFile(ImportMessageScannedFile {
                        path,
                        tags: tags.cloned(),
                    }));
                });

                tracing::info!("Import scan finished");

                event_handler.on_scan_finished();
            }
        });

        loop {
            tokio::select! {
                result = message_rx.recv() => {
                    match result {
                        Some(message) => {
                            if let Err(e) = self.handle_message(message).await {
                                tracing::error!("Import failed to handle message: {e:#}");
                            }
                        },
                        None => {
                            tracing::debug!("Import message rx closed");
                        },
                    }
                }

                _ = cancellation_token.cancelled() => {
                    tracing::debug!("Import task cancelled");
                    break;
                }
            }
        }

        Ok(())
    }

    async fn handle_message(&mut self, message: ImportMessage) -> Result<(), anyhow::Error> {
        match message {
            ImportMessage::Import(rules) => self.handle_import(rules),
            ImportMessage::ScannedFile(file) => {
                self.files.push(file);
                Ok(())
            }
        }
    }

    fn handle_import(&mut self, rules: Rules) -> Result<(), anyhow::Error> {
        let mut changes = Changes::default();

        // TODO: multiple rules
        let rule = &rules.rule;

        for file in &self.files {
            let song_attributes = file.attributes(&rule.attributes);

            let entity = changes.create_entity(self.song_entity, song_attributes);

            for relation_rule in &rule.relations {
                changes.handle_relation_rule(file, entity, relation_rule);
            }
        }

        dbg!(changes);

        Ok(())
    }
}

enum ImportMessage {
    Import(Rules),

    ScannedFile(ImportMessageScannedFile),
}

struct ImportMessageScannedFile {
    path: PathBuf,
    tags: Option<Tag>,
}

impl ImportMessageScannedFile {
    fn attributes(&self, rules: &[AttributeRule]) -> HashMap<AttributeKind, Value> {
        let mut attributes = HashMap::new();

        for attribute_rule in rules {
            match attribute_rule.value {
                ValueKind::Text => {
                    if let Some(text) = self.get_text(attribute_rule.tag.to_lofty()) {
                        attributes.insert(attribute_rule.attribute, Value::Text(text.to_string()));
                    }
                }
                ValueKind::Number => {
                    if let Some(text) = self.get_text(attribute_rule.tag.to_lofty()) {
                        let number = text.parse::<f64>().expect("TODO");

                        attributes.insert(attribute_rule.attribute, Value::Number(number));
                    }
                }
                ValueKind::Bytes => unimplemented!(),
            };
        }

        attributes
    }

    fn get_text(&self, item_key: ItemKey) -> Option<&str> {
        self.tags
            .as_ref()
            .and_then(|tags| tags.get_string(item_key))
    }
}

#[derive(Debug, Default)]
struct Changes {
    create_entities: HashMap<EntityKind, Vec<CreateEntityChange>>,
    create_relations: HashMap<RelationKind, Vec<CreateRelationChange>>,
}

#[derive(Debug)]
struct CreateEntityChange {
    id: EntityId,
    attributes: HashMap<AttributeKind, Value>,
}

#[derive(Debug)]
struct CreateRelationChange {
    id: RelationId,
    source: EntityId,
    target: EntityId,
    attributes: HashMap<AttributeKind, Value>,
}

impl Changes {
    fn handle_relation_rule(
        &mut self,
        file: &ImportMessageScannedFile,
        entity: EntityId,
        relation_rule: &RelationRule,
    ) {
        let relation_key_attributes = file.attributes(&relation_rule.relation_key_attributes);
        let relation_extra_attributes = file.attributes(&relation_rule.relation_extra_attributes);
        let other_key_attributes = file.attributes(&relation_rule.other_key_attributes);
        let other_extra_attributes = file.attributes(&relation_rule.other_extra_attributes);

        let other = self.find_or_create_entity(
            relation_rule.other,
            other_key_attributes,
            other_extra_attributes,
        );

        let (source, target) = match relation_rule.direction {
            RelationRuleDirection::Incoming => (other, entity),
            RelationRuleDirection::Outgoing => (entity, other),
        };

        self.find_or_create_relation(
            relation_rule.relation,
            source,
            target,
            relation_key_attributes,
            relation_extra_attributes,
        );

        for nested_relation_rule in &relation_rule.nested_relations {
            self.handle_relation_rule(file, other, nested_relation_rule);
        }
    }

    fn create_entity(
        &mut self,
        entity: EntityKind,
        attributes: HashMap<AttributeKind, Value>,
    ) -> EntityId {
        let id = EntityId::random(entity);
        let change = CreateEntityChange { id, attributes };
        match self.create_entities.entry(entity) {
            Entry::Occupied(entry) => {
                entry.into_mut().push(change);
            }
            Entry::Vacant(entry) => {
                entry.insert(vec![change]);
            }
        };
        id
    }

    fn find_or_create_entity(
        &mut self,
        entity: EntityKind,
        key_attributes: HashMap<AttributeKind, Value>,
        extra_attributes: HashMap<AttributeKind, Value>,
    ) -> EntityId {
        let existing = self
            .create_entities
            .get_mut(&entity)
            .and_then(|created_entities| {
                created_entities.iter_mut().find(|created_entity| {
                    key_attributes.iter().all(|(key_attribute, value)| {
                        created_entity
                            .attributes
                            .get(key_attribute)
                            .is_some_and(|existing_value| existing_value == value)
                    })
                })
            });

        match existing {
            Some(existing) => {
                // TODO: merge extra_attributes

                existing.id
            }
            None => {
                let id = EntityId::random(entity);
                let change = CreateEntityChange {
                    id,
                    // TODO: merge extra_attributes
                    attributes: key_attributes,
                };
                match self.create_entities.entry(entity) {
                    Entry::Occupied(entry) => {
                        entry.into_mut().push(change);
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(vec![change]);
                    }
                };
                id
            }
        }
    }

    fn find_or_create_relation(
        &mut self,
        relation: RelationKind,
        source: EntityId,
        target: EntityId,
        key_attributes: HashMap<AttributeKind, Value>,
        extra_attributes: HashMap<AttributeKind, Value>,
    ) -> RelationId {
        let existing = self
            .create_relations
            .get_mut(&relation)
            .and_then(|created_relations| {
                created_relations.iter_mut().find(|created_relation| {
                    created_relation.source == source
                        && created_relation.target == target
                        && key_attributes.iter().all(|(key_attribute, value)| {
                            created_relation
                                .attributes
                                .get(key_attribute)
                                .is_some_and(|existing_value| existing_value == value)
                        })
                })
            });

        match existing {
            Some(existing) => {
                // TODO: merge extra_attributes

                existing.id
            }
            None => {
                let id = RelationId::random(relation);
                let change = CreateRelationChange {
                    id,
                    source,
                    target,
                    // TODO: merge extra_attributes
                    attributes: key_attributes,
                };
                match self.create_relations.entry(relation) {
                    Entry::Occupied(entry) => {
                        entry.into_mut().push(change);
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(vec![change]);
                    }
                };
                id
            }
        }
    }
}
