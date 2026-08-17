use crate::error::{CoreError, core_error};
use anyhow::Context;
use directories_next::ProjectDirs;
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;
use std::unimplemented;
use stellar_graph::database::Database;
use stellar_graph::entity::{
    AttributeKind, AuthorId, EntityId, EntityKind, RelationId, RelationKind, Timestamp, Value,
    ValueKind, Version,
};
use stellar_graph::schema::{AttributeSchema, EntitySchema, RelationSchema, Schema};
use stellar_import::import::{ImportEventHandler, ImportTask};
use stellar_log::LogGuard;
use stellar_sync::devices::DevicesState;
use stellar_sync::peers::{PeersDatabaseAdapter, PeersSchemaAdapter, PeersTask};
use stellar_sync::schema::SchemaStoreTask;
use stellar_sync::{EndpointId, SecretKey, devices::DevicesTask};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};
use uuid::Uuid;

#[derive(uniffi::Object)]
pub struct Core {
    runtime_handle: tokio::runtime::Handle,

    cancellation_token: CancellationToken,
    database: Database,
    schema: SchemaStoreTask,
    peers: PeersTask,
    devices: DevicesTask,

    endpoint_id: EndpointId,
    author: AuthorId,

    devices_change_handler: Arc<dyn DevicesChangeHandler>,
    schema_change_handler: Arc<dyn SchemaChangeHandler>,

    #[allow(unused)]
    log_guard: Option<LogGuard>,
}

#[uniffi::export]
impl Core {
    #[uniffi::constructor]
    pub async fn spawn(
        profile: String,
        devices_change_handler: Arc<dyn DevicesChangeHandler>,
        schema_change_handler: Arc<dyn SchemaChangeHandler>,
    ) -> Result<Arc<Self>, CoreError> {
        let log_guard = stellar_log::init(None)?;

        let (core_tx, core_rx) = oneshot::channel();

        std::thread::spawn({
            move || {
                let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    run_core_thread(
                        profile,
                        log_guard,
                        core_tx,
                        devices_change_handler,
                        schema_change_handler,
                    )
                }));

                match result {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => {
                        error!("Core thread exited with error: {error:?}");
                    }
                    Err(error) => {
                        error!("Panic in core thread: {error:?}");

                        if let Some(string) = error.downcast_ref::<std::string::String>() {
                            error!("Panic info: {string}");
                        }
                        if let Some(str) = error.downcast_ref::<&'static str>() {
                            error!("Panic info: {str}");
                        }
                    }
                }

