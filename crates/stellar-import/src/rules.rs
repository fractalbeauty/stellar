use lofty::tag::ItemKey;
use stellar_graph::entity::{AttributeKind, EntityKind, RelationKind, ValueKind};

#[derive(Debug)]
pub struct Rules {
    pub rule: Rule,
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
    pub relation_key_attributes: Vec<AttributeRule>,
    pub relation_extra_attributes: Vec<AttributeRule>,
    pub other_key_attributes: Vec<AttributeRule>,
    pub other_extra_attributes: Vec<AttributeRule>,
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
