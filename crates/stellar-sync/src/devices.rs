use anyhow::Context;
use futures::StreamExt;
use iroh::EndpointId;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sorrel_client::api::keys::{ListKeysResponse, SetKeyRequest};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot, watch};
use tokio_stream::wrappers::WatchStream;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use url::Url;
use uuid::Uuid;

/// Handle to the devices task
pub struct DevicesTask {
    cancellation_token: CancellationToken,
    message_tx: mpsc::UnboundedSender<DevicesMessage>,
}

/// Messages to the devices task
pub enum DevicesMessage {
    StartDeviceCodeFlow {
        verification_uri_complete_tx: oneshot::Sender<String>,
    },
    AddDevice {
        endpoint_id: EndpointId,
        name: Option<String>,
    },
}

impl DevicesTask {
    /// `data_dir` is the directory devices data should be persisted to.
    pub fn spawn(
        cancellation_token: CancellationToken,
        data_dir: impl AsRef<Path>,
        endpoint_id_rx: watch::Receiver<Option<EndpointId>>,
        devices_tx: watch::Sender<Vec<Device>>,
    ) -> Self {
        let data_path = data_dir.as_ref().join("devices.json");

        let (message_tx, message_rx) = mpsc::unbounded_channel();

        tokio::spawn({
            let cancellation_token = cancellation_token.clone();
            async move {
                let (event_tx, event_rx) = mpsc::unbounded_channel();

                let mut devices = Devices {
                    endpoint_id_rx,
                    devices_tx,
                    message_rx,

                    event_tx,
                    event_rx,

                    data_path,

                    added_devices: Vec::new(),
                    auth_devices: Vec::new(),

                    access_token: None,

                    auth: None,
                };

                let result = devices.run(cancellation_token).await;

                if let Err(error) = result {
                    tracing::error!("Devices task errored: {error}");
                } else {
                    tracing::debug!("Devices task finished");
                }
            }
        });

        Self {
            cancellation_token,
            message_tx,
        }
    }

    pub fn cancel(&self) {
        debug!("Cancelling devices task");
        self.cancellation_token.cancel();
    }

    pub fn start_device_code_flow(&self) -> Result<oneshot::Receiver<String>, anyhow::Error> {
        let (verification_uri_complete_tx, verification_uri_complete_rx) = oneshot::channel();

        self.message_tx
            .send(DevicesMessage::StartDeviceCodeFlow {
                verification_uri_complete_tx,
            })
            .map_err(|_| anyhow::anyhow!("Failed to send, devices task has been dropped"))?;

        Ok(verification_uri_complete_rx)
    }

    pub fn add_device(
        &self,
        endpoint_id: EndpointId,
        name: Option<String>,
    ) -> Result<(), anyhow::Error> {
        self.message_tx
            .send(DevicesMessage::AddDevice { endpoint_id, name })
            .map_err(|_| anyhow::anyhow!("Failed to send, devices task has been dropped"))?;

        Ok(())
    }
}

/// Owned state for the devices task
#[derive(Debug)]
struct Devices {
    endpoint_id_rx: watch::Receiver<Option<EndpointId>>,
    devices_tx: watch::Sender<Vec<Device>>,
    message_rx: mpsc::UnboundedReceiver<DevicesMessage>,

    event_tx: mpsc::UnboundedSender<DevicesEvent>,
    event_rx: mpsc::UnboundedReceiver<DevicesEvent>,

    /// The file to load/save data to
    data_path: PathBuf,

    added_devices: Vec<Device>,
    auth_devices: Vec<Device>,

    /// The current access token
    access_token: Option<String>,

    auth: Option<AuthTask>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub endpoint_id: EndpointId,
    pub name: Option<String>,
    pub session: Option<DeviceSession>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSession {
    pub id: Uuid,
    pub last_used_at: u64,
}

/// Internal events from child tasks to the devices task
enum DevicesEvent {
    DeviceCodeFlowFinished { access_token: String },
    AuthDevicesChanged { auth_devices: Vec<Device> },
}

impl Devices {
    async fn run(&mut self, cancellation_token: CancellationToken) -> Result<(), anyhow::Error> {
        // Try to load persisted data and configure stuff
        self.load_data()
            .await
            .context("Failed to load devices data")?;

        loop {
            tokio::select! {
                Some(message) = self.message_rx.recv() => {
                    self.handle_message(message).await.expect("TODO");
                }

                Some(event) = self.event_rx.recv() => {
                    self.handle_event(event).await.expect("TODO");
                }

                _ = cancellation_token.cancelled() => {
                    debug!("Cancelled");
                    break;
                }
            }
        }

        Ok(())
    }

