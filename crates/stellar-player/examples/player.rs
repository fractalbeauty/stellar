use std::time::Duration;
use stellar_player::backend::CpalBackend;
use tracing_subscriber::EnvFilter;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")),
        )
        .init();

    let backend = CpalBackend::new().unwrap();

    tracing::info!("Initialized backend");

    std::thread::sleep(Duration::from_secs(10));
}
