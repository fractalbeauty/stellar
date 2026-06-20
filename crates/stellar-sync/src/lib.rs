use anyhow::Context;
use url::Url;

pub mod device_code;

pub async fn list_sessions(access_token: String) -> anyhow::Result<String> {
    let base_url = Url::parse("https://sorrel.trillia.net").unwrap();

    let client = sorrel_client::Client::new(base_url, access_token)
        .context("Failed to create Sorrel client")?;

    let sessions = client
        .list_sessions()
        .await
        .context("Failed to list sessions")?;

    Ok(format!("{:#?}", sessions))
}
