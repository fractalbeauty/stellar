use std::{sync::Arc, time::Duration};
use stellar_import::import::{ImportEventHandler, ImportEventScannedFile, ImportTask};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let dir = std::env::args().nth(1).expect("expected directory to scan");

    tracing::info!("Scanning {}", dir);

    let cancellation_token = CancellationToken::new();
    let task = ImportTask::spawn(
        cancellation_token,
        Arc::new(ExampleImportEventHandler),
        vec![dir.into()],
    )?;

    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down");

    task.cancel();

    tokio::time::sleep(Duration::from_millis(100)).await;

    Ok(())
}

struct ExampleImportEventHandler;

impl ImportEventHandler for ExampleImportEventHandler {
    fn on_pending_file(&self, path: String) {
        println!("pending: {path}");
    }

    fn on_scanned_file(&self, file: ImportEventScannedFile) {
        println!("scanned {} -> {:?}", file.path, file.tags)
    }
}
