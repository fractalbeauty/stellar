use crate::entity::{AttributeKind, EntityKind, RelationKind, ValueKind};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Schema {
    pub entities: HashMap<EntityKind, EntitySchema>,
    pub relations: HashMap<RelationKind, RelationSchema>,
}

#[derive(Debug)]
pub struct EntitySchema {
    pub name: String,
    pub attributes: HashMap<AttributeKind, AttributeSchema>,
}

#[derive(Debug)]
pub struct RelationSchema {
    pub name: String,
    pub attributes: HashMap<AttributeKind, AttributeSchema>,
}

#[derive(Debug)]
pub struct AttributeSchema {
    pub name: String,
    pub value: ValueKind,
}
