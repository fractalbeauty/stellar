//! Shared custom remote UniFFI types.
//!
//! The same `custom_type!` defined in multiple crates will conflict with each other.
//! Instead, we need to define them all here and use `use_remote_type!(stellar_uniffi::X)`.
//!
//! Also note that `custom_newtype!` doesn't work with types included by `use_remote_type!`.

use iroh::PublicKey;
use uuid::Uuid;

uniffi::setup_scaffolding!();

uniffi::custom_type!(Uuid, Vec<u8>, { remote });
uniffi::custom_type!(PublicKey, Vec<u8>, {
   remote,
   lower: |public_key| public_key.to_vec(),
   try_lift: |bytes| Ok(PublicKey::from_bytes(bytes.as_slice().try_into()?)?),
});
