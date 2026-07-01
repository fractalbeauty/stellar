use std::time::SystemTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId(Uuid);

impl EntityId {
    /// Generates a new entity ID.
    pub fn random() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn inner(&self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityKind(Uuid);

impl EntityKind {
    /// Generates a new entity kind ID.
    pub fn random() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn inner(&self) -> Uuid {
        self.0
    }
}

/// `a` must be less than `b` for uniqueness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttributeKind(Uuid);

impl AttributeKind {
    /// Generates a new attribute kind ID.
    pub fn random() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueKind {
    Text,
    Number,
}

#[derive(Debug, Clone)]
pub enum Value {
    Text(String),
    Number(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Version {
    timestamp: Timestamp,
    author: AuthorId,
}

impl Version {
    pub fn new(timestamp: Timestamp, author: AuthorId) -> Self {
        Self { timestamp, author }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timestamp(u64);

impl Timestamp {
    pub fn now() -> Self {
        let now_millis = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("System time should be after Unix epoch")
            .as_millis();
        Self(now_millis.try_into().unwrap_or(u64::MAX))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AuthorId([u8; 32]);

impl AuthorId {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

pub trait Store {
    fn create_entity(&self, kind: EntityKind, version: Version) -> Result<EntityId, StoreError>;

    // fn delete_entity()

    fn get_entities(&self) -> Result<Vec<EntityId>, StoreError>;

    fn get_entity_attribute(
        &self,
        entity: EntityId,
        attribute: AttributeKind,
    ) -> Result<Option<Value>, StoreError>;

    fn set_entity_attribute(
        &self,
        entity: EntityId,
        attribute: AttributeKind,
        value: Value,
        version: Version,
    ) -> Result<(), StoreError>;

    fn create_relation(&self, a: EntityId, b: EntityId, version: Version)
    -> Result<(), StoreError>;

    // fn delete_relation

    fn get_relation_attribute(
        &self,
        a: EntityId,
        b: EntityId,
        attribute: AttributeKind,
    ) -> Result<Option<Value>, StoreError>;

    fn set_relation_attribute(
        &self,
        a: EntityId,
        b: EntityId,
        attribute: AttributeKind,
        value: Value,
        version: Version,
    ) -> Result<(), StoreError>;
}

// TODO
#[derive(Debug)]
pub struct StoreError {}
