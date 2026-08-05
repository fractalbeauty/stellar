use crate::error::{CoreError, core_error};
use anyhow::Context;
use directories_next::ProjectDirs;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;
use std::unimplemented;
use stellar_graph::database::Database;
use stellar_log::LogGuard;
use stellar_sync::peers::PeersTask;
use stellar_sync::{EndpointId, devices::DevicesTask};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};

#[derive(uniffi::Object)]
pub struct Core {
    cancellation_token: CancellationToken,
    peers_task: PeersTask,
    devices_task: DevicesTask,

    #[allow(unused)]
    log_guard: Option<LogGuard>,
}

#[uniffi::export]
impl Core {
    #[uniffi::constructor]
    pub async fn spawn(profile: String) -> Result<Arc<Self>, CoreError> {
        let log_guard = stellar_log::init(None)?;

        let (core_tx, core_rx) = oneshot::channel();

        std::thread::spawn({
            move || {
                let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    run_core_thread(profile, log_guard, core_tx)
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
}

// stub debug implementation
impl std::fmt::Debug for Core {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Core").finish()
    }
}

fn run_core_thread(
    profile: String,
    log_guard: Option<LogGuard>,
    core_tx: oneshot::Sender<Core>,
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

        let database = Database::open(data_dir).context("Failed to open database")?;

        let cancellation_token = CancellationToken::new();

        let (endpoint_id_tx, endpoint_id_rx) = tokio::sync::watch::channel(None);
        let (devices_tx, devices_rx) = tokio::sync::watch::channel(Vec::new());

        let peers_task = PeersTask::spawn(cancellation_token.child_token(), devices_rx).unwrap();
        let _ = endpoint_id_tx.send(Some(peers_task.endpoint_id()));

        let devices_task =
            DevicesTask::spawn(cancellation_token.child_token(), endpoint_id_rx, devices_tx);

        let core = Core {
            cancellation_token: cancellation_token.clone(),
            peers_task,
            devices_task,
            log_guard,
        };
        core_tx.send(core).expect("Should send core");

        cancellation_token.cancelled().await;

        debug!("Core runtime finishing");

        Ok::<(), anyhow::Error>(())
    })
}
