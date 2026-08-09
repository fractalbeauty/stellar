pub mod devices;
pub mod graph;
pub mod peers;
pub mod protocol;
pub mod schema;

pub use iroh::{EndpointId, PublicKey, SecretKey};

use uuid::Uuid;

uniffi::setup_scaffolding!();

uniffi::use_remote_type!(stellar_uniffi::Uuid);
uniffi::use_remote_type!(stellar_uniffi::PublicKey);
