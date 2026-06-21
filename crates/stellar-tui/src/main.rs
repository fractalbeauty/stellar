use std::time::Duration;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let (endpoint_id_tx, endpoint_id_rx) = tokio::sync::watch::channel(None);
    let (devices_tx, devices_rx) = tokio::sync::watch::channel(Vec::new());

    let peers = stellar::sync::peers::PeersTask::spawn(devices_rx).unwrap();
    let _ = endpoint_id_tx.send(Some(peers.endpoint_id()));

    let mut devices = stellar::sync::devices::Devices::new(endpoint_id_rx, devices_tx);

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
