use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr, time::SystemTime};

/// An entity ID, consisting of an [`EntityKind`] and some random bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityId([u8; 16]);

impl EntityId {
    /// Constructs an [`EntityId`] from a byte array.
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Constructs an [`EntityId`] from a byte slice.
    pub fn from_slice(bytes: &[u8; 16]) -> Self {
        Self(*bytes)
    }

    /// Generates a new random [`EntityId`] with the given [`EntityKind`].
    pub fn random(kind: EntityKind) -> Self {
        let random: [u8; 11] = rand::random();

        let mut bytes = [0u8; 16];
        bytes[0..5].copy_from_slice(&kind.as_bytes());
        bytes[5..16].copy_from_slice(&random);

        Self::from_bytes(bytes)
    }

    /// Returns the [`EntityId`] as a byte array.
    pub fn as_bytes(&self) -> [u8; 16] {
        self.0
    }

    /// Returns the [`EntityId`] as a byte slice.
    pub fn as_slice(&self) -> &[u8; 16] {
        &self.0
    }

    /// Returns the [`EntityKind`] of this [`EntityId`].
    pub fn kind(&self) -> EntityKind {
        EntityKind::from_bytes(self.0[0..5].try_into().unwrap())
    }
}

impl std::fmt::Debug for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("EntityId")
            .field(&hex::encode(self.0))
            .finish()
    }
}

impl Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl FromStr for EntityId {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 16];
        hex::decode_to_slice(s.as_bytes(), &mut bytes)?;
        Ok(Self(bytes))
    }
}

/// A relation ID, consisting of a [`RelationKind`] and some random bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationId([u8; 16]);

impl RelationId {
    /// Constructs a [`RelationId`] from a byte array.
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Constructs a [`RelationId`] from a byte slice.
    pub fn from_slice(bytes: &[u8; 16]) -> Self {
        Self(*bytes)
    }

    /// Generates a new random [`RelationId`] with the given [`RelationKind`].
    pub fn random(kind: RelationKind) -> Self {
        let random: [u8; 11] = rand::random();

        let mut bytes = [0u8; 16];
        bytes[0..5].copy_from_slice(&kind.as_bytes());
        bytes[5..16].copy_from_slice(&random);

        Self::from_bytes(bytes)
    }

    /// Returns the [`RelationId`] as a byte array.
    pub fn as_bytes(&self) -> [u8; 16] {
        self.0
    }

    /// Returns the [`RelationId`] as a byte slice.
    pub fn as_slice(&self) -> &[u8; 16] {
        &self.0
    }

    /// Returns the [`RelationKind`] of this [`RelationId`].
    pub fn kind(&self) -> RelationKind {
        RelationKind::from_bytes(self.0[0..5].try_into().unwrap())
    }
}

impl std::fmt::Debug for RelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RelationId")
            .field(&hex::encode(self.0))
            .finish()
    }
}

impl Display for RelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl FromStr for RelationId {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 16];
        hex::decode_to_slice(s.as_bytes(), &mut bytes)?;
        Ok(Self(bytes))
    }
}

/// A kind of entity in the graph, identified by 5 random bytes.
///
/// The pattern `XX 00 00 00 00` is reserved for application-defined kinds.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, automorph::Automorph)]
#[automorph(transparent)]
pub struct EntityKind([u8; 5]);

impl EntityKind {
    /// Constructs an [`EntityKind`] from a byte array.
    pub fn from_bytes(bytes: [u8; 5]) -> Self {
        Self(bytes)
    }

    /// Constructs an [`EntityKind`] from a byte slice.
    pub fn from_slice(bytes: &[u8; 5]) -> Self {
        Self(*bytes)
    }

    /// Constructs a reserved [`EntityKind`] from a byte.
    pub const fn new_reserved(byte: u8) -> Self {
        Self([byte, 0, 0, 0, 0])
    }

    /// Generates a new random [`EntityKind`].
    ///
    /// The kind is guaranteed not to be reserved.
    pub fn random() -> Self {
        let kind = Self(rand::random());
        if kind.is_reserved() {
            Self::random()
        } else {
            kind
        }
    }

    /// Returns whether the [`EntityKind`] is a reserved kind.
    pub fn is_reserved(self) -> bool {
        self.as_slice()[1..5].iter().all(|byte| *byte == 0)
    }

    /// Returns the [`EntityKind`] as a byte array.
    pub fn as_bytes(&self) -> [u8; 5] {
        self.0
    }

    /// Returns the [`EntityKind`] as a byte slice.
    pub fn as_slice(&self) -> &[u8; 5] {
        &self.0
    }
}

impl std::fmt::Debug for EntityKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("EntityKind")
            .field(&hex::encode(self.0))
            .finish()
    }
}

impl Display for EntityKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl FromStr for EntityKind {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 5];
        hex::decode_to_slice(s.as_bytes(), &mut bytes)?;
        Ok(Self(bytes))
    }
}