                debug!("Core thread exited");
            }
        });

        let core = async_std::future::timeout(Duration::from_secs(10), core_rx)
            .await
            .map_err(|_elapsed| core_error!("Timed out waiting for core to initialize"))?
            .map_err(|_dropped| core_error!("Core failed to initialize, sender dropped"))?;

        Ok(Arc::new(core))
    }

    pub async fn cancel(&self) {
        debug!("Cancelling core");
        self.cancellation_token.cancel();
    }

    pub fn endpoint_id(&self) -> EndpointId {
        self.endpoint_id
    }

    pub async fn start_device_code_flow(&self) -> Result<String, CoreError> {
        let rx = self.devices.start_device_code_flow()?;

        let verification_uri_complete = async_std::future::timeout(Duration::from_secs(10), rx)
            .await
            .map_err(|_elapsed| core_error!("Timed out waiting for device code flow to start"))?
            .map_err(|_dropped| core_error!("Device code flow failed to start, sender dropped"))?;

        Ok(verification_uri_complete)
    }

    pub fn revoke_auth_session(&self, session: Uuid) -> Result<(), CoreError> {
        self.devices.revoke_auth_session(session)?;
        Ok(())
    }

    pub fn add_device(&self, endpoint_id: String, name: Option<String>) -> Result<(), CoreError> {
        let endpoint_id = endpoint_id
            .parse::<EndpointId>()
            .map_err(|_| core_error!("Failed to parse endpoint ID"))?;

        self.devices.add_device(endpoint_id, name)?;

        Ok(())
    }

    /// Gets all non-deleted entities.
    pub fn get_entities(&self) -> Result<HashMap<EntityId, CoreEntity>, CoreError> {
        let entities = self.database.get_entities()?;
        Ok(entities
            .into_iter()
            .filter_map(|(entity, data)| {
                if data.metadata.deleted {
                    return None;
                }

                Some((
                    entity,
                    CoreEntity {
                        // TODO
                        kind: entity.kind(),
                        attributes: data
                            .attributes
                            .into_iter()
                            .map(|(attribute, value)| {
                                (attribute, CoreAttribute { value: value.value })
                            })
                            .collect(),
                    },
                ))
            })
            .collect())
    }

    /// Creates an entity of the given kind, returning its ID.
    pub fn create_entity(&self, kind: EntityKind) -> Result<EntityId, CoreError> {
        let entity = self.database.create_entity(kind, self.version_now())?;
        Ok(entity)
    }

    /// Sets an entity's attribute for an entity to a value.
    pub fn set_entity_attribute(
        &self,
        entity: EntityId,
        attribute: AttributeKind,
        value: Value,
    ) -> Result<(), CoreError> {
        self.database
            .set_entity_attribute(entity, attribute, value, self.version_now())?;
        Ok(())
    }

    /// Deletes an entity.
    pub fn delete_entity(&self, entity: EntityId) -> Result<(), CoreError> {
        self.database.delete_entity(entity, self.version_now())?;
        Ok(())
    }

    /// Gets all non-deleted relations.
    pub fn get_relations(&self) -> Result<HashMap<RelationId, CoreRelation>, CoreError> {
        let relations = self.database.get_relations()?;
        Ok(relations
            .into_iter()
            .filter_map(|(relation, data)| {
                if data.metadata.deleted {
                    return None;
                }

                Some((
                    relation,
                    CoreRelation {
                        // TODO
                        kind: relation.kind(),
                        source: data.metadata.source,
                        target: data.metadata.target,
                        attributes: data
                            .attributes
                            .into_iter()
                            .map(|(attribute, value)| {
                                (attribute, CoreAttribute { value: value.value })
                            })
                            .collect(),
                    },
                ))
            })
            .collect())
    }

    /// Creates a relation of the given kind, returning its ID.
    pub fn create_relation(
        &self,
        kind: RelationKind,
        source: EntityId,
        target: EntityId,
    ) -> Result<RelationId, CoreError> {
        let relation = self
            .database
            .create_relation(kind, source, target, self.version_now())?;
        Ok(relation)
    }

    /// Sets a relation's attribute for a relation to a value.
    pub fn set_relation_attribute(
        &self,
        relation: RelationId,
        attribute: AttributeKind,
        value: Value,
    ) -> Result<(), CoreError> {
        self.database
            .set_relation_attribute(relation, attribute, value, self.version_now())?;
        Ok(())
    }

    /// Deletes a relation.
    pub fn delete_relation(&self, relation: RelationId) -> Result<(), CoreError> {
        self.database
            .delete_relation(relation, self.version_now())?;
        Ok(())
    }

    /// Creates an entity in the schema, returning its ID.
    pub async fn create_schema_entity(&self, name: String) -> Result<EntityKind, CoreError> {
        let (schema, entity_kind) = self
            .schema
            .modify(move |schema| -> Result<_, anyhow::Error> {
                let entity_kind = EntityKind::random();
                schema.entities.insert(
                    entity_kind,
                    EntitySchema {
                        name,
                        attributes: HashMap::new(),
                    },
                );

                Ok((schema.clone(), entity_kind))
            })
            .await?
            .context("Failed to modify schema")?;
        self.schema_change_handler.on_change(schema);
        Ok(entity_kind)
    }

    /// Deletes an entity in the schema.
    pub async fn delete_schema_entity(&self, entity: EntityKind) -> Result<(), CoreError> {
        let schema = self
            .schema
            .modify(move |schema| {
                let removed = schema.entities.remove(&entity);
                if removed.is_none() {
                    anyhow::bail!("Entity kind does not exist");
                }

                Ok(schema.clone())
            })
            .await?
            .context("Failed to modify schema")?;
        self.schema_change_handler.on_change(schema);
        Ok(())
    }

    /// Updates an entity in the schema.
    pub async fn update_schema_entity(
        &self,
        entity: EntityKind,
        name: String,
    ) -> Result<(), CoreError> {
        let schema = self
            .schema
            .modify(move |schema| {
                let Some(entity_schema) = schema.entities.get_mut(&entity) else {
                    anyhow::bail!("Entity kind does not exist");
                };

                entity_schema.name = name;

                Ok(schema.clone())
            })
            .await?
            .context("Failed to modify schema")?;
        self.schema_change_handler.on_change(schema);
        Ok(())
    }

    /// Creates an attribute for an entity in the schema.
    pub async fn create_schema_entity_attribute(
        &self,
        entity: EntityKind,
        name: String,
        value: ValueKind,
    ) -> Result<AttributeKind, CoreError> {
        let (schema, attribute_kind) = self
            .schema
            .modify(move |schema| {
                let Some(entity_schema) = schema.entities.get_mut(&entity) else {
                    anyhow::bail!("Entity kind does not exist");
                };

                let attribute_kind = AttributeKind::random();
                entity_schema
                    .attributes
                    .insert(attribute_kind, AttributeSchema { name, value });

                Ok((schema.clone(), attribute_kind))
            })
            .await?
            .context("Failed to modify schema")?;
        self.schema_change_handler.on_change(schema);
        Ok(attribute_kind)
    }

    /// Deletes an attribute for an entity in the schema.
    pub async fn delete_schema_entity_attribute(
        &self,
        entity: EntityKind,
        attribute: AttributeKind,
    ) -> Result<(), CoreError> {
        let schema = self
            .schema
            .modify(move |schema| {
                let Some(entity_schema) = schema.entities.get_mut(&entity) else {
                    anyhow::bail!("Entity kind does not exist");
                };

                let removed = entity_schema.attributes.remove(&attribute);
                if removed.is_none() {
                    anyhow::bail!("Attribute kind does not exist");
                }

                Ok(schema.clone())
            })
            .await?
            .context("Failed to modify schema")?;
        self.schema_change_handler.on_change(schema);
        Ok(())
    }

    /// Updates an attribute for an entity in the schema.
    pub async fn update_schema_entity_attribute(
        &self,
        entity: EntityKind,
        attribute: AttributeKind,
        name: Option<String>,
        value: Option<ValueKind>,
    ) -> Result<(), CoreError> {
        let schema = self
            .schema
            .modify(move |schema| {
                let Some(entity_schema) = schema.entities.get_mut(&entity) else {
                    anyhow::bail!("Entity kind does not exist");
                };

                let Some(attribute_schema) = entity_schema.attributes.get_mut(&attribute) else {
                    anyhow::bail!("Attribute kind does not exist");
                };

                if let Some(name) = name {
                    attribute_schema.name = name;
                }
                if let Some(value) = value {
                    attribute_schema.value = value;
                }

                Ok(schema.clone())
            })
            .await?
            .context("Failed to modify schema")?;
        self.schema_change_handler.on_change(schema);
        Ok(())
    }

    /// Creates a relation in the schema, returning its ID.
    pub async fn create_schema_relation(
        &self,
        name: String,
        source: EntityKind,
        target: EntityKind,
    ) -> Result<RelationKind, CoreError> {
        let (schema, relation_kind) = self
            .schema
            .modify(move |schema| -> Result<_, anyhow::Error> {
                let relation_kind = RelationKind::random();
                schema.relations.insert(
                    relation_kind,
                    RelationSchema {
                        name,
                        source,
                        target,
                        attributes: HashMap::new(),
                    },
                );

                Ok((schema.clone(), relation_kind))
            })
            .await?
            .context("Failed to modify schema")?;
        self.schema_change_handler.on_change(schema);
        Ok(relation_kind)
    }

    /// Deletes a relation in the schema.
    pub async fn delete_schema_relation(&self, relation: RelationKind) -> Result<(), CoreError> {
        let schema = self
            .schema
            .modify(move |schema| {
                let removed = schema.relations.remove(&relation);
                if removed.is_none() {
                    anyhow::bail!("Relation kind does not exist");
                }

                Ok(schema.clone())
            })
            .await?
            .context("Failed to modify schema")?;
        self.schema_change_handler.on_change(schema);
        Ok(())
    }

    /// Updates a relation in the schema.
    pub async fn update_schema_relation(
        &self,
        relation: RelationKind,
        name: String,
    ) -> Result<(), CoreError> {
        let schema = self
            .schema
            .modify(move |schema| {
                let Some(relation_schema) = schema.relations.get_mut(&relation) else {
                    anyhow::bail!("Relation kind does not exist");
                };

                relation_schema.name = name;

                Ok(schema.clone())
            })
            .await?
            .context("Failed to modify schema")?;
        self.schema_change_handler.on_change(schema);
        Ok(())
    }

    /// Creates an attribute for a relation in the schema.
    pub async fn create_schema_relation_attribute(
        &self,
        relation: RelationKind,
        name: String,
        value: ValueKind,
    ) -> Result<AttributeKind, CoreError> {
        let (schema, attribute_kind) = self
            .schema
            .modify(move |schema| {
                let Some(relation_schema) = schema.relations.get_mut(&relation) else {
                    anyhow::bail!("Relation kind does not exist");
                };

                let attribute_kind = AttributeKind::random();
                relation_schema
                    .attributes
                    .insert(AttributeKind::random(), AttributeSchema { name, value });

                Ok((schema.clone(), attribute_kind))
            })
            .await?
            .context("Failed to modify schema")?;
        self.schema_change_handler.on_change(schema);
        Ok(attribute_kind)
    }

    /// Deletes an attribute for a relation in the schema.
    pub async fn delete_schema_relation_attribute(
        &self,
        relation: RelationKind,
        attribute: AttributeKind,
    ) -> Result<(), CoreError> {
        let schema = self
            .schema
            .modify(move |schema| {
                let Some(relation_schema) = schema.relations.get_mut(&relation) else {
                    anyhow::bail!("Relation kind does not exist");
                };

                let removed = relation_schema.attributes.remove(&attribute);
                if removed.is_none() {
                    anyhow::bail!("Attribute kind does not exist");
                }

                Ok(schema.clone())
            })
            .await?
            .context("Failed to modify schema")?;
        self.schema_change_handler.on_change(schema);
        Ok(())
    }

    /// Updates an attribute for a relation in the schema.
    pub async fn update_schema_relation_attribute(
        &self,
        relation: RelationKind,
        attribute: AttributeKind,
        name: Option<String>,
        value: Option<ValueKind>,
    ) -> Result<(), CoreError> {
        let schema = self
            .schema
            .modify(move |schema| {
                let Some(relation_schema) = schema.relations.get_mut(&relation) else {
                    anyhow::bail!("Relation kind does not exist");
                };

                let Some(attribute_schema) = relation_schema.attributes.get_mut(&attribute) else {
                    anyhow::bail!("Attribute kind does not exist");
                };

                if let Some(name) = name {
                    attribute_schema.name = name;
                }
                if let Some(value) = value {
                    attribute_schema.value = value;
                }

                Ok(schema.clone())
            })
            .await?
            .context("Failed to modify schema")?;
        self.schema_change_handler.on_change(schema);
        Ok(())
    }

    pub fn start_import(
        &self,
        roots: Vec<String>,
        event_handler: Arc<dyn ImportEventHandler>,
    ) -> Result<(), CoreError> {
        let _guard = self.runtime_handle.enter();
        ImportTask::spawn(
            self.cancellation_token.child_token(),
            event_handler,
            roots.into_iter().map(Into::into).collect(),
        )?;
        Ok(())
    }
}

