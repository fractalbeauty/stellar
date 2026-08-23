use crate::{
    ports::ImportDatabasePort,
    rules::{AttributeRule, RelationRule, RelationRuleDirection, Rules},
};
use lofty::tag::{ItemKey, Tag};
use std::{
    collections::{HashMap, hash_map::Entry},
    iter,
    path::PathBuf,
    sync::Arc,
};
use stellar_graph::entity::{
    AttributeKind, AuthorId, EntityId, EntityKind, RelationId, RelationKind, Value, ValueKind,
};
use stellar_resources::audio::{
    AUDIO_RESOURCE_ENTITY, AUDIO_RESOURCE_LOCATION, AudioResourceLocation,
};

pub struct Evaluator<'a> {
    changes: Changes,
    rules: &'a Rules,
    database: &'a Arc<dyn ImportDatabasePort>,
    song_entity: EntityKind,
    song_audio_resource: RelationKind,
    device: AuthorId,
}

impl<'a> Evaluator<'a> {
    pub fn run(
        rules: &'a Rules,
        database: &'a Arc<dyn ImportDatabasePort>,
        song_entity: EntityKind,
        song_audio_resource: RelationKind,
        device: AuthorId,
        files: &[EvaluatorFile],
    ) -> Result<Changes, anyhow::Error> {
        let mut evaluator = Self {
            changes: Changes::default(),
            rules,
            database,
            song_entity,
            song_audio_resource,
            device,
        };
        evaluator.evaluate(files)?;
        Ok(evaluator.changes)
    }

    fn evaluate(&mut self, files: &[EvaluatorFile]) -> Result<(), anyhow::Error> {
        // TODO: multiple rules
        let rule = &self.rules.rule;

        for file in files {
            let song_attributes: HashMap<AttributeKind, Value> = file.attributes(&rule.attributes);

            let entity = self
                .changes
                .create_entity(self.song_entity, song_attributes);

            for relation_rule in &rule.relations {
                self.handle_relation_rule(file, entity, relation_rule);
            }

            let audio_resource_attributes = HashMap::from([(
                AUDIO_RESOURCE_LOCATION,
                Value::Bytes(AudioResourceLocation::encode(&AudioResourceLocation {
                    device: self.device,
                    path: file.path.clone(),
                })),
            )]);
            let audio_resource = self
                .changes
                .create_entity(AUDIO_RESOURCE_ENTITY, audio_resource_attributes);
            self.changes.find_or_create_relation(
                self.song_audio_resource,
                entity,
                audio_resource,
                HashMap::new(),
                iter::empty(),
            );
        }

        for (kind, created_entities) in &self.changes.create_entities {
            let existing = self.database.get_entities_by_kind(*kind)?;

            let key_attributes = self.rules.iter_entity_key_attributes(*kind);

            // If there are no key attributes, don't merge anything
            if key_attributes.len() == 0 {
                continue;
            }

            // Build index by key attributes
            let index = existing
                .iter()
                .filter_map(|(id, existing)| {
                    let mut values = Vec::with_capacity(key_attributes.len());
                    for key_attribute in key_attributes.clone() {
                        let Some(value) = existing.attributes.get(&key_attribute) else {
                            return None;
                        };
                        values.push(value.value.clone());
                    }

                    Some((values, *id))
                })
                .collect::<HashMap<_, _>>();

            'entity: for created_entity in created_entities {
                let mut values = Vec::with_capacity(key_attributes.len());
                for key_attribute in key_attributes.clone() {
                    let Some(value) = created_entity.attributes.get(&key_attribute) else {
                        continue 'entity;
                    };
                    values.push(value.clone());
                }

                let existing = index.get(&values).copied();
                if let Some(existing) = existing {
                    println!(
                        "detected {:?} is a duplicate of {:?}, not implemented",
                        created_entity, existing
                    );

                    // TODO: merge duplicates and update relations
                }
            }

            // TODO: detect duplicate relations and merge
        }

