#[tokio::main]
async fn main() {
    let device_code = stellar::sync::device_code::start_device_code_flow()
        .await
        .unwrap();
    println!(
        "Visit the following URL to authorize the device: {}",
        device_code.verification_uri_complete
    );
    let access_token = stellar::sync::device_code::poll_device_code_flow(device_code)
        .await
        .unwrap();
    println!("Access token: {}", access_token);
    let sessions = stellar::sync::list_sessions(access_token).await.unwrap();
    println!("Sessions: {}", sessions);
}
