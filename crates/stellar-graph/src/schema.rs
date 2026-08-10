use crate::entity::{AttributeKind, EntityKind, RelationKind, ValueKind};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, automorph::Automorph, uniffi::Record)]
pub struct Schema {
    pub entities: HashMap<EntityKind, EntitySchema>,
    pub relations: HashMap<RelationKind, RelationSchema>,
}

#[derive(Debug, Clone, automorph::Automorph, uniffi::Record)]
pub struct EntitySchema {
    pub name: String,
    pub attributes: HashMap<AttributeKind, AttributeSchema>,
}

#[derive(Debug, Clone, automorph::Automorph, uniffi::Record)]
pub struct RelationSchema {
    pub name: String,
    pub attributes: HashMap<AttributeKind, AttributeSchema>,
}

#[derive(Debug, Clone, automorph::Automorph, uniffi::Record)]
pub struct AttributeSchema {
    pub name: String,
    pub value: ValueKind,
}
