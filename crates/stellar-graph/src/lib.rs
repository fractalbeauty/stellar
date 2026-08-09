pub mod database;
pub mod entity;
pub mod schema;
pub mod store;

use uuid::Uuid;

uniffi::setup_scaffolding!();

uniffi::custom_type!(Uuid, Vec<u8>, { remote });
