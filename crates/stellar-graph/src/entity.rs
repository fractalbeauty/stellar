use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr, time::SystemTime};
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

impl Display for EntityKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for EntityKind {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        FromStr::from_str(s).map(Self::new)
    }
}

impl automorph::Automorph for EntityKind {
    type Changes = automorph::uuid_string::Changes;

    type Cursor = automorph::uuid_string::Cursor;

    fn save<D: automerge::transaction::Transactable + automerge::ReadDoc>(
        &self,
        doc: &mut D,
        obj: impl AsRef<automerge::ObjId>,
        prop: impl Into<automerge::Prop>,
    ) -> automorph::Result<()> {
        automorph::uuid_string::save(&self.0, doc, obj, prop)
    }

    fn load_at<D: automerge::ReadDoc>(
        doc: &D,
        obj: impl AsRef<automerge::ObjId>,
        prop: impl Into<automerge::Prop>,
        heads: &[automerge::ChangeHash],
    ) -> automorph::Result<Self> {
        Ok(Self(automorph::uuid_string::load_at(
            doc, obj, prop, heads,
        )?))
    }

    fn diff_at<D: automerge::ReadDoc>(
        &self,
        doc: &D,
        obj: impl AsRef<automerge::ObjId>,
        prop: impl Into<automerge::Prop>,
        heads: &[automerge::ChangeHash],
    ) -> automorph::Result<Self::Changes> {
        automorph::uuid_string::diff_at(&self.0, doc, obj, prop, heads)
    }

    fn load<D: automerge::ReadDoc>(
        doc: &D,
        obj: impl AsRef<automerge::ObjId>,
        prop: impl Into<automerge::Prop>,
    ) -> automorph::Result<Self> {
        Ok(Self(automorph::uuid_string::load(doc, obj, prop)?))
    }

    fn update<D: automerge::ReadDoc>(
        &mut self,
        doc: &D,
        obj: impl AsRef<automerge::ObjId>,
        prop: impl Into<automerge::Prop>,
    ) -> automorph::Result<Self::Changes> {
        automorph::uuid_string::update(&mut self.0, doc, obj, prop)
    }

    fn update_at<D: automerge::ReadDoc>(
        &mut self,
        doc: &D,
        obj: impl AsRef<automerge::ObjId>,
        prop: impl Into<automerge::Prop>,
        heads: &[automerge::ChangeHash],
    ) -> automorph::Result<Self::Changes> {
        automorph::uuid_string::update_at(&mut self.0, doc, obj, prop, heads)
    }

    fn diff<D: automerge::ReadDoc>(
        &self,
        doc: &D,
        obj: impl AsRef<automerge::ObjId>,
        prop: impl Into<automerge::Prop>,
    ) -> automorph::Result<Self::Changes> {
        automorph::uuid_string::diff(&self.0, doc, obj, prop)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationKind(Uuid);

impl RelationKind {
    /// Constructs a [`RelationKind`] from a raw UUID.
    pub fn new(inner: Uuid) -> Self {
        Self(inner)
    }

    /// Generates a random relation kind ID.
    pub fn random() -> Self {
        Self(Uuid::new_v4())
    }

    /// Returns the inner UUID.
    pub fn inner(&self) -> Uuid {
        self.0
    }
}

impl Display for RelationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for RelationKind {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        FromStr::from_str(s).map(Self::new)
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

impl Display for AttributeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.inner().fmt(f)
    }
}

impl FromStr for AttributeKind {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        FromStr::from_str(s).map(Self::new)
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    automorph::Automorph,
    uniffi::Enum,
)]
pub enum ValueKind {
    Text,
    Number,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, uniffi::Enum)]
pub enum Value {
    Text(String),
    /// Cannot be NaN or infinity.
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

    pub fn latest_version<T>(a: T, a_version: Self, b: T, b_version: Self) -> (T, Self) {
        debug_assert_ne!(a_version, b_version, "can't compare equal versions");

        let a_timestamp = a_version.timestamp().inner();
        let b_timestamp = b_version.timestamp().inner();
        let a_author = a_version.author().inner();
        let b_author = b_version.author().inner();

        if (a_timestamp, a_author) > (b_timestamp, b_author) {
            (a, a_version)
        } else {
            (b, b_version)
        }
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
            .field(&hex::encode(self.0))
            .finish()
    }
}

