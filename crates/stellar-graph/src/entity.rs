use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId(Uuid);

impl EntityId {
    /// Constructs an [`EntityId`] from a raw UUID.
    pub fn new(inner: Uuid) -> Self {
        Self(inner)
    }

    /// Generates a random entity ID.
    pub fn random() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the inner UUID.
    pub fn inner(&self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityKind(Uuid);

impl EntityKind {
    /// Constructs an [`EntityKind`] from a raw UUID.
    pub fn new(inner: Uuid) -> Self {
        Self(inner)
    }

    /// Generates a random entity kind ID.
    pub fn random() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the inner UUID.
    pub fn inner(&self) -> Uuid {
        self.0
    }
}

/// `a` must be less than `b` for uniqueness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationKind {
    a: EntityKind,
    b: EntityKind,
}

impl RelationKind {
    pub fn new(a: EntityKind, b: EntityKind) -> Self {
        if a.0 < b.0 {
            Self { a, b }
        } else {
            Self { a: b, b: a }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AttributeKind(Uuid);

impl AttributeKind {
    /// Constructs an [`AttributeKind`] from a raw UUID.
    pub fn new(inner: Uuid) -> Self {
        Self(inner)
    }

    /// Generates a random attribute kind ID.
    pub fn random() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the inner UUID.
    pub fn inner(&self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValueKind {
    Text,
    Number,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Text(String),
    Number(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Version {
    timestamp: Timestamp,
    author: AuthorId,
}

impl Version {
    pub fn new(timestamp: Timestamp, author: AuthorId) -> Self {
        Self { timestamp, author }
    }

    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    pub fn author(&self) -> AuthorId {
        self.author
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Constructs a [`Timestamp`] from a raw u64.
    pub fn new(inner: u64) -> Self {
        Self(inner)
    }

    pub fn now() -> Self {
        let now_millis = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("System time should be after Unix epoch")
            .as_millis();
        Self(now_millis.try_into().unwrap_or(u64::MAX))
    }

    /// Returns the inner milliseconds since the Unix epoch.
    pub fn inner(&self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AuthorId([u8; 32]);

impl AuthorId {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the inner bytes.
    pub fn inner(&self) -> [u8; 32] {
        self.0
    }
}

impl std::fmt::Debug for AuthorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AuthorId")
            .field(&hex::encode(&self.0))
            .finish()
    }
}
