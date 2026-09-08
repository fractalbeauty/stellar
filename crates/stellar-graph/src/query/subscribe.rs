use crate::{
    entity::{AttributeKind, EntityKind, RelationKind},
    query::{exec::SlotValue, plan::TableQuery},
    store::{Store, StoreChange},
};
use std::{
    collections::HashSet,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// How long to wait before rerunning an invalidated query.
const DEBOUNCE: Duration = Duration::from_millis(50);

/// Waits for a pending debounce timeout to finish, or waits forever if there is none pending.
async fn debounce_timeout(timeout: &mut Pin<&mut Option<tokio::time::Sleep>>) {
    match timeout.as_mut().as_pin_mut() {
        Some(sleep) => sleep.await,
        None => std::future::pending().await,
    }
}

/// Foreign trait for receiving change notifications for a subscribed [`TableQuery`].
#[uniffi::export(with_foreign)]
pub trait TableQueryChangeHandler: Send + Sync {
    /// Called when the results may have changed.
    ///
    /// Results should be retrieved using [`TableQuerySubscription::rows`].
    fn on_change(&self);
}

/// Which store entries a [`TableQuery`] reads, to determine whether a change invalidates a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Dependency {
    EntityMetadata(EntityKind),
    EntityAttribute(AttributeKind),
    RelationIndex(RelationKind),
    RelationAttribute(AttributeKind),
}

impl Dependency {
    fn matches(&self, change: &StoreChange) -> bool {
        match (self, change) {
            (
                Dependency::EntityMetadata(dependency_kind),
                StoreChange::EntityMetadata {
                    entity: change_kind,
                    ..
                },
            ) => *dependency_kind == change_kind.kind(),
            (
                Dependency::EntityAttribute(dependency_attribute),
                StoreChange::EntityAttribute {
                    attribute: change_attribute,
                    ..
                },
            ) => dependency_attribute == change_attribute,
            (
                Dependency::RelationIndex(dependency_relation),
                StoreChange::RelationMetadata {
                    relation: change_relation,
                    ..
                },
            ) => *dependency_relation == change_relation.kind(),
            (
                Dependency::RelationAttribute(dependency_attribute),
                StoreChange::RelationAttribute {
                    attribute: change_attribute,
                    ..
                },
            ) => dependency_attribute == change_attribute,

            _ => false,
        }
    }
}

/// Derives the [`Dependency`]s for a [`TableQuery`].
fn dependencies_for_table_query(query: &TableQuery) -> HashSet<Dependency> {
    let mut dependencies = HashSet::new();

    // Always requires scanning entity metadata for the queried entity kind
    dependencies.insert(Dependency::EntityMetadata(query.entity));

    // Querying attributes requires scanning entity attributes
    for &attribute in query.attributes.keys() {
        dependencies.insert(Dependency::EntityAttribute(attribute));
    }

    // Querying any relation information requires scanning the relation indexes
    let relations = query
        .outgoing_relation_attributes
        .keys()
        .chain(query.outgoing_relation_entity_attributes.keys())
        .chain(query.outgoing_relation_others.keys())
        .chain(query.incoming_relation_attributes.keys())
        .chain(query.incoming_relation_entity_attributes.keys())
        .chain(query.incoming_relation_others.keys());
    for &relation in relations {
        dependencies.insert(Dependency::RelationIndex(relation));
    }

    // Querying relation attributes requires scanning relation attributes
    let relation_attributes = query
        .outgoing_relation_attributes
        .values()
        .chain(query.incoming_relation_attributes.values());
    for attributes in relation_attributes {
        for &attribute in attributes.keys() {
            dependencies.insert(Dependency::RelationAttribute(attribute));
        }
    }

    // Querying relation entity attributes requires scanning entity attributes
    let relation_entity_attributes = query
        .outgoing_relation_entity_attributes
        .values()
        .chain(query.incoming_relation_entity_attributes.values());
    for attributes in relation_entity_attributes {
        for &attribute in attributes.keys() {
            dependencies.insert(Dependency::EntityAttribute(attribute));
        }
    }

    dependencies
}

/// Handle to a live subscription for a [`TableQuery`].
#[derive(Debug, Clone)]
pub struct TableQuerySubscription {
    cancellation_token: CancellationToken,
    rows: Arc<Mutex<Vec<Vec<Option<SlotValue>>>>>,
}

impl TableQuerySubscription {
    /// Spawns the subscription.
    ///
    /// The initial query runs synchronously on the calling thread so results are available immediately.
    pub fn spawn(
        store: Store,
        query: TableQuery,
        handler: Arc<dyn TableQueryChangeHandler>,
    ) -> Self {
        let cancellation_token = CancellationToken::new();

        let dependencies = dependencies_for_table_query(&query);

        // Subscribe before running the initial query so concurrent changes are observed
        let mut local_changes = store.subscribe_local();
        let mut remote_changes = store.subscribe_remote();
        let rows = Arc::new(Mutex::new(query.execute(store.clone())));

        tokio::spawn({
            let cancellation_token = cancellation_token.clone();
            let rows = rows.clone();
            async move {
                let timeout = None;
                tokio::pin!(timeout);

                loop {
                    tokio::select! {
                        _ = cancellation_token.cancelled() => break,

                        change = local_changes.recv() => {
                            let invalidated = match change {
                                Ok(change) => dependencies.iter().any(|dependency| dependency.matches(&change)),
                                Err(broadcast::error::RecvError::Lagged(_)) => true,
                                Err(broadcast::error::RecvError::Closed) => break,
                            };

                            if invalidated && timeout.as_mut().as_pin_mut().is_none() {
                                timeout.set(Some(tokio::time::sleep(DEBOUNCE)));
                            }
                        }

                        change = remote_changes.recv() => {
                            let invalidated = match change {
                                Ok(change) => dependencies.iter().any(|dependency| dependency.matches(&change)),
                                Err(broadcast::error::RecvError::Lagged(_)) => true,
                                Err(broadcast::error::RecvError::Closed) => break,
                            };

                            if invalidated && timeout.as_mut().as_pin_mut().is_none() {
                                timeout.set(Some(tokio::time::sleep(DEBOUNCE)));
                            }
                        }

                        _ = debounce_timeout(&mut timeout) => {
                            *rows.lock().unwrap() = query.execute(store.clone());
                            timeout.set(None);
                            handler.on_change();
                        }
                    }
                }

                tracing::debug!("Table query subscription cancelled");
            }
        });

        Self {
            cancellation_token,
            rows,
        }
    }

