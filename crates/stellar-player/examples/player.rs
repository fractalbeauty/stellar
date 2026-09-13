use std::{path::PathBuf, time::Duration};
use stellar_player::backend::CpalBackendTask;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")),
        )
        .init();

    let path = PathBuf::from(std::env::args().nth(1).expect("Usage: player <path>"));

    let backend = CpalBackendTask::spawn(CancellationToken::new()).unwrap();

    tracing::info!("Initialized backend");

    backend.play(path).unwrap();

    tokio::time::sleep(Duration::from_secs(30)).await;
}
