pub mod database;
pub mod entity;
pub mod query;
pub mod schema;
pub mod store;

use uuid::Uuid;

uniffi::setup_scaffolding!();

uniffi::use_remote_type!(stellar_uniffi::Uuid);
