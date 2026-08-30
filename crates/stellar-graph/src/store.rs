use crate::entity::{
    AttributeKind, EntityId, EntityKind, RelationId, RelationKind, Value, Version,
};
use anyhow::Context;
use fjall::{Database, Keyspace, KeyspaceCreateOptions, Slice};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{collections::HashMap, marker::PhantomData, path::Path};

/// Handle to the store for graph data. Provides primitive operations.
#[derive(Clone)]
pub struct Store {
    database: Database,
    keyspace: Keyspace,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let database = Database::builder(path).open()?;

        let keyspace = database.keyspace("graph_v1", KeyspaceCreateOptions::default)?;

        Ok(Self { database, keyspace })
    }

    pub fn get_entity_metadata(
        &self,
        entity: EntityId,
    ) -> Result<Option<EntityMetadataValue>, anyhow::Error> {
        let key = make_entity_metadata_key(entity);
        let metadata = self
            .keyspace
            .get(key)?
            .map(|value| {
                postcard::from_bytes::<EntityMetadataValue>(value.as_ref())
                    .context("Failed to parse metadata value")
            })
            .transpose()?;
        Ok(metadata)
    }

    pub fn get_relation_metadata(
        &self,
        relation: RelationId,
    ) -> Result<Option<RelationMetadataValue>, anyhow::Error> {
        let key = make_relation_metadata_key(relation);
        let metadata = self
            .keyspace
            .get(key)?
            .map(|value| {
                postcard::from_bytes::<RelationMetadataValue>(value.as_ref())
                    .context("Failed to parse metadata value")
            })
            .transpose()?;
        Ok(metadata)
    }

    pub fn merge_entity_metadata(
        &self,
        entity: EntityId,
        incoming: EntityMetadataValue,
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

        if let Some(existing) = existing {
            if incoming
                .deleted_version
                .greater_than(existing.deleted_version)
            {
                // changed
                let metadata = postcard::to_allocvec(&EntityMetadataValue {
                    deleted: incoming.deleted,
                    deleted_version: incoming.deleted_version,
                })?;
                self.keyspace.insert(key, metadata)?;
                Ok(())
            } else {
                // unchanged
                Ok(())
            }
        } else {
            // new
            let metadata = postcard::to_allocvec(&EntityMetadataValue {
                deleted: incoming.deleted,
                deleted_version: incoming.deleted_version,
            })?;
            self.keyspace.insert(key, metadata)?;
            Ok(())
        }
    }

    pub fn merge_entity_attribute(
        &self,
        entity: EntityId,
        attribute: AttributeKind,
        incoming: EntityAttributeValue,
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

        if let Some(existing) = existing {
            if incoming.version.greater_than(existing.version) {
                // changed
                let metadata = postcard::to_allocvec(&EntityAttributeValue {
                    value: incoming.value,
                    version: incoming.version,
                })?;
                self.keyspace.insert(key, metadata)?;
                Ok(())
            } else {
                // unchanged
                Ok(())
            }
        } else {
            // new
            let metadata = postcard::to_allocvec(&EntityAttributeValue {
                value: incoming.value,
                version: incoming.version,
            })?;
            self.keyspace.insert(key, metadata)?;
            Ok(())
        }
    }

    pub fn merge_relation_metadata(
        &self,
        relation: RelationId,
        incoming: RelationMetadataValue,
    ) -> Result<(), anyhow::Error> {
        let key = make_relation_metadata_key(relation);

        let existing = self
            .keyspace
            .get(key)?
            .map(|value| {
                postcard::from_bytes::<RelationMetadataValue>(value.as_ref())
                    .context("Failed to parse metadata value")
            })
            .transpose()?;

        if let Some(existing) = existing {
            if incoming.source != existing.source {
                tracing::warn!("merge_relation_metadata incoming.source != existing.source");
            }
            if incoming.target != existing.target {
                tracing::warn!("merge_relation_metadata incoming.target != existing.target");
            }

            if incoming
                .deleted_version
                .greater_than(existing.deleted_version)
            {
                // changed
                let metadata = postcard::to_allocvec(&RelationMetadataValue {
                    source: incoming.source,
                    target: incoming.target,
                    deleted: incoming.deleted,
                    deleted_version: incoming.deleted_version,
                })?;
                self.keyspace.insert(key, metadata)?;

                let source_index = postcard::to_allocvec(&RelationIndexValue {
                    other: incoming.target,
                    deleted: incoming.deleted,
                })?;
                let source_key = make_relation_source_key(incoming.source, relation);
                self.keyspace.insert(source_key, source_index)?;

                let target_index = postcard::to_allocvec(&RelationIndexValue {
                    other: incoming.source,
                    deleted: incoming.deleted,
                })?;
                let target_key = make_relation_target_key(incoming.target, relation);
                self.keyspace.insert(target_key, target_index)?;

                Ok(())
            } else {
                // unchanged
                Ok(())
            }
        } else {
            // new
            let metadata = postcard::to_allocvec(&RelationMetadataValue {
                source: incoming.source,
                target: incoming.target,
                deleted: incoming.deleted,
                deleted_version: incoming.deleted_version,
            })?;
            self.keyspace.insert(key, metadata)?;

            let source_index = postcard::to_allocvec(&RelationIndexValue {
                other: incoming.target,
                deleted: incoming.deleted,
            })?;
            let source_key = make_relation_source_key(incoming.source, relation);
            self.keyspace.insert(source_key, source_index)?;

            let target_index = postcard::to_allocvec(&RelationIndexValue {
                other: incoming.source,
                deleted: incoming.deleted,
            })?;
            let target_key = make_relation_target_key(incoming.target, relation);
            self.keyspace.insert(target_key, target_index)?;

            Ok(())
        }
    }

    pub fn merge_relation_attribute(
        &self,
        relation: RelationId,
        attribute: AttributeKind,
        incoming: RelationAttributeValue,
    ) -> Result<(), anyhow::Error> {
        let key = make_relation_attribute_key(relation, attribute);

        let existing = self
            .keyspace
            .get(key)?
            .map(|value| {
                postcard::from_bytes::<RelationAttributeValue>(value.as_ref())
                    .context("Failed to parse attribute value")
            })
            .transpose()?;

        if let Some(existing) = existing {
            if incoming.version.greater_than(existing.version) {
                // changed
                let metadata = postcard::to_allocvec(&RelationAttributeValue {
                    value: incoming.value,
                    version: incoming.version,
                })?;
                self.keyspace.insert(key, metadata)?;
                Ok(())
            } else {
                // unchanged
                Ok(())
            }
        } else {
            // new
            let metadata = postcard::to_allocvec(&RelationAttributeValue {
                value: incoming.value,
                version: incoming.version,
            })?;
            self.keyspace.insert(key, metadata)?;
            Ok(())
        }
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

    pub fn get_relations(&self) -> Result<HashMap<RelationId, RelationData>, anyhow::Error> {
        let metadata_iter = self
            .keyspace
            .prefix([RELATION_METADATA_PREFIX])
            .map(|guard| {
                let (key, value) = guard.into_inner().context("Fjall error reading metadata")?;

                let key =
                    parse_relation_metadata_key(key).context("Failed to parse metadata key")?;
                let value = postcard::from_bytes::<RelationMetadataValue>(value.as_ref())
                    .context("Failed to parse metadata value")?;

                Ok::<_, anyhow::Error>((key, value))
            });

        let mut attributes_iter = self
            .keyspace
            .prefix([RELATION_ATTRIBUTE_PREFIX])
            .map(|guard| {
                let (key, value) = guard
                    .into_inner()
                    .context("Fjall error reading attribute")?;

                let key =
                    parse_relation_attribute_key(key).context("Failed to parse attribute key")?;
                let value = postcard::from_bytes::<RelationAttributeValue>(value.as_ref())
                    .context("Failed to parse attribute value")?;

                Ok::<_, anyhow::Error>((key, value))
            })
            .peekable();

        let mut relations = HashMap::new();

        for next in metadata_iter {
            let (relation, metadata) = next?;

            let mut attributes = HashMap::new();

            while attributes_iter.peek().is_some_and(|next| {
                let Ok(((attribute_relation, _attribute_kind), _attribute_value)) = next else {
                    // Error, enter loop to bail out
                    return true;
                };

                // Enter loop if this attribute is for the current relation
                *attribute_relation == relation
            }) {
                let ((_attribute_relation, attribute_kind), attribute_value) =
                    attributes_iter.next().expect("peek returned Some")?;

                attributes.insert(attribute_kind, attribute_value);
            }

            relations.insert(
                relation,
                RelationData {
                    metadata,
                    attributes,
                },
            );
        }

        Ok(relations)
    }

    pub fn scan_entity_metadata_by_kind(
        &self,
        entity: EntityKind,
    ) -> impl Iterator<Item = (EntityId, RawValue<EntityMetadataValue>)> + use<> {
        self.keyspace
            .prefix(make_entity_metadata_prefix_by_kind(entity))
            .filter_map(|guard| {
                let (key, value) = match guard.into_inner() {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!(?e, "Fjall error reading entity metadata");
                        return None;
                    }
                };

                let entity = match parse_entity_metadata_key(key) {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!(?e, "Failed to parse entity metadata key");
                        return None;
                    }
                };

                let value = RawValue::from_slice(value);

                Some((entity, value))
            })
    }

    pub fn scan_entity_attribute_by_kind(
        &self,
        entity: EntityKind,
    ) -> impl Iterator<Item = (EntityId, AttributeKind, RawValue<EntityAttributeValue>)> + use<>
    {
        self.keyspace
            .prefix(make_entity_attribute_prefix_by_kind(entity))
            .filter_map(|guard| {
                let (key, value) = match guard.into_inner() {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!(?e, "Fjall error reading entity attribute");
                        return None;
                    }
                };

                let (entity, attribute) = match parse_entity_attribute_key(key) {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!(?e, "Failed to parse entity attribute key");
                        return None;
                    }
                };

                let value = RawValue::from_slice(value);

                Some((entity, attribute, value))
            })
    }

    pub fn scan_entity_attribute_by_id(
        &self,
        entity: EntityId,
    ) -> impl Iterator<Item = (AttributeKind, RawValue<EntityAttributeValue>)> + use<> {
        self.keyspace
            .prefix(make_entity_attribute_prefix_by_id(entity))
            .filter_map(|guard| {
                let (key, value) = match guard.into_inner() {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!(?e, "Fjall error reading entity attribute");
                        return None;
                    }
                };

                let (_entity, attribute) = match parse_entity_attribute_key(key) {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!(?e, "Failed to parse entity attribute key");
                        return None;
                    }
                };

                let value = RawValue::from_slice(value);

                Some((attribute, value))
            })
    }

    pub fn scan_relation_attribute_by_id(
        &self,
        relation: RelationId,
    ) -> impl Iterator<Item = (AttributeKind, RawValue<RelationAttributeValue>)> + use<> {
        self.keyspace
            .prefix(make_relation_attribute_prefix_by_id(relation))
            .filter_map(|guard| {
                let (key, value) = match guard.into_inner() {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!(?e, "Fjall error reading relation attribute");
                        return None;
                    }
                };

                let (_relation, attribute) = match parse_relation_attribute_key(key) {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!(?e, "Failed to parse relation attribute key");
                        return None;
                    }
                };

                let value = RawValue::from_slice(value);

                Some((attribute, value))
            })
    }

    pub fn scan_relation_source_index_by_id_and_relation(
        &self,
        source: EntityId,
        kind: RelationKind,
    ) -> impl Iterator<Item = (RelationId, RawValue<RelationIndexValue>)> + use<> {
        self.keyspace
            .prefix(make_relation_source_prefix_by_source_and_kind(source, kind))
            .filter_map(|guard| {
                let (key, value) = match guard.into_inner() {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!(?e, "Fjall error reading relation index");
                        return None;
                    }
                };

                let (_entity, relation) = match parse_relation_source_key(key) {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!(?e, "Failed to parse relation index key");
                        return None;
                    }
                };

                let value = RawValue::from_slice(value);

                Some((relation, value))
            })
    }

    pub fn scan_relation_target_index_by_id_and_relation(
        &self,
        target: EntityId,
        kind: RelationKind,
    ) -> impl Iterator<Item = (RelationId, RawValue<RelationIndexValue>)> + use<> {
        self.keyspace
            .prefix(make_relation_target_prefix_by_target_and_kind(target, kind))
            .filter_map(|guard| {
                let (key, value) = match guard.into_inner() {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!(?e, "Fjall error reading relation index");
                        return None;
                    }
                };

                let (_entity, relation) = match parse_relation_target_key(key) {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!(?e, "Failed to parse relation index key");
                        return None;
                    }
                };

                let value = RawValue::from_slice(value);

                Some((relation, value))
            })
    }

    pub fn scan_relation_source_index_by_entity_kind(
        &self,
        entity_kind: EntityKind,
    ) -> impl Iterator<Item = (EntityId, RelationId, RawValue<RelationIndexValue>)> + use<> {
        self.keyspace
            .prefix(make_relation_source_prefix_by_entity_kind(entity_kind))
            .filter_map(|guard| {
                let (key, value) = match guard.into_inner() {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!(?e, "Fjall error reading relation index");
                        return None;
                    }
                };

                let (source, relation) = match parse_relation_source_key(key) {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!(?e, "Failed to parse relation index key");
                        return None;
                    }
                };

                let value = RawValue::from_slice(value);

                Some((source, relation, value))
            })
    }

    pub fn scan_relation_target_index_by_entity_kind(
        &self,
        entity_kind: EntityKind,
    ) -> impl Iterator<Item = (EntityId, RelationId, RawValue<RelationIndexValue>)> + use<> {
        self.keyspace
            .prefix(make_relation_target_prefix_by_entity_kind(entity_kind))
            .filter_map(|guard| {
                let (key, value) = match guard.into_inner() {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!(?e, "Fjall error reading relation index");
                        return None;
                    }
                };

                let (target, relation) = match parse_relation_target_key(key) {
                    Ok(x) => x,
                    Err(e) => {
                        tracing::error!(?e, "Failed to parse relation index key");
                        return None;
                    }
                };

                let value = RawValue::from_slice(value);

                Some((target, relation, value))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityData {
    pub metadata: EntityMetadataValue,
    pub attributes: HashMap<AttributeKind, EntityAttributeValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityMetadataValue {
    pub deleted: bool,
    pub deleted_version: Version,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityAttributeValue {
    pub value: Value,
    pub version: Version,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationData {
    pub metadata: RelationMetadataValue,
    pub attributes: HashMap<AttributeKind, RelationAttributeValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationMetadataValue {
    pub source: EntityId,
    pub target: EntityId,
    pub deleted: bool,
    pub deleted_version: Version,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationAttributeValue {
    pub value: Value,
    pub version: Version,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationIndexValue {
    pub other: EntityId,
    /// Whether the relation is deleted.
    pub deleted: bool,
}

const ENTITY_METADATA_PREFIX: u8 = 1u8;
const ENTITY_ATTRIBUTE_PREFIX: u8 = 2u8;
const RELATION_METADATA_PREFIX: u8 = 3u8;
const RELATION_ATTRIBUTE_PREFIX: u8 = 4u8;
const RELATION_SOURCE_PREFIX: u8 = 5u8;
const RELATION_TARGET_PREFIX: u8 = 6u8;

// prefix + entity ID
//
// EntityId starts with EntityKind so we can do range queries over kinds later
fn make_entity_metadata_key(entity: EntityId) -> [u8; 17] {
    let mut key = [0u8; 17];
    key[0] = ENTITY_METADATA_PREFIX;
    key[1..17].copy_from_slice(entity.as_slice());
    key
}

fn make_entity_metadata_prefix_by_kind(kind: EntityKind) -> [u8; 6] {
    let mut key = [0u8; 6];
    key[0] = ENTITY_METADATA_PREFIX;
    key[1..6].copy_from_slice(kind.as_slice());
    key
}

fn parse_entity_metadata_key(key: Slice) -> Result<EntityId, anyhow::Error> {
    if key.len() != 17 {
        anyhow::bail!("wrong key len");
    }
    let entity = EntityId::from_slice(key[1..17].try_into().unwrap());
    Ok(entity)
}

// prefix + entity ID + attribute ID
fn make_entity_attribute_key(entity: EntityId, attribute: AttributeKind) -> [u8; 22] {
    let mut key = [0u8; 22];
    key[0] = ENTITY_ATTRIBUTE_PREFIX;
    key[1..17].copy_from_slice(entity.as_slice());
    key[17..22].copy_from_slice(attribute.as_slice());
    key
}

fn make_entity_attribute_prefix_by_kind(kind: EntityKind) -> [u8; 6] {
    let mut key = [0u8; 6];
    key[0] = ENTITY_ATTRIBUTE_PREFIX;
    key[1..6].copy_from_slice(kind.as_slice());
    key
}

fn make_entity_attribute_prefix_by_id(id: EntityId) -> [u8; 17] {
    let mut key = [0u8; 17];
    key[0] = ENTITY_ATTRIBUTE_PREFIX;
    key[1..17].copy_from_slice(id.as_slice());
    key
}

fn parse_entity_attribute_key(key: Slice) -> Result<(EntityId, AttributeKind), anyhow::Error> {
    if key.len() != 22 {
        anyhow::bail!("wrong key len");
    }
    let entity = EntityId::from_slice(key[1..17].try_into().unwrap());
    let attribute = AttributeKind::from_bytes(key[17..22].try_into().unwrap());
    Ok((entity, attribute))
}

// prefix + relation ID
//
// RelationId starts with RelationKind so we can do range queries over kinds later
fn make_relation_metadata_key(relation: RelationId) -> [u8; 17] {
    let mut key = [0u8; 17];
    key[0] = RELATION_METADATA_PREFIX;
    key[1..17].copy_from_slice(relation.as_slice());
    key
}

fn parse_relation_metadata_key(key: Slice) -> Result<RelationId, anyhow::Error> {
    if key.len() != 17 {
        anyhow::bail!("wrong key len");
    }
    let relation = RelationId::from_slice(key[1..17].try_into().unwrap());
    Ok(relation)
}

// prefix + relation ID + attribute kind
fn make_relation_attribute_key(relation: RelationId, attribute: AttributeKind) -> [u8; 22] {
    let mut key = [0u8; 22];
    key[0] = RELATION_ATTRIBUTE_PREFIX;
    key[1..17].copy_from_slice(relation.as_slice());
    key[17..22].copy_from_slice(attribute.as_slice());
    key
}

fn make_relation_attribute_prefix_by_id(id: RelationId) -> [u8; 17] {
    let mut key = [0u8; 17];
    key[0] = RELATION_ATTRIBUTE_PREFIX;
    key[1..17].copy_from_slice(id.as_slice());
    key
}

fn parse_relation_attribute_key(key: Slice) -> Result<(RelationId, AttributeKind), anyhow::Error> {
    if key.len() != 22 {
        anyhow::bail!("wrong key len");
    }
    let relation = RelationId::from_slice(key[1..17].try_into().unwrap());
    let attribute = AttributeKind::from_bytes(key[17..22].try_into().unwrap());
    Ok((relation, attribute))
}

// prefix + source entity ID + relation ID
//
// Supports range query for all of an entity's outgoing relations, ordered by kind
fn make_relation_source_key(source: EntityId, relation: RelationId) -> [u8; 33] {
    let mut key = [0u8; 33];
    key[0] = RELATION_SOURCE_PREFIX;
    key[1..17].copy_from_slice(source.as_slice());
    key[17..33].copy_from_slice(relation.as_slice());
    key
}

fn make_relation_source_prefix_by_source_and_kind(
    source: EntityId,
    kind: RelationKind,
) -> [u8; 22] {
    let mut key = [0u8; 22];
    key[0] = RELATION_SOURCE_PREFIX;
    key[1..17].copy_from_slice(source.as_slice());
    key[17..22].copy_from_slice(kind.as_slice());
    key
}

fn make_relation_source_prefix_by_entity_kind(entity_kind: EntityKind) -> [u8; 6] {
    let mut key = [0u8; 6];
    key[0] = RELATION_SOURCE_PREFIX;
    key[1..6].copy_from_slice(&entity_kind.as_bytes());
    key
}

fn parse_relation_source_key(key: Slice) -> Result<(EntityId, RelationId), anyhow::Error> {
    if key.len() != 33 {
        anyhow::bail!("wrong key len");
    }
    let source = EntityId::from_slice(key[1..17].try_into().unwrap());
    let relation = RelationId::from_bytes(key[17..33].try_into().unwrap());
    Ok((source, relation))
}

// prefix + target entity ID + relation ID
//
// Supports range query for all of an entity's incoming relations, ordered by kind
fn make_relation_target_key(target: EntityId, relation: RelationId) -> [u8; 33] {
    let mut key = [0u8; 33];
    key[0] = RELATION_TARGET_PREFIX;
    key[1..17].copy_from_slice(target.as_slice());
    key[17..33].copy_from_slice(relation.as_slice());
    key
}

fn make_relation_target_prefix_by_target_and_kind(
    target: EntityId,
    kind: RelationKind,
) -> [u8; 22] {
    let mut key = [0u8; 22];
    key[0] = RELATION_TARGET_PREFIX;
    key[1..17].copy_from_slice(target.as_slice());
    key[17..22].copy_from_slice(kind.as_slice());
    key
}

fn make_relation_target_prefix_by_entity_kind(entity_kind: EntityKind) -> [u8; 6] {
    let mut key = [0u8; 6];
    key[0] = RELATION_TARGET_PREFIX;
    key[1..6].copy_from_slice(&entity_kind.as_bytes());
    key
}

fn parse_relation_target_key(key: Slice) -> Result<(EntityId, RelationId), anyhow::Error> {
    if key.len() != 33 {
        anyhow::bail!("wrong key len");
    }
    let target = EntityId::from_slice(key[1..17].try_into().unwrap());
    let relation = RelationId::from_bytes(key[17..33].try_into().unwrap());
    Ok((target, relation))
}

pub struct RawValue<T> {
    inner: Slice,
    phantom: PhantomData<T>,
}

impl<T: DeserializeOwned> RawValue<T> {
    fn from_slice(slice: Slice) -> Self {
        Self {
            inner: slice,
            phantom: PhantomData,
        }
    }

    // TODO: don't copy
    pub fn decode(&self) -> Result<T, anyhow::Error> {
        postcard::from_bytes::<T>(self.inner.as_ref()).context("Failed to parse value")
    }
}

#[cfg(test)]
mod test {
    use crate::{
        entity::{
            Version,
            hegel::{gen_attribute_kind, gen_entity_id, gen_relation_id, gen_value, gen_version},
        },
        store::{
            EntityAttributeValue, EntityMetadataValue, Store,
            hegel::{gen_entity_data, gen_relation_data},
            make_entity_attribute_key, make_entity_attribute_prefix_by_id,
            make_entity_attribute_prefix_by_kind, make_entity_metadata_key,
            make_entity_metadata_prefix_by_kind, make_relation_attribute_key,
            make_relation_attribute_prefix_by_id, make_relation_metadata_key,
            make_relation_source_key, make_relation_source_prefix_by_entity_kind,
            make_relation_source_prefix_by_source_and_kind, make_relation_target_key,
            make_relation_target_prefix_by_entity_kind,
            make_relation_target_prefix_by_target_and_kind, parse_entity_attribute_key,
            parse_entity_metadata_key, parse_relation_attribute_key, parse_relation_metadata_key,
            parse_relation_source_key, parse_relation_target_key,
        },
    };
    use hegel::{Generator, TestCase, generators as gs};
    use uuid::Uuid;

    #[hegel::test(test_cases = 10)]
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

    #[hegel::test(test_cases = 10)]
    fn retrieve_relations(tc: TestCase) {
        let store = Store::open(
            testdir::testdir!()
                .join(Uuid::new_v4().to_string())
                .join("store"),
        )
        .expect("should open");

        let relations = tc.draw(gs::hashmaps(gen_relation_id(), gen_relation_data()));

        for (relation, data) in relations.clone() {
            store
                .merge_relation_metadata(relation, data.metadata)
                .expect("should merge relation metadata");

            for (attribute, value) in data.attributes {
                store
                    .merge_relation_attribute(relation, attribute, value)
                    .expect("should merge relation attribute");
            }
        }

        let result = store.get_relations().expect("should get relations");
        assert_eq!(result, relations);
    }

    #[hegel::test(test_cases = 10)]
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

    #[hegel::test]
    fn entity_metadata_key(tc: TestCase) {
        let entity = tc.draw(gen_entity_id());

        let key = make_entity_metadata_key(entity);
        let parsed = parse_entity_metadata_key(key.into()).expect("should parse");

        assert_eq!(parsed, entity);

        let prefix = make_entity_metadata_prefix_by_kind(entity.kind());
        assert!(key.starts_with(&prefix));
    }

    #[hegel::test]
    fn entity_attribute_key(tc: TestCase) {
        let entity = tc.draw(gen_entity_id());
        let attribute = tc.draw(gen_attribute_kind());

        let key = make_entity_attribute_key(entity, attribute);
        let parsed = parse_entity_attribute_key(key.into()).expect("should parse");

        assert_eq!(parsed, (entity, attribute));

        let kind_prefix = make_entity_attribute_prefix_by_kind(entity.kind());
        assert!(key.starts_with(&kind_prefix));

        let id_prefix = make_entity_attribute_prefix_by_id(entity);
        assert!(key.starts_with(&id_prefix));
    }

    #[hegel::test]
    fn relation_metadata_key(tc: TestCase) {
        let relation = tc.draw(gen_relation_id());

        let key = make_relation_metadata_key(relation);
        let parsed = parse_relation_metadata_key(key.into()).expect("should parse");

        assert_eq!(parsed, relation);
    }

    #[hegel::test]
    fn relation_attribute_key(tc: TestCase) {
        let relation = tc.draw(gen_relation_id());
        let attribute = tc.draw(gen_attribute_kind());

        let key = make_relation_attribute_key(relation, attribute);
        let parsed = parse_relation_attribute_key(key.into()).expect("should parse");

        assert_eq!(parsed, (relation, attribute));

        let id_prefix = make_relation_attribute_prefix_by_id(relation);
        assert!(key.starts_with(&id_prefix));
    }

    #[hegel::test]
    fn relation_source_key(tc: TestCase) {
        let source = tc.draw(gen_entity_id());
        let relation = tc.draw(gen_relation_id());

        let key = make_relation_source_key(source, relation);
        let parsed = parse_relation_source_key(key.into()).expect("should parse");

        assert_eq!(parsed, (source, relation));

        let prefix = make_relation_source_prefix_by_source_and_kind(source, relation.kind());
        assert!(key.starts_with(&prefix));

        let entity_kind_prefix = make_relation_source_prefix_by_entity_kind(source.kind());
        assert!(key.starts_with(&entity_kind_prefix));
    }

    #[hegel::test]
    fn relation_target_key(tc: TestCase) {
        let target = tc.draw(gen_entity_id());
        let relation = tc.draw(gen_relation_id());

        let key = make_relation_target_key(target, relation);
        let parsed = parse_relation_target_key(key.into()).expect("should parse");

        assert_eq!(parsed, (target, relation));

        let prefix = make_relation_target_prefix_by_target_and_kind(target, relation.kind());
        assert!(key.starts_with(&prefix));

        let entity_kind_prefix = make_relation_target_prefix_by_entity_kind(target.kind());
        assert!(key.starts_with(&entity_kind_prefix));
    }
}

pub mod hegel {
    use crate::{
        entity::hegel::{gen_attribute_kind, gen_entity_id, gen_value, gen_version},
        store::{
            EntityAttributeValue, EntityData, EntityMetadataValue, RelationAttributeValue,
            RelationData, RelationMetadataValue,
        },
    };
    use hegel::{TestCase, compose, generators as gs};

    #[hegel::composite]
    pub fn gen_entity_data(tc: TestCase) -> EntityData {
        EntityData {
            metadata: EntityMetadataValue {
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

    #[hegel::composite]
    pub fn gen_relation_data(tc: TestCase) -> RelationData {
        RelationData {
            metadata: RelationMetadataValue {
                source: tc.draw(gen_entity_id()),
                target: tc.draw(gen_entity_id()),
                deleted: tc.draw(gs::booleans()),
                deleted_version: tc.draw(gen_version()),
            },
            attributes: tc.draw(gs::hashmaps(
                gen_attribute_kind(),
                compose!(|tc| {
                    RelationAttributeValue {
                        value: tc.draw(gen_value()),
                        version: tc.draw(gen_version()),
                    }
                }),
            )),
        }
    }
}