    /// Returns the query's latest results.
    pub fn rows(&self) -> Vec<Vec<Option<SlotValue>>> {
        self.rows.lock().unwrap().clone()
    }

    /// Cancels the subscription.
    ///
    /// This must be called when the subscriber is done with it, or the subscription will leak.
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        entity::{AttributeKind, AuthorId, EntityId, EntityKind, Timestamp, Value, Version},
        query::plan::OutputIndex,
        store::{EntityAttributeValue, EntityMetadataValue},
    };
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };
    use tokio::sync::Notify;

    struct TestHandler {
        calls: Mutex<u32>,
        notify: Notify,
    }

    impl TestHandler {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                calls: Mutex::new(0),
                notify: Notify::new(),
            })
        }

        /// Waits until `on_change` has been called at least `n` times.
        async fn wait_for_calls(self: &Arc<Self>, n: u32) {
            loop {
                if *self.calls.lock().unwrap() >= n {
                    return;
                }
                self.notify.notified().await;
            }
        }
    }

    impl TableQueryChangeHandler for TestHandler {
        fn on_change(&self) {
            *self.calls.lock().unwrap() += 1;
            self.notify.notify_waiters();
        }
    }

    fn version(millis: u64) -> Version {
        Version::new(Timestamp::new(millis), AuthorId::from_bytes([0u8; 32]))
    }

    #[tokio::test]
    async fn requeries_on_matching_change() {
        let store = Store::in_memory();

        let entity_kind = EntityKind::random();
        let attribute = AttributeKind::random();

        let entity = EntityId::random(entity_kind);

        store
            .apply_local_entity_metadata(
                entity,
                EntityMetadataValue {
                    deleted: false,
                    deleted_version: version(0),
                },
            )
            .expect("should update");
        store
            .apply_local_entity_attribute(
                entity,
                attribute,
                EntityAttributeValue {
                    value: Value::Text("first".to_string()),
                    version: version(1),
                },
            )
            .expect("should update");

        let query = TableQuery {
            entity: entity_kind,
            id: None,
            attributes: HashMap::from([(attribute, OutputIndex(0))]),
            outgoing_relation_attributes: HashMap::new(),
            outgoing_relation_entity_attributes: HashMap::new(),
            outgoing_relation_others: HashMap::new(),
            incoming_relation_attributes: HashMap::new(),
            incoming_relation_entity_attributes: HashMap::new(),
            incoming_relation_others: HashMap::new(),
            sort: None,
        };

        let handler = TestHandler::new();
        let subscription = TableQuerySubscription::spawn(store.clone(), query, handler.clone());

        assert_eq!(
            subscription.rows(),
            vec![vec![Some(SlotValue::SVValue(Value::Text(
                "first".to_string()
            )))]]
        );

        store
            .apply_local_entity_attribute(
                entity,
                attribute,
                EntityAttributeValue {
                    value: Value::Text("second".to_string()),
                    version: version(2),
                },
            )
            .expect("should update");

        handler.wait_for_calls(1).await;
        assert_eq!(
            subscription.rows(),
            vec![vec![Some(SlotValue::SVValue(Value::Text(
                "second".to_string()
            )))]]
        );

        subscription.cancel();
    }

    #[tokio::test]
    async fn ignores_unrelated_change() {
        let store = Store::in_memory();

        let entity_kind = EntityKind::random();
        let other_kind = EntityKind::random();
        let attribute = AttributeKind::random();

        let query = TableQuery {
            entity: entity_kind,
            id: None,
            attributes: HashMap::from([(attribute, OutputIndex(0))]),
            outgoing_relation_attributes: HashMap::new(),
            outgoing_relation_entity_attributes: HashMap::new(),
            outgoing_relation_others: HashMap::new(),
            incoming_relation_attributes: HashMap::new(),
            incoming_relation_entity_attributes: HashMap::new(),
            incoming_relation_others: HashMap::new(),
            sort: None,
        };

        let handler = TestHandler::new();
        let subscription = TableQuerySubscription::spawn(store.clone(), query, handler.clone());

        // A change happens for a different entity kind
        let other_entity = EntityId::random(other_kind);
        store
            .apply_local_entity_metadata(
                other_entity,
                EntityMetadataValue {
                    deleted: false,
                    deleted_version: version(0),
                },
            )
            .expect("should update");

        // Should not rerun the query. Wait 100ms to check that it wasn't called
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(*handler.calls.lock().unwrap(), 0);

        subscription.cancel();
    }
}
