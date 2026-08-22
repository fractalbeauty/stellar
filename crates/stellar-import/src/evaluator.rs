use crate::{
    ports::ImportDatabasePort,
    rules::{AttributeRule, RelationRule, RelationRuleDirection, Rules},
};
use lofty::tag::{ItemKey, Tag};
use std::{
    collections::{HashMap, hash_map::Entry},
    path::PathBuf,
    sync::Arc,
};
use stellar_graph::entity::{
    AttributeKind, EntityId, EntityKind, RelationId, RelationKind, Value, ValueKind,
};

pub struct Evaluator {}

impl Evaluator {
    pub fn run(
        rules: &Rules,
        database: &Arc<dyn ImportDatabasePort>,
        song_entity: EntityKind,
        files: &[EvaluatorFile],
    ) -> Changes {
        let mut changes = Changes::default();

        // TODO: multiple rules
        let rule = &rules.rule;

        for file in files {
            let song_attributes = file.attributes(&rule.attributes);

            let entity = changes.create_entity(song_entity, song_attributes);

            for relation_rule in &rule.relations {
                changes.handle_relation_rule(file, entity, relation_rule);
            }
        }

        changes
    }
}

#[derive(Debug, Default)]
pub struct Changes {
    pub create_entities: HashMap<EntityKind, Vec<CreateEntityChange>>,
    pub create_relations: HashMap<RelationKind, Vec<CreateRelationChange>>,
}

#[derive(Debug)]
pub struct CreateEntityChange {
    pub id: EntityId,
    pub attributes: HashMap<AttributeKind, Value>,
}

#[derive(Debug)]
pub struct CreateRelationChange {
    pub id: RelationId,
    pub source: EntityId,
    pub target: EntityId,
    pub attributes: HashMap<AttributeKind, Value>,
}

impl Changes {
    fn handle_relation_rule(
        &mut self,
        file: &EvaluatorFile,
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
