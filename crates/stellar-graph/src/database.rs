use crate::{
    entity::{EntityId, EntityKind, Version},
    store::{EntityData, EntityMetadataValue, Store},
};
use std::{collections::HashMap, path::Path};

/// Handle to the database for graph data. Provides higher-level operations than the store.
pub struct Database {
    store: Store,
}

impl Database {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let path = dir.as_ref().join("store");
        let store = Store::open(path)?;

        Ok(Self { store })
    }

    pub fn create_entity(
        &self,
        kind: EntityKind,
        version: Version,
    ) -> Result<EntityId, anyhow::Error> {
        let entity = EntityId::random();

        let value = EntityMetadataValue {
            kind,
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

    pub fn get_entities(&self) -> Result<HashMap<EntityId, EntityData>, anyhow::Error> {
        self.store.get_entities()
    }
}