    async fn handle_message(&mut self, message: DevicesMessage) -> Result<(), anyhow::Error> {
        match message {
            DevicesMessage::StartDeviceCodeFlow {
                verification_uri_complete_tx,
            } => {
                if self.auth.is_some() {
                    anyhow::bail!("Already authorized");
                }

                tokio::task::spawn({
                    let event_tx = self.event_tx.clone();
                    async move {
                        let device_code = match start_device_code_flow().await {
                            Ok(device_code) => device_code,
                            Err(error) => {
                                tracing::error!("Failed to start device code flow: {:?}", error);
                                return;
                            }
                        };

                        let _ = verification_uri_complete_tx
                            .send(device_code.verification_uri_complete.clone());

                        let access_token = match poll_device_code_flow(device_code).await {
                            Ok(access_token) => access_token,
                            Err(error) => {
                                tracing::error!("Failed to poll device code flow: {:?}", error);
                                return;
                            }
                        };

                        let _ =
                            event_tx.send(DevicesEvent::DeviceCodeFlowFinished { access_token });
                    }
                });
            }
            DevicesMessage::AddDevice { endpoint_id, name } => {
                self.added_devices.push(Device {
                    endpoint_id,
                    name,
                    session: None,
                });
                self.notify_devices()?;
                self.save_data()
                    .await
                    .context("Failed to save devices data")?;
            }
        }

        Ok(())
    }

    async fn handle_event(&mut self, event: DevicesEvent) -> Result<(), anyhow::Error> {
        match event {
            DevicesEvent::DeviceCodeFlowFinished { access_token } => {
                self.set_access_token_without_saving(access_token)?;

                self.save_data()
                    .await
                    .context("Failed to save devices data")?;
            }
            DevicesEvent::AuthDevicesChanged { auth_devices } => {
                self.auth_devices = auth_devices;
                self.notify_devices()?;
            }
        }

        Ok(())
    }

    fn notify_devices(&self) -> Result<(), anyhow::Error> {
        let all_devices = self
            .added_devices
            .iter()
            .chain(self.auth_devices.iter())
            .cloned()
            .collect();
        self.devices_tx.send(all_devices)?;
        Ok(())
    }

    /// Sets the access token and starts the auth task
    fn set_access_token_without_saving(
        &mut self,
        access_token: String,
    ) -> Result<(), anyhow::Error> {
        if self.auth.is_some() {
            anyhow::bail!("Already authorized, ignoring new access token");
        }

        self.access_token = Some(access_token.clone());

        self.auth = Some(spawn_auth_task(
            self.endpoint_id_rx.clone(),
            self.event_tx.clone(),
            access_token,
        ));

        Ok(())
    }

    /// Loads data from file (or initializes with defaults) and configures stuff
    async fn load_data(&mut self) -> Result<(), anyhow::Error> {
        let data = match tokio::fs::read(&self.data_path).await {
            Ok(bytes) => {
                tracing::debug!("Devices task read data");
                serde_json::from_slice::<DevicesData>(&bytes)
                    .context("Failed to deserialize devices data")?
            }
            Err(e) => {
                tracing::debug!("Devices failed to read data, using defaults: {e:?}");
                DevicesData::default()
            }
        };

        if let Some(access_token) = data.access_token {
            self.set_access_token_without_saving(access_token)?;
        }

        Ok(())
    }

