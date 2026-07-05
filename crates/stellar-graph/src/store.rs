use crate::entity::{AttributeKind, EntityId, EntityKind, Value, Version};
use anyhow::Context;
use fjall::{Database, Keyspace, KeyspaceCreateOptions, Slice};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};
use uuid::Uuid;

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

    pub fn set_entity_metadata(
        &self,
        entity: EntityId,
        value: EntityMetadataValue,
    ) -> Result<(), anyhow::Error> {
        let key = make_entity_metadata_key(entity);
        let value = postcard::to_allocvec(&value)?;
        self.keyspace.insert(key, value)?;
        Ok(())
    }

    fn set_entity_attribute(
        &self,
        entity: EntityId,
        attribute: AttributeKind,
        value: EntityAttributeValue,
    ) -> Result<(), anyhow::Error> {
        let key = make_entity_attribute_key(entity, attribute);
        let value = postcard::to_allocvec(&value)?;
        self.keyspace.insert(key, value)?;
        Ok(())
    }

    pub fn get_entities(&self) -> Result<HashMap<EntityId, EntityData>, anyhow::Error> {
        let mut metadata_iter = self.keyspace.prefix([ENTITY_METADATA_PREFIX]).map(|guard| {
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

        while let Some(next) = metadata_iter.next() {
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

#[derive(Debug, Clone, PartialEq)]
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
        entity::{AttributeKind, AuthorId, EntityId, EntityKind, Timestamp, Value, Version},
        store::{EntityAttributeValue, EntityMetadataValue, Store},
    };

    fn version() -> Version {
        Version::new(Timestamp::now(), AuthorId::new([1u8; 32]))
    }

    fn metadata(kind: EntityKind, deleted: bool) -> EntityMetadataValue {
        EntityMetadataValue {
            kind,
            deleted,
            deleted_version: version(),
        }
    }

    fn attribute(value: Value) -> EntityAttributeValue {
        EntityAttributeValue {
            value,
            version: version(),
        }
    }

    #[test]
    fn get_entities() {
        let store = Store::open(testdir::testdir!().join("store")).expect("should open");

        let k1 = EntityKind::random();

        let a1 = AttributeKind::random();
        let a2 = AttributeKind::random();

        let e1 = EntityId::random();
        store
            .set_entity_metadata(e1, metadata(k1, false))
            .expect("should set entity metadata");
        store
            .set_entity_attribute(e1, a1, attribute(Value::Number(1.0)))
            .expect("should set entity attribute");
        store
            .set_entity_attribute(e1, a2, attribute(Value::Number(2.0)))
            .expect("should set entity attribute");

        let e2 = EntityId::random();
        store
            .set_entity_metadata(e2, metadata(k1, false))
            .expect("should set entity metadata");
        store
            .set_entity_attribute(e2, a1, attribute(Value::Number(3.0)))
            .expect("should set entity attribute");
        store
            .set_entity_attribute(e2, a2, attribute(Value::Number(4.0)))
            .expect("should set entity attribute");

        let entities = store.get_entities().expect("should get entities");

        assert_eq!(entities.len(), 2);
        assert!(entities.contains_key(&e1));
        assert!(entities.contains_key(&e2));

        assert_eq!(entities.get(&e1).unwrap().attributes.len(), 2);
        assert!(entities.get(&e1).unwrap().attributes.contains_key(&a1));
        assert_eq!(
            entities
                .get(&e1)
                .unwrap()
                .attributes
                .get(&a1)
                .unwrap()
                .value,
            Value::Number(1.0)
        );
    }
}

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
