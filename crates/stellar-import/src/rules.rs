use lofty::tag::ItemKey;
use std::collections::HashMap;
use stellar_graph::entity::{AttributeKind, EntityKind, RelationKind, ValueKind};

#[derive(Debug, Clone, automorph::Automorph, uniffi::Record)]
pub struct Rules {
    pub rule: Rule,
    pub entity_key_attributes: HashMap<EntityKind, Vec<AttributeKind>>,
    pub relation_key_attributes: HashMap<RelationKind, Vec<AttributeKind>>,
    pub song_entity: EntityKind,
}

#[derive(Debug, Clone, automorph::Automorph, uniffi::Record)]
pub struct Rule {
    pub attributes: Vec<AttributeRule>,
    pub relations: Vec<RelationRule>,
}

#[derive(Debug, Clone, automorph::Automorph, uniffi::Record)]
pub struct AttributeRule {
    pub attribute: AttributeKind,
    pub value: ValueKind,
    pub tag: TagKind,
}

#[derive(Debug, Clone, automorph::Automorph, uniffi::Record)]
pub struct RelationRule {
    pub relation: RelationKind,
    pub other: EntityKind,
    pub direction: RelationRuleDirection,
    pub relation_attributes: Vec<AttributeRule>,
    pub other_attributes: Vec<AttributeRule>,
    pub nested_relations: Vec<RelationRule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, automorph::Automorph, uniffi::Enum)]
pub enum RelationRuleDirection {
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, automorph::Automorph, uniffi::Enum)]
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

impl Rules {
    pub fn iter_entity_key_attributes(
        &self,
        entity: EntityKind,
    ) -> impl ExactSizeIterator<Item = AttributeKind> + Clone {
        self.entity_key_attributes
            .get(&entity)
            .map(|attributes| attributes.into_iter().copied())
            .unwrap_or_default()
    }

    pub fn iter_relation_key_attributes(
        &self,
        relation: RelationKind,
    ) -> impl ExactSizeIterator<Item = AttributeKind> + Clone {
        self.relation_key_attributes
            .get(&relation)
            .map(|attributes| attributes.into_iter().copied())
            .unwrap_or_default()
    }
}
