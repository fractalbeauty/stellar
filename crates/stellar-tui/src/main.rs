use std::time::Duration;
use stellar::core::Core;

#[tokio::main]
async fn main() {
    // tracing_subscriber::fmt::init();

    let core = Core::spawn("default".to_string())
        .await
        .expect("Should spawn core");

    // let verification_uri_complete = core
    //     .start_device_code_flow()
    //     .await
    //     .expect("Should start device code flow");

    // println!(
    //     "Visit the following URL to authorize the device: {}",
    //     verification_uri_complete
    // );

    if let Ok(endpoint_id) = std::env::var("STELLAR_CONNECT_TO") {
        core.add_device(endpoint_id, None).unwrap();
    }

    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
