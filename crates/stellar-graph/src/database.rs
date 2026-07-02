use crate::{
    entity::{EntityId, EntityKind, Version},
    store::{EntityData, EntityMetadataValue, Store},
};
use std::{collections::HashMap, path::Path};

pub struct Database {
    store: Store,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
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
        self.store.set_entity_metadata(entity, value)?;

        Ok(entity)
    }

    pub fn get_entities(&self) -> Result<HashMap<EntityId, EntityData>, anyhow::Error> {
        self.store.get_entities()
    }
}
