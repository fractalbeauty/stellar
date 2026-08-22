use lofty::tag::ItemKey;
use std::collections::HashMap;
use stellar_graph::entity::{AttributeKind, EntityKind, RelationKind, ValueKind};

#[derive(Debug)]
pub struct Rules {
    pub rule: Rule,
    pub entity_key_attributes: HashMap<EntityKind, Vec<AttributeKind>>,
    pub relation_key_attributes: HashMap<RelationKind, Vec<AttributeKind>>,
}

#[derive(Debug)]
pub struct Rule {
    pub attributes: Vec<AttributeRule>,
    pub relations: Vec<RelationRule>,
}

#[derive(Debug)]
pub struct AttributeRule {
    pub attribute: AttributeKind,
    pub value: ValueKind,
    pub tag: TagKind,
}

#[derive(Debug)]
pub struct RelationRule {
    pub relation: RelationKind,
    pub other: EntityKind,
    pub direction: RelationRuleDirection,
    pub relation_attributes: Vec<AttributeRule>,
    pub other_attributes: Vec<AttributeRule>,
    pub nested_relations: Vec<RelationRule>,
}

#[derive(Debug)]
pub enum RelationRuleDirection {
    Incoming,
    Outgoing,
}

#[derive(Debug)]
pub enum TagKind {
    AlbumTitle,
    AlbumArtist,
    TrackArtist,
    TrackNumber,
    TrackTitle,
}

impl TagKind {
    pub fn to_lofty(&self) -> ItemKey {
        match self {
            TagKind::AlbumTitle => ItemKey::AlbumTitle,
            TagKind::AlbumArtist => ItemKey::AlbumArtist,
            TagKind::TrackArtist => ItemKey::TrackArtist,
            TagKind::TrackNumber => ItemKey::TrackNumber,
            TagKind::TrackTitle => ItemKey::TrackTitle,
        }
    }
}
