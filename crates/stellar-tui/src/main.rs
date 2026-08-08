use clap::{Args, Parser};
use std::time::Duration;
use stellar::core::Core;

/// Stellar TUI
#[derive(Parser)]
#[command(about)]
struct Cli {
    #[command(flatten)]
    profile: Profile,
}

#[derive(Args)]
#[group(required = false, multiple = false)]
struct Profile {
    #[arg(long, short)]
    profile: Option<String>,

    #[arg(long, short)]
    anonymous: bool,
}

#[tokio::main]
async fn main() {
    // tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let profile = if let Some(profile) = cli.profile.profile {
        profile
    } else if cli.profile.anonymous {
        let id: String = std::iter::repeat_with(fastrand::lowercase)
            .take(7)
            .collect();
        format!("anonymous-{id}")
    } else {
        "default".to_string()
    };

    let core = Core::spawn(profile).await.expect("Should spawn core");

    if std::env::var("STELLAR_ADD_ENTITY").is_ok_and(|var| !var.is_empty()) {
        core.add_random_entity().unwrap();
    }
    // let _ = dbg!(core.debug_entities());

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
