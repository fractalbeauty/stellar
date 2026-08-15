use crate::{
    entity::{AttributeKind, EntityId, EntityKind, RelationId, RelationKind, Value, Version},
    store::{
        EntityAttributeValue, EntityData, EntityMetadataValue, RelationAttributeValue,
        RelationData, RelationMetadataValue, Store,
    },
};
use std::{collections::HashMap, path::Path};

/// Handle to the database for graph data. Provides higher-level operations than the store.
#[derive(Clone)]
pub struct Database {
    store: Store,
}

impl Database {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let path = dir.as_ref().join("store");
        let store = Store::open(path)?;

        Ok(Self { store })
    }

    pub fn get_entities(&self) -> Result<HashMap<EntityId, EntityData>, anyhow::Error> {
        self.store.get_entities()
    }

    pub fn create_entity(
        &self,
        kind: EntityKind,
        version: Version,
    ) -> Result<EntityId, anyhow::Error> {
        let entity = EntityId::random(kind);

        let value = EntityMetadataValue {
            deleted: false,
            deleted_version: version,
        };
        self.store.merge_entity_metadata(entity, value)?;

        Ok(entity)
    }

    pub fn upsert_entity(&self, entity: EntityId, data: EntityData) -> Result<(), anyhow::Error> {
        self.store.merge_entity_metadata(entity, data.metadata)?;
        for (attribute, value) in data.attributes {
            self.store
                .merge_entity_attribute(entity, attribute, value)?;
        }
        Ok(())
    }

    pub fn set_entity_attribute(
        &self,
        entity: EntityId,
        attribute: AttributeKind,
        value: Value,
        version: Version,
    ) -> Result<(), anyhow::Error> {
        self.store.merge_entity_attribute(
            entity,
            attribute,
            EntityAttributeValue { value, version },
        )?;
        Ok(())
    }

    pub fn delete_entity(&self, entity: EntityId, version: Version) -> Result<(), anyhow::Error> {
        self.store.merge_entity_metadata(
            entity,
            EntityMetadataValue {
                deleted: true,
                deleted_version: version,
            },
        )?;
        Ok(())
    }

    pub fn get_relations(&self) -> Result<HashMap<RelationId, RelationData>, anyhow::Error> {
        self.store.get_relations()
    }

    pub fn create_relation(
        &self,
        kind: RelationKind,
        source: EntityId,
        target: EntityId,
        version: Version,
    ) -> Result<RelationId, anyhow::Error> {
        let relation = RelationId::random(kind);

        let value = RelationMetadataValue {
            source,
            target,
            deleted: false,
            deleted_version: version,
        };
        self.store.merge_relation_metadata(relation, value)?;

        Ok(relation)
    }

    pub fn upsert_relation(
        &self,
        relation: RelationId,
        data: RelationData,
    ) -> Result<(), anyhow::Error> {
        self.store
            .merge_relation_metadata(relation, data.metadata)?;
        for (attribute, value) in data.attributes {
            self.store
                .merge_relation_attribute(relation, attribute, value)?;
        }
        Ok(())
    }

    pub fn set_relation_attribute(
        &self,
        relation: RelationId,
        attribute: AttributeKind,
        value: Value,
        version: Version,
    ) -> Result<(), anyhow::Error> {
        self.store.merge_relation_attribute(
            relation,
            attribute,
            RelationAttributeValue { value, version },
        )?;
        Ok(())
    }

    pub fn delete_relation(
        &self,
        relation: RelationId,
        version: Version,
    ) -> Result<(), anyhow::Error> {
        let Some(existing) = self.store.get_relation_metadata(relation)? else {
            anyhow::bail!("Relation does not exist");
        };
        self.store.merge_relation_metadata(
            relation,
            RelationMetadataValue {
                source: existing.source,
                target: existing.target,
                deleted: true,
                deleted_version: version,
            },
        )?;
        Ok(())
    }
}