uniffi::custom_type!(EntityId, Vec<u8>, {
    lower: |entity| entity.inner().as_bytes().to_vec(),
    try_lift: |bytes| Ok(EntityId(Uuid::from_slice(&bytes)?)),
});
uniffi::custom_type!(EntityKind, Vec<u8>, {
    lower: |entity_kind| entity_kind.inner().as_bytes().to_vec(),
    try_lift: |bytes| Ok(EntityKind(Uuid::from_slice(&bytes)?)),
});
uniffi::custom_type!(RelationKind, Vec<u8>, {
    lower: |relation_kind| relation_kind.inner().as_bytes().to_vec(),
    try_lift: |bytes| Ok(RelationKind(Uuid::from_slice(&bytes)?)),
});
uniffi::custom_type!(AttributeKind, Vec<u8>, {
    lower: |entity_kind| entity_kind.inner().as_bytes().to_vec(),
    try_lift: |bytes| Ok(AttributeKind(Uuid::from_slice(&bytes)?)),
});

#[cfg(test)]
mod test {
    use crate::entity::{
        Version,
        hegel::{gen_author_id, gen_timestamp},
    };
    use hegel::{Generator, TestCase};

    #[hegel::test]
    fn latest_version_same_timestamp(tc: TestCase) {
        let author1 = tc.draw(gen_author_id());
        let author2 = tc.draw(gen_author_id().filter(|author| *author != author1));

        let timestamp = tc.draw(gen_timestamp());

        // Same timestamp
        let version1 = Version::new(timestamp, author1);
        let version2 = Version::new(timestamp, author2);

        let latest = Version::latest_version("version1", version1, "version2", version2);

        // Larger author ID should be latest
        if author1.inner() > author2.inner() {
            assert_eq!(latest, ("version1", version1));
        } else {
            assert_eq!(latest, ("version2", version2));
        }
    }

    #[hegel::test]
    fn latest_version_same_author(tc: TestCase) {
        let author = tc.draw(gen_author_id());

        let timestamp1 = tc.draw(gen_timestamp());
        let timestamp2 = tc.draw(gen_timestamp().filter(|timestamp| *timestamp != timestamp1));

        // Same author
        let version1 = Version::new(timestamp1, author);
        let version2 = Version::new(timestamp2, author);

        let latest = Version::latest_version("version1", version1, "version2", version2);

        // Larger timestamp should be latest
        if timestamp1.inner() > timestamp2.inner() {
            assert_eq!(latest, ("version1", version1));
        } else {
            assert_eq!(latest, ("version2", version2));
        }
    }
}

pub mod hegel {
    use crate::entity::{AttributeKind, AuthorId, EntityId, EntityKind, Timestamp, Value, Version};
    use hegel::{TestCase, compose, generators as gs, one_of};
    use uuid::Uuid;

    #[hegel::composite]
    pub fn gen_version(tc: TestCase) -> Version {
        Version::new(tc.draw(gen_timestamp()), tc.draw(gen_author_id()))
    }

    #[hegel::composite]
    pub fn gen_timestamp(tc: TestCase) -> Timestamp {
        Timestamp::new(tc.draw(gs::integers()))
    }

    #[hegel::composite]
    pub fn gen_author_id(tc: TestCase) -> AuthorId {
        AuthorId::new(
            tc.draw(gs::vecs(gs::integers()).min_size(32).max_size(32))
                .try_into()
                .unwrap(),
        )
    }

    #[hegel::composite]
    pub fn gen_entity_id(tc: TestCase) -> EntityId {
        EntityId::new(tc.draw(gen_uuid()))
    }

    #[hegel::composite]
    pub fn gen_entity_kind(tc: TestCase) -> EntityKind {
        EntityKind::new(tc.draw(gen_uuid()))
    }

    #[hegel::composite]
    pub fn gen_attribute_kind(tc: TestCase) -> AttributeKind {
        AttributeKind::new(tc.draw(gen_uuid()))
    }

    #[hegel::composite]
    pub fn gen_uuid(tc: TestCase) -> Uuid {
        Uuid::from_u128(tc.draw(gs::integers().min_value(1)))
    }

    #[hegel::composite]
    pub fn gen_value(tc: TestCase) -> Value {
        tc.draw(one_of!(
            compose!(|tc| { Value::Text(tc.draw(gs::text())) }),
            compose!(|tc| {
                Value::Number(tc.draw(gs::floats().allow_nan(false).allow_infinity(false)))
            }),
        ))
    }
}
