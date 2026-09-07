use crate::{
    entity::{AttributeKind, EntityId, EntityKind, RelationId, RelationKind, Value, Version},
    query::{exec::SlotValue, plan::TableQuery},
    store::{
        EntityAttributeValue, EntityData, EntityMetadataValue, RelationAttributeValue,
        RelationData, RelationMetadataValue, Store, StoreChange,
    },
};
use std::{collections::HashMap, path::Path};
use tokio::sync::broadcast;

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

    pub fn in_memory() -> Self {
        Self {
            store: Store::in_memory(),
        }
    }

    pub fn table_query(
        &self,
        query: &TableQuery,
    ) -> Result<Vec<Vec<Option<SlotValue>>>, anyhow::Error> {
        Ok(query.execute(self.store.clone()))
    }

    /// Subscribes to local changes. Remote changes are not re-broadcast.
    pub fn subscribe(&self) -> broadcast::Receiver<StoreChange> {
        self.store.subscribe()
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
        self.store.apply_local_entity_metadata(entity, value)?;

        Ok(entity)
    }

    /// Applies local entity changes.
    pub fn apply_local_entity(
        &self,
        entity: EntityId,
        data: EntityData,
    ) -> Result<(), anyhow::Error> {
        self.store
            .apply_local_entity_metadata(entity, data.metadata)?;
        for (attribute, value) in data.attributes {
            self.store
                .apply_local_entity_attribute(entity, attribute, value)?;
        }
        Ok(())
    }

    /// Applies remote entity changes.
    pub fn apply_remote_entity(
        &self,
        entity: EntityId,
        data: EntityData,
    ) -> Result<(), anyhow::Error> {
        self.store
            .apply_remote_entity_metadata(entity, data.metadata)?;
        for (attribute, value) in data.attributes {
            self.store
                .apply_remote_entity_attribute(entity, attribute, value)?;
        }
        Ok(())
    }

    /// Applies remote entity metadata changes.
    pub fn apply_remote_entity_metadata(
        &self,
        entity: EntityId,
        value: EntityMetadataValue,
    ) -> Result<(), anyhow::Error> {
        self.store.apply_remote_entity_metadata(entity, value)
    }

    /// Applies remote entity metadata changes.
    pub fn apply_remote_entity_attribute(
        &self,
        entity: EntityId,
        attribute: AttributeKind,
        value: EntityAttributeValue,
    ) -> Result<(), anyhow::Error> {
        self.store
            .apply_remote_entity_attribute(entity, attribute, value)
    }

    pub fn set_entity_attribute(
        &self,
        entity: EntityId,
        attribute: AttributeKind,
        value: Value,
        version: Version,
    ) -> Result<(), anyhow::Error> {
        self.store.apply_local_entity_attribute(
            entity,
            attribute,
            EntityAttributeValue { value, version },
        )?;
        Ok(())
    }

    pub fn delete_entity(&self, entity: EntityId, version: Version) -> Result<(), anyhow::Error> {
        self.store.apply_local_entity_metadata(
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
        self.store.apply_local_relation_metadata(relation, value)?;

        Ok(relation)
    }

    /// Applies local relation changes.
    pub fn apply_local_relation(
        &self,
        relation: RelationId,
        data: RelationData,
    ) -> Result<(), anyhow::Error> {
        self.store
            .apply_local_relation_metadata(relation, data.metadata)?;
        for (attribute, value) in data.attributes {
            self.store
                .apply_local_relation_attribute(relation, attribute, value)?;
        }
        Ok(())
    }

    /// Applies remote relation changes.
    pub fn apply_remote_relation(
        &self,
        relation: RelationId,
        data: RelationData,
    ) -> Result<(), anyhow::Error> {
        self.store
            .apply_remote_relation_metadata(relation, data.metadata)?;
        for (attribute, value) in data.attributes {
            self.store
                .apply_remote_relation_attribute(relation, attribute, value)?;
        }
        Ok(())
    }

    /// Applies remote relation metadata changes.
    pub fn apply_remote_relation_metadata(
        &self,
        relation: RelationId,
        value: RelationMetadataValue,
    ) -> Result<(), anyhow::Error> {
        self.store.apply_remote_relation_metadata(relation, value)
    }

    /// Applies remote relation attribute changes.
    pub fn apply_remote_relation_attribute(
        &self,
        relation: RelationId,
        attribute: AttributeKind,
        value: RelationAttributeValue,
    ) -> Result<(), anyhow::Error> {
        self.store
            .apply_remote_relation_attribute(relation, attribute, value)
    }

    pub fn set_relation_attribute(
        &self,
        relation: RelationId,
        attribute: AttributeKind,
        value: Value,
        version: Version,
    ) -> Result<(), anyhow::Error> {
        self.store.apply_local_relation_attribute(
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
        self.store.apply_local_relation_metadata(
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
