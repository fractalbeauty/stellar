use std::{collections::HashMap, sync::Arc};
use stellar_graph::{
    database::Database,
    entity::{AttributeKind, EntityId, RelationId, RelationKind, Value},
};

pub trait ImportDatabasePort: Send + Sync {
    fn find_entity(
        &self,
        kind: AttributeKind,
        attributes: HashMap<AttributeKind, Value>,
    ) -> Option<EntityId>;

    fn find_relation(
        &self,
        kind: RelationKind,
        source: EntityId,
        target: EntityId,
        attributes: HashMap<AttributeKind, Value>,
    ) -> Option<RelationId>;
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
    fn find_entity(
        &self,
        kind: AttributeKind,
        attributes: HashMap<AttributeKind, Value>,
    ) -> Option<EntityId> {
        todo!()
    }

    fn find_relation(
        &self,
        kind: RelationKind,
        source: EntityId,
        target: EntityId,
        attributes: HashMap<AttributeKind, Value>,
    ) -> Option<RelationId> {
        todo!()
    }
}
