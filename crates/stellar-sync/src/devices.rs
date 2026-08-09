use anyhow::Context;
use futures::StreamExt;
use iroh::EndpointId;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sorrel_client::Client;
use sorrel_client::api::keys::{ListKeysResponse, SetKeyRequest};
use sorrel_client::api::sessions::SessionRevokeResponse;
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
    state_rx: watch::Receiver<Option<DevicesState>>,
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
    RevokeAuthSession {
        session: Uuid,
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

        let (state_tx, state_rx) = watch::channel(None);
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        tokio::spawn({
            let cancellation_token = cancellation_token.clone();
            async move {
                let (event_tx, event_rx) = mpsc::unbounded_channel();

                let mut devices = Devices {
                    endpoint_id_rx,
                    devices_tx,
                    state_tx,
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
            state_rx,
            message_tx,
        }
    }

    pub fn cancel(&self) {
        debug!("Cancelling devices task");
        self.cancellation_token.cancel();
    }

    pub fn watch_state(&self) -> watch::Receiver<Option<DevicesState>> {
        self.state_rx.clone()
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

    pub fn revoke_auth_session(&self, session: Uuid) -> Result<(), anyhow::Error> {
        self.message_tx
            .send(DevicesMessage::RevokeAuthSession { session })
            .map_err(|_| anyhow::anyhow!("Failed to send, devices task has been dropped"))?;

        Ok(())
    }
}

/// Owned state for the devices task
#[derive(Debug)]
struct Devices {
    endpoint_id_rx: watch::Receiver<Option<EndpointId>>,
    devices_tx: watch::Sender<Vec<Device>>,
    state_tx: watch::Sender<Option<DevicesState>>,
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

        self.notify_state();

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

                self.notify_devices_and_state();

                self.save_data()
                    .await
                    .context("Failed to save devices data")?;
            }
            DevicesMessage::RevokeAuthSession { session } => {
                let Some(auth) = &self.auth else {
                    anyhow::bail!("Auth task is None");
                };
                auth.revoke_session(session)?;
            }
        }

        Ok(())
    }

    async fn handle_event(&mut self, event: DevicesEvent) -> Result<(), anyhow::Error> {
        match event {
            DevicesEvent::DeviceCodeFlowFinished { access_token } => {
                self.set_access_token_without_saving(access_token)?;

                self.notify_state();

                self.save_data()
                    .await
                    .context("Failed to save devices data")?;
            }
            DevicesEvent::AuthDevicesChanged { auth_devices } => {
                self.auth_devices = auth_devices;

                self.notify_devices_and_state();
            }
        }

        Ok(())
    }

    fn notify_devices_and_state(&self) {
        let all_devices = self
            .added_devices
            .iter()
            .chain(self.auth_devices.iter())
            .cloned()
            .collect();
        self.devices_tx.send_replace(all_devices);

        self.notify_state();
    }

    fn notify_state(&self) {
        let added_devices = self
            .added_devices
            .iter()
            .map(|device| DevicesStateDevice {
                endpoint_id: device.endpoint_id,
                name: device.name.clone(),
                session: device
                    .session
                    .as_ref()
                    .map(|session| DevicesStateDeviceSession {
                        id: session.id,
                        last_used_at: session.last_used_at,
                    }),
            })
            .collect();
        let auth_devices = self
            .auth_devices
            .iter()
            .map(|device| DevicesStateDevice {
                endpoint_id: device.endpoint_id,
                name: device.name.clone(),
                session: device
                    .session
                    .as_ref()
                    .map(|session| DevicesStateDeviceSession {
                        id: session.id,
                        last_used_at: session.last_used_at,
                    }),
            })
            .collect();

        let state = DevicesState {
            authed: self.access_token.is_some(),
            added_devices,
            auth_devices,
        };
        self.state_tx.send_replace(Some(state));
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

/// Devices state exposed to the UI
#[derive(Debug, Clone, uniffi::Record)]
pub struct DevicesState {
    authed: bool,
    added_devices: Vec<DevicesStateDevice>,
    auth_devices: Vec<DevicesStateDevice>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DevicesStateDevice {
    pub endpoint_id: EndpointId,
    pub name: Option<String>,
    pub session: Option<DevicesStateDeviceSession>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DevicesStateDeviceSession {
    pub id: Uuid,
    pub last_used_at: u64,
}

/// Persisted devices data
#[derive(Debug, Default, Serialize, Deserialize)]
struct DevicesData {
    access_token: Option<String>,
    // added_devices
}

/// Handle for the auth task
#[derive(Debug)]
struct AuthTask {
    cancellation_token: CancellationToken,
    message_tx: mpsc::UnboundedSender<AuthMessage>,
}

impl AuthTask {
    fn revoke_session(&self, session: Uuid) -> Result<(), anyhow::Error> {
        self.message_tx
            .send(AuthMessage::RevokeSession { session })?;
        Ok(())
    }
}

/// Messages to the auths task
enum AuthMessage {
    RevokeSession { session: Uuid },
}

fn spawn_auth_task(
    endpoint_id: watch::Receiver<Option<EndpointId>>,
    event_tx: mpsc::UnboundedSender<DevicesEvent>,
    access_token: String,
) -> AuthTask {
    let cancellation_token = CancellationToken::new();
    let (message_tx, message_rx) = mpsc::unbounded_channel();

    tokio::spawn({
        let event_tx = event_tx.clone();
        let cancellation_token = cancellation_token.clone();
        async move {
            let mut auth = match Auth::init(access_token, event_tx) {
                Ok(auth) => auth,
                Err(e) => {
                    tracing::error!("Auth task failed to init: {e:?}");
                    return;
                }
            };

            if let Err(error) = auth.run(endpoint_id, message_rx, cancellation_token).await {
                tracing::error!("Auth task errored: {:?}", error);
            } else {
                tracing::debug!("Auth task finished");
            }
        }
    });

    AuthTask {
        cancellation_token,
        message_tx,
    }
}

/// Owned state for the auth task
struct Auth {
    client: Client,
    event_tx: mpsc::UnboundedSender<DevicesEvent>,
}

impl Auth {
    fn init(
        access_token: String,
        event_tx: mpsc::UnboundedSender<DevicesEvent>,
    ) -> Result<Self, anyhow::Error> {
        let base_url = Url::parse("https://sorrel.trillia.net").unwrap();
        let client = sorrel_client::Client::new(base_url, access_token)?;

        Ok(Self { client, event_tx })
    }

    async fn run(
        &mut self,
        endpoint_id: watch::Receiver<Option<EndpointId>>,
        mut message_rx: mpsc::UnboundedReceiver<AuthMessage>,
        cancellation_token: CancellationToken,
    ) -> Result<(), anyhow::Error> {
        let mut endpoint_id = WatchStream::new(endpoint_id);

        // TODO
        let mut interval = tokio::time::interval(Duration::from_secs(60));

        loop {
            tokio::select! {
                _ = cancellation_token.cancelled() => {
                    tracing::debug!("Cancelled");
                    break;
                }

                Some(message) = message_rx.recv() => {
                    match message {
                        AuthMessage::RevokeSession { session } => {
                            let response = self.client.revoke_session(session).await?;

                            match response {
                                SessionRevokeResponse::Success {..} => {
                                    tracing::info!(?session, "Revoked session");
                                }
                                response => {
                                    tracing::error!(?session, "Failed to revoke session, response: {response:?}");
                                }
                            }

                            if let Err(e) = self.list_keys().await {
                                tracing::error!("Refreshing keys after revoking session failed: {e:?}");
                            }
                        }
                    }
                }

                // Set device key when the endpoint ID changes
                Some(Some(endpoint_id)) = endpoint_id.next() => {
                    let result = self.client.set_key(SetKeyRequest {
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
                    if let Err(e) = self.list_keys().await {
                        tracing::error!("Periodic auth devices refresh failed: {e:?}");
                    }
                }
            }
        }

        Ok(())
    }

    async fn list_keys(&mut self) -> Result<(), anyhow::Error> {
        let response = self.client.list_keys().await?;

        match response {
            ListKeysResponse::Success(response) => {
                let new_devices = response
                    .keys
                    .into_iter()
                    .filter(|key| key.app == SORREL_APP)
                    .filter_map(|key| {
                        let Ok(endpoint_id) = EndpointId::from_bytes(&key.public_key) else {
                            tracing::error!(
                                "Failed to parse endpoint ID from public key bytes, skipping device"
                            );
                            return None;
                        };

                        Some(Device {
                            endpoint_id,
                            name: key.session_device_name,
                            session: Some(DeviceSession {
                                id: key.session_id,
                                last_used_at: key.session_last_used_at,
                            }),
                        })
                    })
                    .collect::<Vec<_>>();

                let _ = self.event_tx.send(DevicesEvent::AuthDevicesChanged {
                    auth_devices: new_devices,
                });

                tracing::debug!("Refreshed devices");

                Ok(())
            }

            response => {
                anyhow::bail!("Failed to refresh keys, response: {:?}", response);
            }
        }
    }
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
