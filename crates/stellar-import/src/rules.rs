use lofty::tag::ItemKey;
use stellar_graph::entity::{AttributeKind, ValueKind};

#[derive(Debug)]
pub struct Rules {
    pub rules: Vec<Rule>,
}

#[derive(Debug)]
pub enum Rule {
    TagRule(TagRule),
}

#[derive(Debug)]
pub struct TagRule {
    pub attribute: AttributeKind,
    pub value: ValueKind,
    pub tag: TagKind,
}

#[derive(Debug)]
pub enum TagKind {
    TrackTitle,
}

impl TagKind {
    pub fn to_lofty(&self) -> ItemKey {
        match self {
            TagKind::TrackTitle => ItemKey::TrackTitle,
        }
    }
}
