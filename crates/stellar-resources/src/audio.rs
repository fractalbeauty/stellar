use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
use stellar_graph::{
    entity::{AttributeKind, AuthorId, EntityKind, Value, ValueKind, Version},
    schema::{AttributeSchema, EntitySchema},
    store::EntityAttributeValue,
};

pub const AUDIO_RESOURCE_ENTITY: EntityKind = EntityKind::new_reserved(1u8);
pub const AUDIO_RESOURCE_PROVIDER: AttributeKind = AttributeKind::from_bytes([1u8, 0, 0, 0, 0]); // reserved
pub const AUDIO_RESOURCE_LOCATION: AttributeKind = AttributeKind::from_bytes([2u8, 0, 0, 0, 0]);
pub const AUDIO_RESOURCE_HASH: AttributeKind = AttributeKind::from_bytes([3u8, 0, 0, 0, 0]);
pub const AUDIO_RESOURCE_SIZE: AttributeKind = AttributeKind::from_bytes([4u8, 0, 0, 0, 0]);
pub const AUDIO_RESOURCE_QUALITY: AttributeKind = AttributeKind::from_bytes([5u8, 0, 0, 0, 0]);
pub const AUDIO_RESOURCE_DURATION: AttributeKind = AttributeKind::from_bytes([6u8, 0, 0, 0, 0]);

#[derive(Debug, Clone, PartialEq)]
pub struct AudioResource {
    pub location: AudioResourceLocation,
    pub hash: [u8; 32],
    pub size: u64,
    pub quality: AudioResourceQuality,
    pub duration: f64,

    pub location_version: Version,
    pub hash_version: Version,
    pub size_version: Version,
    pub quality_version: Version,
    pub duration_version: Version,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioResourceLocation {
    pub device: AuthorId,
    pub path: PathBuf,
}

impl AudioResourceLocation {
    pub fn encode(value: &Self) -> Vec<u8> {
        postcard::to_stdvec(&value)
            .expect("Failed to serialize AudioResourceLocation")
            .into()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        postcard::from_bytes(bytes)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize AudioResourceLocation: {e:?}"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioResourceQuality {
    // TODO
}

impl AudioResourceQuality {
    pub fn encode(value: &Self) -> Vec<u8> {
        postcard::to_stdvec(&value)
            .expect("Failed to serialize AudioResourceQuality")
            .into()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        postcard::from_bytes(bytes)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize AudioResourceQuality: {e:?}"))
    }
}

impl AudioResource {
    pub fn try_from_attributes(
        attributes: &HashMap<AttributeKind, EntityAttributeValue>,
    ) -> Result<Self, anyhow::Error> {
        let (location, location_version) = get_bytes_attribute(attributes, AUDIO_RESOURCE_LOCATION)
            .ok_or_else(|| {
                anyhow::anyhow!("AudioResource.location is missing or wrong value kind")
            })?;
        let location = AudioResourceLocation::decode(location)?;

        let (hash, hash_version) = get_bytes_attribute(attributes, AUDIO_RESOURCE_HASH)
            .ok_or_else(|| anyhow::anyhow!("AudioResource.hash is missing or wrong value kind"))?;
        let hash = hash.try_into().map_err(|_| {
            anyhow::anyhow!("Failed to deserialize AudioResource.hash: wrong length")
        })?;

        let (size, size_version) = get_number_attribute(attributes, AUDIO_RESOURCE_SIZE)
            .ok_or_else(|| anyhow::anyhow!("AudioResource.size is missing or wrong value kind"))?;
        let size = size as u64;

        let (quality, quality_version) = get_bytes_attribute(attributes, AUDIO_RESOURCE_QUALITY)
            .ok_or_else(|| {
                anyhow::anyhow!("AudioResource.quality is missing or wrong value kind")
            })?;
        let quality = AudioResourceQuality::decode(quality)?;

        let (duration, duration_version) =
            get_number_attribute(attributes, AUDIO_RESOURCE_DURATION).ok_or_else(|| {
                anyhow::anyhow!("AudioResource.duration is missing or wrong value kind")
            })?;

        Ok(Self {
            location,
            hash,
            size,
            quality,
            duration,

            location_version,
            hash_version,
            size_version,
            quality_version,
            duration_version,
        })
    }