    /// Saves data to file
    async fn save_data(&mut self) -> Result<(), anyhow::Error> {
        let data = DevicesData {
            access_token: self.access_token.clone(),
        };

        let json =
            serde_json::to_string_pretty(&data).context("Failed to serialize devices data")?;

        tokio::fs::write(&self.data_path, json.as_bytes())
            .await
            .context("Failed to write devices data file")?;

        tracing::debug!("Devices task saved data");

        Ok(())
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DevicesData {
    access_token: Option<String>,
    // added_devices
}

/// Handle for the auth task
#[derive(Debug)]
struct AuthTask {
    cancellation_token: CancellationToken,
}

fn spawn_auth_task(
    endpoint_id: watch::Receiver<Option<EndpointId>>,
    event_tx: mpsc::UnboundedSender<DevicesEvent>,
    access_token: String,
) -> AuthTask {
    let cancellation_token = CancellationToken::new();

    tokio::spawn({
        let event_tx = event_tx.clone();
        let cancellation_token = cancellation_token.clone();
        async move {
            if let Err(error) =
                run_auth_task(endpoint_id, access_token, event_tx, cancellation_token).await
            {
                tracing::error!("Auth task errored: {:?}", error);
            } else {
                tracing::debug!("Auth task finished");
            }
        }
    });

    AuthTask { cancellation_token }
}

async fn run_auth_task(
    endpoint_id: watch::Receiver<Option<EndpointId>>,
    access_token: String,
    event_tx: mpsc::UnboundedSender<DevicesEvent>,
    cancellation_token: CancellationToken,
) -> Result<(), anyhow::Error> {
    let base_url = Url::parse("https://sorrel.trillia.net").unwrap();
    let client = sorrel_client::Client::new(base_url, access_token)?;

    let mut endpoint_id = WatchStream::new(endpoint_id);

    // TODO
    let mut interval = tokio::time::interval(Duration::from_secs(60));

    loop {
        tokio::select! {
            _ = cancellation_token.cancelled() => {
                tracing::debug!("Cancelled");
                break;
            }

            // Set device key when the endpoint ID changes
            Some(Some(endpoint_id)) = endpoint_id.next() => {
                let result = client.set_key(SetKeyRequest {
                    app: SORREL_APP.to_string(),
                    public_key: *endpoint_id,
                }).await;

                match result {
                    Ok(_) => tracing::debug!("Set key"),
                    Err(error) => tracing::error!("Failed to set key, error: {:?}", error),
                }
            }

            // Periodically get device keys
            _ = interval.tick() => {
                let response = client.list_keys().await?;

                match response {
                    ListKeysResponse::Success(response) => {
                        let new_devices = response.keys.into_iter()
                            .filter(|key| key.app == SORREL_APP)
                            .filter_map(|key| {
                                let Ok(endpoint_id) = EndpointId::from_bytes(&key.public_key)  else {
                                    tracing::error!("Failed to parse endpoint ID from public key bytes, skipping device");
                                    return None;
                                };

                                Some(Device {
                                    endpoint_id,
                                    name: key.session_device_name,
                                    session: Some(DeviceSession {
                                        id: key.session_id,
                                        last_used_at: key.session_last_used_at,
                                    })
                                })
                            })
                            .collect::<Vec<_>>();

                        let _ = event_tx.send(DevicesEvent::AuthDevicesChanged {
                            auth_devices: new_devices,
                        });

                        tracing::debug!("Refreshed devices");
                    }

                    response => {
                        tracing::error!("Failed to refresh keys, response: {:?}", response);
                    }
                }
            }
        }
    }

    Ok(())
}

const SORREL_APP: &str = "stellar";

pub struct DeviceCode {
    device_code: String,
    expires_at: Instant,
    interval: Duration,

    pub verification_uri_complete: String,
}

/// Start the device code flow, returning the verification URI and polling information.
async fn start_device_code_flow() -> anyhow::Result<DeviceCode> {
    let base_url = Url::parse("https://sorrel.trillia.net").unwrap();

    let device_name = "stellar-test";

    let client = reqwest::Client::new();

    let start_req = HashMap::from([("device_name", device_name)]);
    let start_url = base_url.join("api/oauth/device").unwrap();
    let start_response = client
        .post(start_url)
        .json(&start_req)
        .send()
        .await
        .context("Failed to send device code start request")?;

    let start_response_status = start_response.status();
    if start_response_status != StatusCode::OK {
        anyhow::bail!(
            "Device code start request failed with status {}",
            start_response_status
        );
    }

    let start_response = start_response
        .json::<DeviceStartResponse>()
        .await
        .context("Failed to receive device code response")?;

    let expires_at =
        Instant::now() + std::time::Duration::from_secs(start_response.expires_in as u64);

    Ok(DeviceCode {
        device_code: start_response.device_code,
        verification_uri_complete: start_response.verification_uri_complete,
        expires_at,
        interval: std::time::Duration::from_secs(start_response.interval as u64),
    })
}

/// Poll the device code flow until the user authorizes, returning the access token.
async fn poll_device_code_flow(device_code: DeviceCode) -> anyhow::Result<String> {
    let base_url = Url::parse("https://sorrel.trillia.net").unwrap();

    let client = reqwest::Client::new();

    loop {
        if Instant::now() > device_code.expires_at {
            anyhow::bail!("Device code flow timed out");
        }

        tokio::time::sleep(device_code.interval).await;

        let poll_req = HashMap::from([("device_code", device_code.device_code.clone())]);
        let poll_url = base_url.join("api/oauth/device/poll").unwrap();
        let poll_response = client
            .post(poll_url)
            .json(&poll_req)
            .send()
            .await
            .context("Failed to send device code poll request")?;

        let poll_response_status = poll_response.status();
        if poll_response_status != StatusCode::OK {
            tracing::info!(
                "Device code poll request returned status {}, retrying",
                poll_response_status
            );
            continue;
        }

        let poll_response = match poll_response.json::<DevicePollResponse>().await {
            Ok(res) => res,
            Err(e) => {
                tracing::error!(
                    "Failed to receive device code poll response, retrying: {:?}",
                    e
                );
                continue;
            }
        };

        return Ok(poll_response.access_token);
    }
}

#[derive(Deserialize)]
struct DeviceStartResponse {
    device_code: String,
    // user_code: String,
    // verification_uri: String,
    verification_uri_complete: String,
    expires_in: i64,
    interval: i64,
}

#[derive(Deserialize)]
struct DevicePollResponse {
    access_token: String,
}