        Ok(())
    }

    fn handle_relation_rule(
        &mut self,
        file: &EvaluatorFile,
        entity: EntityId,
        relation_rule: &RelationRule,
    ) {
        let other_attributes = file.attributes(&relation_rule.other_attributes);
        let other_key_attributes = self.rules.iter_entity_key_attributes(relation_rule.other);

        let other = self.changes.find_or_create_entity(
            relation_rule.other,
            other_attributes,
            other_key_attributes,
        );

        let (source, target) = match relation_rule.direction {
            RelationRuleDirection::Incoming => (other, entity),
            RelationRuleDirection::Outgoing => (entity, other),
        };

        let relation_attributes = file.attributes(&relation_rule.relation_attributes);
        let relation_key_attributes = self
            .rules
            .iter_relation_key_attributes(relation_rule.relation);

        self.changes.find_or_create_relation(
            relation_rule.relation,
            source,
            target,
            relation_attributes,
            relation_key_attributes,
        );

        for nested_relation_rule in &relation_rule.nested_relations {
            self.handle_relation_rule(file, other, nested_relation_rule);
        }
    }
}

#[derive(Debug, PartialEq, Default)]
pub struct Changes {
    pub create_entities: HashMap<EntityKind, Vec<CreateEntityChange>>,
    pub create_relations: HashMap<RelationKind, Vec<CreateRelationChange>>,
}

#[derive(Debug, PartialEq)]
pub struct CreateEntityChange {
    pub id: EntityId,
    pub attributes: HashMap<AttributeKind, Value>,
}

#[derive(Debug, PartialEq)]
pub struct CreateRelationChange {
    pub id: RelationId,
    pub source: EntityId,
    pub target: EntityId,
    pub attributes: HashMap<AttributeKind, Value>,
}

impl Changes {
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

    /// Find an existing entity to reuse, matching by `key_attributes`.
    ///
    /// Matches if:
    /// - There are `key_attributes`
    /// - All `key_attributes` are present on both entities and are equal
    fn find_entity(
        &mut self,
        entity: EntityKind,
        attributes: &HashMap<AttributeKind, Value>,
        mut key_attributes: impl ExactSizeIterator<Item = AttributeKind>,
    ) -> Option<&mut CreateEntityChange> {
        // If there are no key attributes, always create a new entity
        if key_attributes.len() == 0 {
            return None;
        }

        let Some(entities) = self.create_entities.get_mut(&entity) else {
            // No entities of kind created yet
            return None;
        };

        // Match by key attributes
        entities.iter_mut().find(|created_entity| {
            key_attributes.all(|key_attribute| {
                let incoming_value = attributes.get(&key_attribute);
                let created_value = created_entity.attributes.get(&key_attribute);

                match (incoming_value, created_value) {
                    // Match if the value of the key attribute matches
                    (Some(incoming_value), Some(created_value)) => incoming_value == created_value,
                    // If the incoming or existing attributes are missing a key attribute, don't match
                    (_, _) => false,
                }
            })
        })
    }

