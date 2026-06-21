use std::time::Duration;

use n0_watcher::Watchable;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let endpoint_id = Watchable::new(None);

    let peer = stellar::sync::peer::Peer::start().await.unwrap();
    let _ = endpoint_id.set(Some(peer.endpoint_id()));

    let mut devices = stellar::sync::devices::Devices::new(endpoint_id);

    let device_code = devices.start_device_code_flow().await.unwrap();
    println!(
        "Visit the following URL to authorize the device: {}",
        device_code.verification_uri_complete
    );

    devices.poll_device_code_flow(device_code).await.unwrap();
    println!("Device authorized");

    // let device_code = stellar::sync::devices::start_device_code_flow()
    //     .await
    //     .unwrap();
    // println!(
    //     "Visit the following URL to authorize the device: {}",
    //     device_code.verification_uri_complete
    // );
    // let access_token = stellar::sync::devices::poll_device_code_flow(device_code)
    //     .await
    //     .unwrap();
    // println!("Access token: {}", access_token);
    // let sessions = stellar::sync::list_sessions(access_token).await.unwrap();
    // println!("Sessions: {}", sessions);

    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
