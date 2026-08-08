use crate::entity::{AttributeKind, EntityId, EntityKind, Value, Version};
use anyhow::Context;
use fjall::{Database, Keyspace, KeyspaceCreateOptions, Slice};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};
use tracing::warn;
use uuid::Uuid;

/// Handle to the store for graph data. Provides primitive operations.
#[derive(Clone)]
pub struct Store {
    database: Database,
    keyspace: Keyspace,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let database = Database::builder(path).open()?;

        let keyspace = database.keyspace("graph", KeyspaceCreateOptions::default)?;

        Ok(Self { database, keyspace })
    }

    pub fn get_entity_metadata(
        &self,
        entity: EntityId,
    ) -> Result<Option<EntityMetadataValue>, anyhow::Error> {
        let key = make_entity_metadata_key(entity);
        let value = self
            .keyspace
            .get(key)?
            .map(|value| {
                postcard::from_bytes::<EntityMetadataValue>(value.as_ref())
                    .context("Failed to parse metadata value")
            })
            .transpose()?;
        Ok(value)
    }

    pub fn merge_entity_metadata(
        &self,
        entity: EntityId,
        value: EntityMetadataValue,
    ) -> Result<(), anyhow::Error> {
        let key = make_entity_metadata_key(entity);

        let existing = self
            .keyspace
            .get(key)?
            .map(|value| {
                postcard::from_bytes::<EntityMetadataValue>(value.as_ref())
                    .context("Failed to parse metadata value")
            })
            .transpose()?;

        // TODO: don't write if the incoming value was older. this might not apply if there are multiple metadata fields to be merged eventually
        let merged_value = if let Some(existing) = existing {
            if value.kind != existing.kind {
                warn!("merge_entity_metadata new kind != existing kind");
            }

            let (deleted, deleted_version) = Version::latest_version(
                value.deleted,
                value.deleted_version,
                existing.deleted,
                existing.deleted_version,
            );
            EntityMetadataValue {
                kind: existing.kind,
                deleted,
                deleted_version,
            }
        } else {
            value
        };

        let merged_value = postcard::to_allocvec(&merged_value)?;
        self.keyspace.insert(key, merged_value)?;

        Ok(())
    }

    pub fn merge_entity_attribute(
        &self,
        entity: EntityId,
        attribute: AttributeKind,
        value: EntityAttributeValue,
    ) -> Result<(), anyhow::Error> {
        let key = make_entity_attribute_key(entity, attribute);

        let existing = self
            .keyspace
            .get(key)?
            .map(|value| {
                postcard::from_bytes::<EntityAttributeValue>(value.as_ref())
                    .context("Failed to parse attribute value")
            })
            .transpose()?;

        // TODO: don't write if the incoming value was older
        let merged_value = if let Some(existing) = existing {
            let (value, version) = Version::latest_version(
                value.value,
                value.version,
                existing.value,
                existing.version,
            );
            EntityAttributeValue { value, version }
        } else {
            value
        };

        let merged_value = postcard::to_allocvec(&merged_value)?;
        self.keyspace.insert(key, merged_value)?;

        Ok(())
    }

    pub fn get_entities(&self) -> Result<HashMap<EntityId, EntityData>, anyhow::Error> {
        let metadata_iter = self.keyspace.prefix([ENTITY_METADATA_PREFIX]).map(|guard| {
            let (key, value) = guard.into_inner().context("Fjall error reading metadata")?;

            let key = parse_entity_metadata_key(key).context("Failed to parse metadata key")?;
            let value = postcard::from_bytes::<EntityMetadataValue>(value.as_ref())
                .context("Failed to parse metadata value")?;

            Ok::<_, anyhow::Error>((key, value))
        });

        let mut attributes_iter = self
            .keyspace
            .prefix([ENTITY_ATTRIBUTE_PREFIX])
            .map(|guard| {
                let (key, value) = guard
                    .into_inner()
                    .context("Fjall error reading attribute")?;

                let key =
                    parse_entity_attribute_key(key).context("Failed to parse attribute key")?;
                let value = postcard::from_bytes::<EntityAttributeValue>(value.as_ref())
                    .context("Failed to parse attribute value")?;

                Ok::<_, anyhow::Error>((key, value))
            })
            .peekable();

        let mut entities = HashMap::new();

        for next in metadata_iter {
            let (entity, metadata) = next?;

            let mut attributes = HashMap::new();

            while attributes_iter.peek().is_some_and(|next| {
                let Ok(((attribute_entity, _attribute_kind), _attribute_value)) = next else {
                    // Error, enter loop to bail out
                    return true;
                };

                // Enter loop if this attribute is for the current entity
                *attribute_entity == entity
            }) {
                let ((_attribute_entity, attribute_kind), attribute_value) =
                    attributes_iter.next().expect("peek returned Some")?;

                attributes.insert(attribute_kind, attribute_value);
            }

            entities.insert(
                entity,
                EntityData {
                    metadata,
                    attributes,
                },
            );
        }

        Ok(entities)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityData {
    pub metadata: EntityMetadataValue,
    pub attributes: HashMap<AttributeKind, EntityAttributeValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityMetadataValue {
    pub kind: EntityKind,
    pub deleted: bool,
    pub deleted_version: Version,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityAttributeValue {
    pub value: Value,
    pub version: Version,
}

const ENTITY_METADATA_PREFIX: u8 = 1u8;
const ENTITY_ATTRIBUTE_PREFIX: u8 = 2u8;

fn make_entity_metadata_key(entity: EntityId) -> [u8; 17] {
    let mut key = [0u8; 17];
    key[0] = ENTITY_METADATA_PREFIX;
    key[1..17].copy_from_slice(entity.inner().as_bytes());
    key
}

fn parse_entity_metadata_key(key: Slice) -> Result<EntityId, anyhow::Error> {
    if key.len() != 17 {
        anyhow::bail!("wrong key len");
    }
    let entity = EntityId::new(Uuid::from_slice(&key[1..17])?);
    Ok(entity)
}

fn make_entity_attribute_key(entity: EntityId, attribute: AttributeKind) -> [u8; 33] {
    let mut key = [0u8; 33];
    key[0] = ENTITY_ATTRIBUTE_PREFIX;
    key[1..17].copy_from_slice(entity.inner().as_bytes());
    key[17..33].copy_from_slice(attribute.inner().as_bytes());
    key
}

fn parse_entity_attribute_key(key: Slice) -> Result<(EntityId, AttributeKind), anyhow::Error> {
    if key.len() != 33 {
        anyhow::bail!("wrong key len");
    }
    let entity = EntityId::new(Uuid::from_slice(&key[1..17])?);
    let attribute = AttributeKind::new(Uuid::from_slice(&key[17..33])?);
    Ok((entity, attribute))
}

#[cfg(test)]
mod test {
    use crate::{
        entity::{
            Version,
            hegel::{gen_attribute_kind, gen_entity_id, gen_entity_kind, gen_value, gen_version},
        },
        store::{EntityMetadataValue, Store, hegel::gen_entity_data},
    };
    use hegel::{Generator, TestCase, generators as gs};
    use uuid::Uuid;

    use super::EntityAttributeValue;

    #[hegel::test]
    fn retrieve_entities(tc: TestCase) {
        let store = Store::open(
            testdir::testdir!()
                .join(Uuid::new_v4().to_string())
                .join("store"),
        )
        .expect("should open");

        let entities = tc.draw(gs::hashmaps(gen_entity_id(), gen_entity_data()));

        for (entity, data) in entities.clone() {
            store
                .merge_entity_metadata(entity, data.metadata)
                .expect("should merge entity metadata");

            for (attribute, value) in data.attributes {
                store
                    .merge_entity_attribute(entity, attribute, value)
                    .expect("should merge entity attribute");
            }
        }

        let result = store.get_entities().expect("should get entities");
        assert_eq!(result, entities);
    }

    #[hegel::test]
    fn merge_entity_attribute(tc: TestCase) {
        let store = Store::open(
            testdir::testdir!()
                .join(Uuid::new_v4().to_string())
                .join("store"),
        )
        .expect("should open");

        let entity = tc.draw(gen_entity_id());

        store
            .merge_entity_metadata(
                entity,
                EntityMetadataValue {
                    kind: tc.draw(gen_entity_kind()),
                    deleted: false,
                    deleted_version: tc.draw(gen_version()),
                },
            )
            .expect("should merge entity metadata");

        let attribute = tc.draw(gen_attribute_kind());

        // Generate attribute
        let first = EntityAttributeValue {
            value: tc.draw(gen_value()),
            version: tc.draw(gen_version()),
        };

        store
            .merge_entity_attribute(entity, attribute, first.clone())
            .expect("should merge entity attribute");

        let first_result = store
            .get_entities()
            .expect("should get entities")
            .clone()
            .get(&entity)
            .expect("should get entity")
            .clone()
            .attributes
            .get(&attribute)
            .expect("should get attribute")
            .clone();
        assert_eq!(first_result, first);

        // Generate attribute with different version
        let second = EntityAttributeValue {
            value: tc.draw(gen_value()),
            version: tc.draw(gen_version().filter(|version| *version != first.version)),
        };

        store
            .merge_entity_attribute(entity, attribute, second.clone())
            .expect("should merge entity attribute");

        let second_result = store
            .get_entities()
            .expect("should get entities")
            .clone()
            .get(&entity)
            .expect("should get entity")
            .clone()
            .attributes
            .get(&attribute)
            .expect("should get attribute")
            .clone();

        let (latest_value, latest_version) =
            Version::latest_version(first.value, first.version, second.value, second.version);
        assert_eq!(
            second_result,
            EntityAttributeValue {
                value: latest_value,
                version: latest_version
            }
        );
    }
}

pub mod hegel {
    use crate::{
        entity::hegel::{gen_attribute_kind, gen_entity_kind, gen_value, gen_version},
        store::{EntityAttributeValue, EntityData, EntityMetadataValue},
    };
    use hegel::{TestCase, compose, generators as gs};

    #[hegel::composite]
    pub fn gen_entity_data(tc: TestCase) -> EntityData {
        EntityData {
            metadata: EntityMetadataValue {
                kind: tc.draw(gen_entity_kind()),
                deleted: tc.draw(gs::booleans()),
                deleted_version: tc.draw(gen_version()),
            },
            attributes: tc.draw(gs::hashmaps(
                gen_attribute_kind(),
                compose!(|tc| {
                    EntityAttributeValue {
                        value: tc.draw(gen_value()),
                        version: tc.draw(gen_version()),
                    }
                }),
            )),
        }
    }
}

// TODO
// fn sort_relation_key(
//     a: EntityId,
//     b: EntityId,
//     a_kind: EntityKind,
//     b_kind: EntityKind,
// ) -> (EntityId, EntityId) {
//     if a_kind == b_kind {
//         if a.inner() < b.inner() {
//             (a, b)
//         } else {
//             (b, a)
//         }
//     } else if a_kind.inner() < b_kind.inner() {
//         (a, b)
//     } else {
//         (b, a)
//     }
// }
