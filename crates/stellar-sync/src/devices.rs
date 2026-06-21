use anyhow::Context;
use futures::StreamExt;
use iroh::EndpointId;
use reqwest::StatusCode;
use serde::Deserialize;
use sorrel_client::api::keys::{ListKeysResponse, SetKeyRequest};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

pub struct Devices {
    endpoint_id_rx: watch::Receiver<Option<EndpointId>>,
    devices_tx: watch::Sender<Vec<Device>>,

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

impl Devices {
    pub fn new(
        endpoint_id_rx: watch::Receiver<Option<EndpointId>>,
        devices_tx: watch::Sender<Vec<Device>>,
    ) -> Self {
        Self {
            endpoint_id_rx,
            devices_tx,

            auth: None,
        }
    }

    /// Start the device code flow.
    ///
    /// Returns the verification URI and polling information.
    pub async fn start_device_code_flow(&self) -> Result<DeviceCode, anyhow::Error> {
        if self.auth.is_some() {
            anyhow::bail!("Already authorized");
        }

        let device_code = start_device_code_flow().await?;
        Ok(device_code)
    }

    /// Poll the device code flow.
    ///
    /// Returns Ok if the user authorized.
    pub async fn poll_device_code_flow(
        // TODO
        &mut self,
        device_code: DeviceCode,
    ) -> Result<(), anyhow::Error> {
        if self.auth.is_some() {
            anyhow::bail!("Already authorized");
        }

        let access_token = poll_device_code_flow(device_code).await?;
        self.auth = Some(spawn_auth_task(
            self.endpoint_id_rx.clone(),
            self.devices_tx.clone(),
            access_token,
        ));

        Ok(())
    }
}

struct AuthTask {
    cancellation_token: CancellationToken,
}

fn spawn_auth_task(
    endpoint_id: watch::Receiver<Option<EndpointId>>,
    devices_tx: watch::Sender<Vec<Device>>,
    access_token: String,
) -> AuthTask {
    let cancellation_token = CancellationToken::new();

    tokio::spawn({
        let devices_tx = devices_tx.clone();
        let cancellation_token = cancellation_token.clone();
        async move {
            if let Err(error) =
                run_auth_task(endpoint_id, access_token, devices_tx, cancellation_token).await
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
    devices_tx: watch::Sender<Vec<Device>>,
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

                        let _ = devices_tx.send(new_devices);

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
