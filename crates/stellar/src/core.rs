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
    AttributeKind, AuthorId, EntityId, EntityKind, Timestamp, Value, Version,
};
use stellar_graph::schema::{EntitySchema, Schema};
use stellar_log::LogGuard;
use stellar_sync::peers::{PeersDatabaseAdapter, PeersTask};
use stellar_sync::schema::SchemaStoreTask;
use stellar_sync::{EndpointId, SecretKey, devices::DevicesTask};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};

#[derive(uniffi::Object)]
pub struct Core {
    cancellation_token: CancellationToken,
    database: Database,
    schema: SchemaStoreTask,
    peers_task: PeersTask,
    devices_task: DevicesTask,

    author: AuthorId,

    #[allow(unused)]
    log_guard: Option<LogGuard>,
}

#[uniffi::export]
impl Core {
    #[uniffi::constructor]
    pub async fn spawn(
        profile: String,
        schema_change_handler: Arc<dyn SchemaChangeHandler>,
    ) -> Result<Arc<Self>, CoreError> {
        let log_guard = stellar_log::init(None)?;

        let (core_tx, core_rx) = oneshot::channel();

        std::thread::spawn({
            move || {
                let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    run_core_thread(profile, log_guard, core_tx, schema_change_handler)
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

    pub async fn start_device_code_flow(&self) -> Result<String, CoreError> {
        let rx = self.devices_task.start_device_code_flow()?;

        let verification_uri_complete = async_std::future::timeout(Duration::from_secs(10), rx)
            .await
            .map_err(|_elapsed| core_error!("Timed out waiting for device code flow to start"))?
            .map_err(|_dropped| core_error!("Device code flow failed to start, sender dropped"))?;

        Ok(verification_uri_complete)
    }

    pub fn add_device(&self, endpoint_id: String, name: Option<String>) -> Result<(), CoreError> {
        let endpoint_id = endpoint_id
            .parse::<EndpointId>()
            .map_err(|_| core_error!("Failed to parse endpoint ID"))?;

        self.devices_task.add_device(endpoint_id, name)?;

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
                        kind: data.metadata.kind,
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

    /// Creates an entity kind, returning its ID.
    pub async fn create_schema_entity(&self, name: String) -> Result<EntityKind, CoreError> {
        let entity = EntityKind::random();
        self.schema
            .modify(move |schema| {
                schema.entities.insert(
                    entity,
                    EntitySchema {
                        name,
                        attributes: HashMap::new(),
                    },
                );
            })
            .await?;
        Ok(entity)
    }

    /// Deletes an entity kind.
    pub async fn delete_schema_entity(&self, entity: EntityKind) -> Result<(), CoreError> {
        self.schema
            .modify(move |schema| {
                if !schema.entities.contains_key(&entity) {
                    anyhow::bail!("Entity kind does not exist");
                }

                schema.entities.remove(&entity);
                Ok(())
            })
            .await?
            .context("Failed to delete entity")?;
        Ok(())
    }
}

impl Core {
    // TODO
    pub fn add_random_entity(&self) -> Result<(), anyhow::Error> {
        self.database.upsert_entity(
            stellar_graph::entity::EntityId::random(),
            stellar_graph::store::EntityData {
                metadata: stellar_graph::store::EntityMetadataValue {
                    kind: stellar_graph::entity::EntityKind::random(),
                    deleted: false,
                    deleted_version: stellar_graph::entity::Version::new(
                        stellar_graph::entity::Timestamp::now(),
                        stellar_graph::entity::AuthorId::new([0u8; 32]),
                    ),
                },
                attributes: HashMap::new(),
            },
        )?;
        Ok(())
    }

    // TODO
    pub fn debug_entities(&self) -> Result<String, anyhow::Error> {
        Ok(format!("{:?}", self.database.get_entities()?))
    }

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
pub struct CoreAttribute {
    value: Value,
    // version: CoreVersion,
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
        let author = AuthorId::new(*public_key.as_bytes());

        let database = Database::open(&data_dir).context("Failed to open database")?;

        let cancellation_token = CancellationToken::new();

        let schema = SchemaStoreTask::spawn(cancellation_token.child_token(), &data_dir, author)?;

        // Spawn task to forward schema changes to provided SchemaChangeHandler
        tokio::task::spawn({
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

        let peers_task = PeersTask::spawn(
            cancellation_token.child_token(),
            PeersDatabaseAdapter::new(database.clone()),
            devices_rx,
            secret_key,
        );
        let _ = endpoint_id_tx.send(Some(peers_task.endpoint_id()));

        let devices_task =
            DevicesTask::spawn(cancellation_token.child_token(), endpoint_id_rx, devices_tx);

        let core = Core {
            cancellation_token: cancellation_token.clone(),
            database,
            schema,
            peers_task,
            devices_task,
            author,
            log_guard,
        };
        core_tx.send(core).expect("Should send core");

        cancellation_token.cancelled().await;

        debug!("Core runtime finishing");

        Ok::<(), anyhow::Error>(())
    })
}
