use siphasher::sip::SipHasher;
use std::{collections::HashMap, hash::Hasher};
use stellar_graph::entity::{AttributeKind, EntityId, Value, Version};
use uuid::Uuid;

struct EntitySymbol {
    id: EntityId,
    hash: u64,
}

impl EntitySymbol {
    fn new(
        id: EntityId,
        deleted: (bool, Version),
        attributes: HashMap<AttributeKind, (Value, Version)>,
        key0: u64,
        key1: u64,
    ) -> Self {
        let mut hasher = SipHasher::new_with_keys(key0, key1);

        // Hash deleted version
        write_version(&mut hasher, deleted.1);

        // Sort attributes by kind
        let mut sorted_attributes = attributes.iter().collect::<Vec<_>>();
        sorted_attributes.sort_by_key(|(attribute, (_value, _version))| attribute.inner());

        // Hash attributes
        for (attribute, (_value, version)) in sorted_attributes {
            hasher.write_u128(attribute.inner().as_u128());
            write_version(&mut hasher, *version);
        }

        let hash = hasher.finish();

        Self { id, hash }
    }
}

impl riblt::Symbol for EntitySymbol {
    fn zero() -> Self {
        Self {
            id: EntityId::new(Uuid::nil()),
            hash: 0,
        }
    }

    fn xor(&self, other: &Self) -> Self {
        Self {
            id: EntityId::new(Uuid::from_u128(
                self.id.inner().as_u128() ^ other.id.inner().as_u128(),
            )),
            hash: self.hash ^ other.hash,
        }
    }

    fn hash(&self) -> u64 {
        let mut hasher = SipHasher::new_with_keys(123, 456);
        hasher.write_u128(self.id.inner().as_u128());
        hasher.write_u64(self.hash);
        hasher.finish()
    }
}

fn write_version(hasher: &mut SipHasher, version: Version) {
    hasher.write_u64(version.timestamp().inner());
    hasher.write(&version.author().inner());
}
