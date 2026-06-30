use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityId(Uuid);

impl EntityId {
    /// Generates a new entity ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityKind(Uuid);

impl EntityKind {
    /// Generates a new entity kind ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttributeId(Uuid);

impl AttributeId {
    /// Generates a new attribute ID.
    pub fn new() -> Self {
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

pub trait Store {
    fn create_entity(&self, kind: EntityKind) -> Result<EntityId, StoreError>;

    // fn delete_entity()

    fn get_entities(&self) -> Result<Vec<EntityId>, StoreError>;

    fn get_entity_attribute(
        &self,
        entity: EntityId,
        attribute: AttributeId,
    ) -> Result<Option<Value>, StoreError>;

    fn set_entity_attribute(
        &self,
        entity: EntityId,
        attribute: AttributeId,
        value: Value,
    ) -> Result<(), StoreError>;
}

// TODO
#[derive(Debug)]
pub struct StoreError {}
