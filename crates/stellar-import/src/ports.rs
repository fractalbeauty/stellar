use crate::{evaluator::Changes, rules::Rules};
use std::{collections::HashMap, sync::Arc};
use stellar_graph::{
    database::Database,
    entity::{AuthorId, EntityId, EntityKind, Timestamp, Version},
    schema::GraphSchema,
    store::{
        EntityAttributeValue, EntityData, EntityMetadataValue, RelationAttributeValue,
        RelationData, RelationMetadataValue,
    },
};
use tokio::sync::watch;

pub trait ImportDatabasePort: Send + Sync {
    fn get_entities_by_kind(
        &self,
        kind: EntityKind,
    ) -> Result<HashMap<EntityId, EntityData>, anyhow::Error>;

    fn apply_changes(&self, changes: Changes, author: AuthorId) -> Result<(), anyhow::Error>;
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

    fn apply_changes(&self, changes: Changes, author: AuthorId) -> Result<(), anyhow::Error> {
        let version = Version::new(Timestamp::now(), author);

        for (_, created_entities) in changes.create_entities {
            for change in created_entities {
                self.database.upsert_entity(
                    change.id,
                    EntityData {
                        metadata: EntityMetadataValue {
                            deleted: false,
                            deleted_version: version,
                        },
                        attributes: change
                            .attributes
                            .into_iter()
                            .map(|(attribute, value)| {
                                (attribute, EntityAttributeValue { value, version })
                            })
                            .collect(),
                    },
                )?; // TODO 
            }
        }

        for (_, created_relations) in changes.create_relations {
            for change in created_relations {
                self.database.upsert_relation(
                    change.id,
                    RelationData {
                        metadata: RelationMetadataValue {
                            source: change.source,
                            target: change.target,
                            deleted: false,
                            deleted_version: version,
                        },
                        attributes: change
                            .attributes
                            .into_iter()
                            .map(|(attribute, value)| {
                                (attribute, RelationAttributeValue { value, version })
                            })
                            .collect(),
                    },
                )?; // TODO 
            }
        }

        Ok(())
    }
}

pub trait ImportSchemaPort: Send + Sync {
    fn watch_schema(&self) -> watch::Receiver<Option<(GraphSchema, Rules)>>;
}
