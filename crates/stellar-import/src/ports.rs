use std::{collections::HashMap, sync::Arc};
use stellar_graph::{
    database::Database,
    entity::{AttributeKind, EntityId, EntityKind, RelationId, RelationKind, Value},
    store::EntityData,
};

pub trait ImportDatabasePort: Send + Sync {
    fn get_entities_by_kind(
        &self,
        kind: EntityKind,
    ) -> Result<HashMap<EntityId, EntityData>, anyhow::Error>;
}

pub struct ImportDatabaseAdapter {
    database: Database,
}

impl ImportDatabaseAdapter {
    pub fn new(database: Database) -> Arc<Self> {
        Arc::new(Self { database })
    }
}

impl ImportDatabasePort for ImportDatabaseAdapter {
    fn get_entities_by_kind(
        &self,
        kind: EntityKind,
    ) -> Result<HashMap<EntityId, EntityData>, anyhow::Error> {
        // TODO: optimize
        let entities = self.database.get_entities()?;
        Ok(entities
            .into_iter()
            .filter(|(id, _)| id.kind() == kind)
            .collect())
    }
}
