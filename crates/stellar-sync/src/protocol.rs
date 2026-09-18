use crate::connection::Message;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub enum StreamHeader {
    Sync,
    Difference,
    SchemaSync,
    Incremental,
    Resource,
}

impl Message for StreamHeader {}