    fn find_or_create_entity(
        &mut self,
        entity: EntityKind,
        attributes: HashMap<AttributeKind, Value>,
        key_attributes: impl ExactSizeIterator<Item = AttributeKind>,
    ) -> EntityId {
        let existing = self.find_entity(entity, &attributes, key_attributes);

        match existing {
            Some(existing) => {
                // TODO: merge attributes

                existing.id
            }
            None => {
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
        }
    }

    /// Find an existing relation to reuse, matching by `source` and `target`, then `key_attributes`.
    ///
    /// Matches if:
    /// - `source` and `target` are the same
    /// - If there are `key_attributes`, all `key_attributes` match
    fn find_relation(
        &mut self,
        relation: RelationKind,
        source: EntityId,
        target: EntityId,
        attributes: &HashMap<AttributeKind, Value>,
        mut key_attributes: impl ExactSizeIterator<Item = AttributeKind>,
    ) -> Option<&mut CreateRelationChange> {
        let Some(relations) = self.create_relations.get_mut(&relation) else {
            // No relations of kind created yet
            return None;
        };

        // Filter by `source` and `target`
        let mut entity_matches = relations.iter_mut().filter(|created_relation| {
            created_relation.source == source && created_relation.target == target
        });

        // If there are no key attributes, return the first match
        if key_attributes.len() == 0 {
            return entity_matches.next();
        }

        // Match by key attributes
        entity_matches.find(|created_relation| {
            key_attributes.all(|key_attribute| {
                let incoming_value = attributes.get(&key_attribute);
                let created_value = created_relation.attributes.get(&key_attribute);

                match (incoming_value, created_value) {
                    // Match if the value of the key attribute matches
                    (Some(incoming_value), Some(created_value)) => incoming_value == created_value,
                    // If the incoming or existing attributes are missing a key attribute, don't match
                    (_, _) => false,
                }
            })
        })
    }

    fn find_or_create_relation(
        &mut self,
        relation: RelationKind,
        source: EntityId,
        target: EntityId,
        attributes: HashMap<AttributeKind, Value>,
        key_attributes: impl ExactSizeIterator<Item = AttributeKind>,
    ) -> RelationId {
        let existing = self.find_relation(relation, source, target, &attributes, key_attributes);

        match existing {
            Some(existing) => {
                // TODO: merge attributes

                existing.id
            }
            None => {
                let id = RelationId::random(relation);
                let change = CreateRelationChange {
                    id,
                    source,
                    target,
                    attributes,
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

pub struct EvaluatorFile {
    pub path: PathBuf,
    pub tags: Option<Tag>,
}

impl EvaluatorFile {
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

                        attributes.insert(attribute_rule.attribute, Value::number_from_f64(number));
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

#[cfg(test)]
mod test {
    use crate::{
        evaluator::{Evaluator, EvaluatorFile},
        ports::ImportDatabasePort,
        rules::{AttributeRule, RelationRule, RelationRuleDirection, Rule, Rules, TagKind},
    };
    use lofty::tag::{Accessor, Tag};
    use std::{collections::HashMap, path::PathBuf, sync::Arc};
    use stellar_graph::{
        entity::{
            AttributeKind, AuthorId, EntityId, EntityKind, RelationKind, Timestamp, Value,
            ValueKind, Version,
        },
        store::{EntityAttributeValue, EntityData, EntityMetadataValue},
    };

    struct TestImportDatabaseAdapter {
        entities: HashMap<EntityId, EntityData>,
    }

    impl ImportDatabasePort for TestImportDatabaseAdapter {
        fn get_entities_by_kind(
            &self,
            kind: stellar_graph::entity::EntityKind,
        ) -> Result<HashMap<EntityId, EntityData>, anyhow::Error> {
            Ok(self
                .entities
                .clone()
                .into_iter()
                .filter(|(id, _)| id.kind() == kind)
                .collect())
        }
    }

    #[test]
    fn merges_with_database() {
        let song = EntityKind::random();
        let song_title = AttributeKind::random();

        let album = EntityKind::random();
        let album_title = AttributeKind::random();

        let album_song = RelationKind::random();

        let song_audio_resource = RelationKind::random();

        let rules = Rules {
            rule: Rule {
                attributes: vec![AttributeRule {
                    attribute: song_title,
                    value: ValueKind::Text,
                    tag: TagKind::TrackTitle,
                }],
                relations: vec![RelationRule {
                    relation: album_song,
                    other: album,
                    direction: RelationRuleDirection::Incoming,
                    relation_attributes: vec![],
                    other_attributes: vec![AttributeRule {
                        attribute: album_title,
                        value: ValueKind::Text,
                        tag: TagKind::AlbumTitle,
                    }],
                    nested_relations: vec![],
                }],
            },
            entity_key_attributes: HashMap::from([(album, vec![album_title])]),
            relation_key_attributes: HashMap::new(),
        };

        let existing_album = EntityId::random(album);

        let empty_version = Version::new(Timestamp::new(0), AuthorId::from_bytes([0u8; 32]));

        let database: Arc<dyn ImportDatabasePort> = Arc::new(TestImportDatabaseAdapter {
            entities: HashMap::from([(
                existing_album,
                EntityData {
                    metadata: EntityMetadataValue {
                        deleted: false,
                        deleted_version: empty_version.clone(),
                    },
                    attributes: HashMap::from([(
                        album_title,
                        EntityAttributeValue {
                            value: Value::Text("test album".to_string()),
                            version: empty_version.clone(),
                        },
                    )]),
                },
            )]),
        });

        let files = [EvaluatorFile {
            path: PathBuf::from("test song.mp3"),
            tags: Some({
                let mut tag = Tag::new(lofty::tag::TagType::Id3v2);
                tag.set_title("test song".to_string());
                tag.set_album("test album".to_string());
                tag
            }),
        }];

        let changes = Evaluator::run(
            &rules,
            &database,
            song,
            song_audio_resource,
            AuthorId::from_bytes([0u8; 32]),
            &files,
        )
        .expect("should run");

        // TODO
        assert_eq!(changes.create_entities, HashMap::new());
    }
}