#[derive(uniffi::Record)]
pub struct CreateSchemaEntityResult {
    schema: Schema,
    entity_kind: EntityKind,
}

impl Core {
    fn version_now(&self) -> Version {
        Version::new(Timestamp::now(), self.author)
    }
}

// Stub debug implementation
impl std::fmt::Debug for Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Core").finish()
    }
}

#[derive(uniffi::Record)]
pub struct CoreEntity {
    kind: EntityKind,
    attributes: HashMap<AttributeKind, CoreAttribute>,
}

#[derive(uniffi::Record)]
pub struct CoreRelation {
    kind: RelationKind,
    source: EntityId,
    target: EntityId,
    attributes: HashMap<AttributeKind, CoreAttribute>,
}

#[derive(uniffi::Record)]
pub struct CoreAttribute {
    value: Value,
    // version: CoreVersion,
}

/// Foreign trait for receiving devices change events.
#[uniffi::export(with_foreign)]
pub trait DevicesChangeHandler: Send + Sync {
    fn on_change(&self, devices_state: DevicesState);
}

/// Foreign trait for receiving schema change events.
#[uniffi::export(with_foreign)]
pub trait SchemaChangeHandler: Send + Sync {
    fn on_change(&self, schema: Schema);
}

fn run_core_thread(
    profile: String,
    log_guard: Option<LogGuard>,
    core_tx: oneshot::Sender<Core>,
    devices_change_handler: Arc<dyn DevicesChangeHandler>,
    schema_change_handler: Arc<dyn SchemaChangeHandler>,
) -> Result<(), anyhow::Error> {
    debug!("Core thread started");

    let builder = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Should build runtime");

    builder.block_on(async move {
        debug!("Core runtime started");

        let Some(project_dirs) = ProjectDirs::from("", "", "Stellar") else {
            unimplemented!("ProjectDirs returned None");
        };

        let data_dir = {
            let mut data_dir = project_dirs.data_local_dir().to_path_buf();
            data_dir.push(profile);
            data_dir
        };
        std::fs::create_dir_all(&data_dir).context("Failed to create data dir")?;

        let key_path = data_dir.join("secret_key");
        let secret_key = if key_path.exists() {
            let key_bytes = std::fs::read(&key_path).context("Failed to read secret key file")?;
            SecretKey::from_bytes(
                key_bytes
                    .as_slice()
                    .try_into()
                    .context("Failed to parse secret key file")?,
            )
        } else {
            let new_key = SecretKey::generate();
            std::fs::write(&key_path, new_key.to_bytes())
                .context("Failed to write secret key file")?;
            new_key
        };
        let public_key = secret_key.public();
        let endpoint_id = EndpointId::from(public_key);
        let author = AuthorId::from_slice(public_key.as_bytes());

        let database = Database::open(&data_dir).context("Failed to open database")?;

        let cancellation_token = CancellationToken::new();

        let schema = SchemaStoreTask::spawn(cancellation_token.child_token(), &data_dir, author)?;

        // Spawn task to forward schema changes to provided SchemaChangeHandler
        tokio::task::spawn({
            let schema_change_handler = schema_change_handler.clone();
            let mut schema_rx = schema.watch_schema();
            async move {
                loop {
                    match schema_rx.changed().await {
                        Ok(()) => {
                            let schema = schema_rx.borrow();
                            if let Some(schema) = schema.as_ref() {
                                schema_change_handler.on_change(schema.clone());
                            }
                        }
                        Err(_) => {
                            tracing::debug!("SchemaChangeHandler task exiting");
                            break;
                        }
                    }
                }
            }
        });

        let (endpoint_id_tx, endpoint_id_rx) = tokio::sync::watch::channel(None);
        let (devices_tx, devices_rx) = tokio::sync::watch::channel(Vec::new());

        let peers = PeersTask::spawn(
            cancellation_token.child_token(),
            PeersDatabaseAdapter::new(database.clone()),
            PeersSchemaAdapter::new(schema.clone()),
            devices_rx,
            secret_key,
        );
        let _ = endpoint_id_tx.send(Some(peers.endpoint_id()));

        let devices = DevicesTask::spawn(
            cancellation_token.child_token(),
            data_dir,
            endpoint_id_rx,
            devices_tx,
        );

        // Spawn task to forward devices state changes to provided DevicesChangeHandler
        tokio::task::spawn({
            let devices_change_handler = devices_change_handler.clone();
            let mut state_rx = devices.watch_state();
            async move {
                loop {
                    match state_rx.changed().await {
                        Ok(()) => {
                            let state = state_rx.borrow();
                            if let Some(state) = state.as_ref() {
                                devices_change_handler.on_change(state.clone());
                            }
                        }
                        Err(_) => {
                            tracing::debug!("SchemaChangeHandler task exiting");
                            break;
                        }
                    }
                }
            }
        });

        let core = Core {
            runtime_handle: tokio::runtime::Handle::current(),
            cancellation_token: cancellation_token.clone(),
            database,
            schema,
            peers,
            devices,
            endpoint_id,
            author,
            devices_change_handler,
            schema_change_handler,
            log_guard,
        };
        core_tx.send(core).expect("Should send core");

        cancellation_token.cancelled().await;

        debug!("Core runtime finishing");

        Ok::<(), anyhow::Error>(())
    })
}
