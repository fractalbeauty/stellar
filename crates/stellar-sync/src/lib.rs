pub mod devices;
pub mod graph;
pub mod peers;
pub mod protocol;
pub mod schema;

pub use iroh::{EndpointId, PublicKey, SecretKey};

use uuid::Uuid;

uniffi::setup_scaffolding!();

uniffi::custom_type!(Uuid, Vec<u8>, { remote });
uniffi::custom_type!(PublicKey, Vec<u8>, {
   remote,
   lower: |public_key| public_key.to_vec(),
   try_lift: |bytes| Ok(PublicKey::from_bytes(bytes.as_slice().try_into()?)?),
});
