use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use anyhow::Context;
use reqwest::StatusCode;
use serde::Deserialize;
use url::Url;

pub struct DeviceCode {
    device_code: String,
    expires_at: Instant,
    interval: Duration,

    pub verification_uri_complete: String,
}

/// Start the device code flow, returning the verification URI and polling information.
pub async fn start_device_code_flow() -> anyhow::Result<DeviceCode> {
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
pub async fn poll_device_code_flow(device_code: DeviceCode) -> anyhow::Result<String> {
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