/// A kind of relation in the graph, identified by 5 random bytes.
///
/// The pattern `XX 00 00 00 00` is reserved for application-defined kinds.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, automorph::Automorph)]
#[automorph(transparent)]
pub struct RelationKind([u8; 5]);

impl RelationKind {
    /// Constructs a [`RelationKind`] from a byte array.
    pub fn from_bytes(bytes: [u8; 5]) -> Self {
        Self(bytes)
    }

    /// Constructs a [`RelationKind`] from a byte slice.
    pub fn from_slice(bytes: &[u8; 5]) -> Self {
        Self(*bytes)
    }

    /// Constructs a reserved [`RelationKind`] from a byte.
    pub const fn new_reserved(byte: u8) -> Self {
        Self([byte, 0, 0, 0, 0])
    }

    /// Generates a new random [`RelationKind`].
    ///
    /// The kind is guaranteed not to be reserved.
    pub fn random() -> Self {
        let kind = Self(rand::random());
        if kind.is_reserved() {
            Self::random()
        } else {
            kind
        }
    }

    /// Returns whether the [`RelationKind`] is a reserved kind.
    pub fn is_reserved(self) -> bool {
        self.as_slice()[1..5].iter().all(|byte| *byte == 0)
    }

    /// Returns the [`RelationKind`] as a byte array.
    pub fn as_bytes(&self) -> [u8; 5] {
        self.0
    }

    /// Returns the [`RelationKind`] as a byte slice.
    pub fn as_slice(&self) -> &[u8; 5] {
        &self.0
    }
}

impl std::fmt::Debug for RelationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RelationKind")
            .field(&hex::encode(self.0))
            .finish()
    }
}

impl Display for RelationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl FromStr for RelationKind {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 5];
        hex::decode_to_slice(s.as_bytes(), &mut bytes)?;
        Ok(Self(bytes))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, automorph::Automorph)]
#[automorph(transparent)]
pub struct AttributeKind([u8; 5]);

impl AttributeKind {
    /// Constructs an [`AttributeKind`] from a byte array.
    pub const fn from_bytes(bytes: [u8; 5]) -> Self {
        Self(bytes)
    }

    /// Constructs an [`AttributeKind`] from a byte slice.
    pub fn from_slice(bytes: &[u8; 5]) -> Self {
        Self(*bytes)
    }

    /// Generates a new random [`AttributeKind`].
    pub fn random() -> Self {
        Self(rand::random())
    }

    /// Returns the [`AttributeKind`] as a byte array.
    pub fn as_bytes(&self) -> [u8; 5] {
        self.0
    }

    /// Returns the [`AttributeKind`] as a byte slice.
    pub fn as_slice(&self) -> &[u8; 5] {
        &self.0
    }
}

impl std::fmt::Debug for AttributeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AttributeKind")
            .field(&hex::encode(self.0))
            .finish()
    }
}

impl Display for AttributeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl FromStr for AttributeKind {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 5];
        hex::decode_to_slice(s.as_bytes(), &mut bytes)?;
        Ok(Self(bytes))
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
    Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, uniffi::Enum)]
pub enum Value {
    Text(String),
    /// Cannot be NaN or infinity.
    Number(OrderedFloat<f64>),
    Bool(bool),
    // TODO: maybe Arc<[u8]>
    Bytes(Vec<u8>),
}

impl Value {
    pub fn number_from_f64(value: f64) -> Self {
        Self::Number(value.into())
    }
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

        if a_version.greater_than(b_version) {
            (a, a_version)
        } else {
            (b, b_version)
        }
    }

    pub fn greater_than(self, other: Self) -> bool {
        debug_assert_ne!(self, other, "can't compare equal versions");

        let a_timestamp = self.timestamp().inner();
        let b_timestamp = other.timestamp().inner();
        let a_author = self.author().as_bytes();
        let b_author = other.author().as_bytes();

        (a_timestamp, a_author) > (b_timestamp, b_author)
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
    /// Constructs an [`AuthorId`] from a byte array.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Constructs an [`AuthorId`] from a byte slice.
    pub fn from_slice(bytes: &[u8; 32]) -> Self {
        Self(*bytes)
    }

    /// Returns the [`AuthorId`] as a byte array.
    pub fn as_bytes(&self) -> [u8; 32] {
        self.0
    }

    /// Returns the [`AuthorId`] as a byte slice.
    pub fn as_slice(&self) -> &[u8; 32] {
        &self.0
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
    lower: |entity| entity.as_bytes().to_vec(),
    try_lift: |bytes| Ok(EntityId::from_bytes(bytes.try_into().map_err(|_| anyhow::anyhow!("Failed to lift EntityId"))?)),
});
uniffi::custom_type!(RelationId, Vec<u8>, {
    lower: |relation| relation.as_bytes().to_vec(),
    try_lift: |bytes| Ok(RelationId::from_bytes(bytes.try_into().map_err(|_| anyhow::anyhow!("Failed to lift RelationId"))?)),
});
uniffi::custom_type!(EntityKind, Vec<u8>, {
    lower: |entity_kind| entity_kind.as_bytes().to_vec(),
    try_lift: |bytes| Ok(EntityKind::from_bytes(bytes.try_into().map_err(|_| anyhow::anyhow!("Failed to lift EntityKind"))?)),
});
uniffi::custom_type!(RelationKind, Vec<u8>, {
    lower: |relation_kind| relation_kind.as_bytes().to_vec(),
    try_lift: |bytes| Ok(RelationKind::from_bytes(bytes.try_into().map_err(|_| anyhow::anyhow!("Failed to lift RelationKind"))?)),
});
uniffi::custom_type!(AttributeKind, Vec<u8>, {
    lower: |attribute_kind| attribute_kind.as_bytes().to_vec(),
    try_lift: |bytes| Ok(AttributeKind::from_bytes(bytes.try_into().map_err(|_| anyhow::anyhow!("Failed to lift AttributeKind"))?)),
});