    pub fn to_attributes(&self) -> HashMap<AttributeKind, EntityAttributeValue> {
        HashMap::from([
            (
                AUDIO_RESOURCE_LOCATION,
                EntityAttributeValue {
                    value: Value::Bytes(AudioResourceLocation::encode(&self.location)),
                    version: self.location_version,
                },
            ),
            (
                AUDIO_RESOURCE_HASH,
                EntityAttributeValue {
                    value: Value::Bytes(self.hash.to_vec()),
                    version: self.hash_version,
                },
            ),
            (
                AUDIO_RESOURCE_SIZE,
                EntityAttributeValue {
                    value: Value::number_from_f64(self.size as f64),
                    version: self.size_version,
                },
            ),
            (
                AUDIO_RESOURCE_QUALITY,
                EntityAttributeValue {
                    value: Value::Bytes(AudioResourceQuality::encode(&self.quality)),
                    version: self.quality_version,
                },
            ),
            (
                AUDIO_RESOURCE_DURATION,
                EntityAttributeValue {
                    value: Value::number_from_f64(self.duration as f64),
                    version: self.duration_version,
                },
            ),
        ])
    }
}

pub fn audio_resource_schema() -> EntitySchema {
    EntitySchema {
        name: "Audio Resource".to_string(),
        attributes: HashMap::from([
            (
                AUDIO_RESOURCE_LOCATION,
                AttributeSchema {
                    name: "Location".to_string(),
                    value: ValueKind::Bytes,
                },
            ),
            (
                AUDIO_RESOURCE_HASH,
                AttributeSchema {
                    name: "Hash".to_string(),
                    value: ValueKind::Bytes,
                },
            ),
            (
                AUDIO_RESOURCE_SIZE,
                AttributeSchema {
                    name: "Size".to_string(),
                    value: ValueKind::Number,
                },
            ),
            (
                AUDIO_RESOURCE_QUALITY,
                AttributeSchema {
                    name: "Quality".to_string(),
                    value: ValueKind::Bytes,
                },
            ),
            (
                AUDIO_RESOURCE_DURATION,
                AttributeSchema {
                    name: "Duration".to_string(),
                    value: ValueKind::Number,
                },
            ),
        ]),
    }
}

fn get_number_attribute(
    attributes: &HashMap<AttributeKind, EntityAttributeValue>,
    attribute: AttributeKind,
) -> Option<(f64, Version)> {
    let EntityAttributeValue { value, version } = attributes.get(&attribute)?;
    let Value::Number(value) = value else {
        return None;
    };
    Some((**value, *version))
}

fn get_bytes_attribute(
    attributes: &HashMap<AttributeKind, EntityAttributeValue>,
    attribute: AttributeKind,
) -> Option<(&[u8], Version)> {
    let EntityAttributeValue { value, version } = attributes.get(&attribute)?;
    let Value::Bytes(value) = value else {
        return None;
    };
    Some((value.as_slice(), *version))
}

#[cfg(test)]
mod test {
    use crate::audio::{AudioResource, hegel::gen_audio_resource};
    use hegel::TestCase;

    #[hegel::test]
    fn audio_resource_roundtrip(tc: TestCase) {
        let original = tc.draw(gen_audio_resource());

        let attributes = original.to_attributes();
        let parsed = AudioResource::try_from_attributes(&attributes).expect("should parse");

        assert_eq!(original, parsed);
    }
}

pub mod hegel {
    use crate::audio::{AudioResource, AudioResourceLocation, AudioResourceQuality};
    use hegel::{TestCase, generators as gs};
    use stellar_graph::entity::hegel::{gen_author_id, gen_value_number, gen_version};

    #[hegel::composite]
    pub fn gen_audio_resource(tc: TestCase) -> AudioResource {
        let size = tc.draw(gs::integers());

        // Very large sizes aren't roundtripped correctly since they're converted to floats.
        // Assume each resource is smaller than 1 TB.
        tc.assume(size < 1_000_000_000_000);

        AudioResource {
            location: tc.draw(gen_audio_resource_location()),
            hash: tc.draw(gs::arrays(gs::integers())),
            size,
            quality: tc.draw(gen_audio_resource_quality()),
            duration: tc.draw(gen_value_number()),

            location_version: tc.draw(gen_version()),
            hash_version: tc.draw(gen_version()),
            size_version: tc.draw(gen_version()),
            quality_version: tc.draw(gen_version()),
            duration_version: tc.draw(gen_version()),
        }
    }

    #[hegel::composite]
    pub fn gen_audio_resource_location(tc: TestCase) -> AudioResourceLocation {
        AudioResourceLocation {
            device: tc.draw(gen_author_id()),
            path: tc.draw(gs::text()).into(), // TODO: generate paths instead of text
        }
    }

    #[hegel::composite]
    pub fn gen_audio_resource_quality(tc: TestCase) -> AudioResourceQuality {
        AudioResourceQuality {}
    }
}