type OrderedFloatF64 = OrderedFloat<f64>;
uniffi::custom_type!(OrderedFloatF64, f64, {
    remote,
    lower: |ordered_float| ordered_float.into_inner(),
    try_lift: |float| Ok(float.into()),
});

#[cfg(test)]
mod test {
    use crate::entity::{
        AttributeKind, EntityId, EntityKind, RelationId, RelationKind, Version,
        hegel::{gen_author_id, gen_timestamp},
    };
    use hegel::{Generator, TestCase, generators as gs};

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
        if author1.as_bytes() > author2.as_bytes() {
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

    #[test]
    fn random() {
        let entity = EntityKind::random();
        EntityId::random(entity);

        let relation = RelationKind::random();
        RelationId::random(relation);

        AttributeKind::random();
    }

    #[test]
    fn reserved_kinds() {
        assert!(EntityKind::from_bytes([00, 00, 00, 00, 00]).is_reserved());
        assert!(EntityKind::from_bytes([01, 00, 00, 00, 00]).is_reserved());
        assert!(!EntityKind::from_bytes([00, 01, 00, 00, 00]).is_reserved());
        assert!(!EntityKind::from_bytes([01, 01, 00, 00, 00]).is_reserved());

        assert!(RelationKind::from_bytes([00, 00, 00, 00, 00]).is_reserved());
        assert!(RelationKind::from_bytes([01, 00, 00, 00, 00]).is_reserved());
        assert!(!RelationKind::from_bytes([00, 01, 00, 00, 00]).is_reserved());
        assert!(!RelationKind::from_bytes([01, 01, 00, 00, 00]).is_reserved());
    }

    #[hegel::test]
    fn new_reserved_is_reserved(tc: TestCase) {
        let kind = EntityKind::new_reserved(tc.draw(gs::integers()));
        assert!(kind.is_reserved())
    }
}

pub mod hegel {
    use crate::entity::{
        AttributeKind, AuthorId, EntityId, EntityKind, RelationId, RelationKind, Timestamp, Value,
        Version,
    };
    use hegel::{
        TestCase, compose,
        generators::{self as gs},
        one_of,
    };
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
        AuthorId::from_bytes(
            tc.draw(gs::vecs(gs::integers()).min_size(32).max_size(32))
                .as_slice()
                .try_into()
                .unwrap(),
        )
    }

    #[hegel::composite]
    pub fn gen_entity_id(tc: TestCase) -> EntityId {
        EntityId::from_bytes(tc.draw(gs::arrays(gs::integers())))
    }

    #[hegel::composite]
    pub fn gen_relation_id(tc: TestCase) -> RelationId {
        RelationId::from_bytes(tc.draw(gs::arrays(gs::integers())))
    }

    #[hegel::composite]
    pub fn gen_entity_kind(tc: TestCase) -> EntityKind {
        EntityKind::from_bytes(tc.draw(gs::arrays(gs::integers())))
    }

    #[hegel::composite]
    pub fn gen_relation_kind(tc: TestCase) -> RelationKind {
        RelationKind::from_bytes(tc.draw(gs::arrays(gs::integers())))
    }

    #[hegel::composite]
    pub fn gen_attribute_kind(tc: TestCase) -> AttributeKind {
        AttributeKind::from_bytes(tc.draw(gs::arrays(gs::integers())))
    }

    #[hegel::composite]
    pub fn gen_uuid(tc: TestCase) -> Uuid {
        Uuid::from_u128(tc.draw(gs::integers().min_value(1)))
    }

    #[hegel::composite]
    pub fn gen_value(tc: TestCase) -> Value {
        tc.draw(one_of!(
            compose!(|tc| { Value::Text(tc.draw(gs::text())) }),
            compose!(|tc| { Value::Number(tc.draw(gen_value_number()).into()) }),
            compose!(|tc| { Value::Bool(tc.draw(gs::booleans()).into()) }),
        ))
    }

    #[hegel::composite]
    pub fn gen_value_number(tc: TestCase) -> f64 {
        tc.draw(gs::floats().allow_nan(false).allow_infinity(false))
    }
}
