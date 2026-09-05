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
pub const AUDIO_RESOURCE_DURATION: AttributeKind = AttributeKind::from_bytes([5u8, 0, 0, 0, 0]);
pub const AUDIO_RESOURCE_CODEC: AttributeKind = AttributeKind::from_bytes([6u8, 0, 0, 0, 0]);
pub const AUDIO_RESOURCE_BITRATE: AttributeKind = AttributeKind::from_bytes([7u8, 0, 0, 0, 0]);
pub const AUDIO_RESOURCE_SAMPLE_RATE: AttributeKind = AttributeKind::from_bytes([8u8, 0, 0, 0, 0]);
pub const AUDIO_RESOURCE_BIT_DEPTH: AttributeKind = AttributeKind::from_bytes([9u8, 0, 0, 0, 0]);
pub const AUDIO_RESOURCE_CHANNELS: AttributeKind = AttributeKind::from_bytes([10u8, 0, 0, 0, 0]);

#[derive(Debug, Clone, PartialEq)]
pub struct AudioResource {
    pub location: AudioResourceLocation,
    pub hash: [u8; 32],
    pub size: u64,
    pub duration: f64,
    pub codec: String,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub bit_depth: u8,
    pub channels: u8,

    pub location_version: Version,
    pub hash_version: Version,
    pub size_version: Version,
    pub duration_version: Version,
    pub codec_version: Version,
    pub bitrate_version: Version,
    pub sample_rate_version: Version,
    pub bit_depth_version: Version,
    pub channels_version: Version,
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
        let size: u64 = size as u64;

        let (duration, duration_version) =
            get_number_attribute(attributes, AUDIO_RESOURCE_DURATION).ok_or_else(|| {
                anyhow::anyhow!("AudioResource.duration is missing or wrong value kind")
            })?;

        let (codec, codec_version) = get_text_attribute(attributes, AUDIO_RESOURCE_CODEC)
            .ok_or_else(|| anyhow::anyhow!("AudioResource.codec is missing or wrong value kind"))?;

        let (bitrate, bitrate_version) = get_number_attribute(attributes, AUDIO_RESOURCE_BITRATE)
            .ok_or_else(|| {
            anyhow::anyhow!("AudioResource.bitrate is missing or wrong value kind")
        })?;
        let bitrate = bitrate as u32;

        let (sample_rate, sample_rate_version) =
            get_number_attribute(attributes, AUDIO_RESOURCE_SAMPLE_RATE).ok_or_else(|| {
                anyhow::anyhow!("AudioResource.sample_rate is missing or wrong value kind")
            })?;
        let sample_rate = sample_rate as u32;

        let (bit_depth, bit_depth_version) =
            get_number_attribute(attributes, AUDIO_RESOURCE_BIT_DEPTH).ok_or_else(|| {
                anyhow::anyhow!("AudioResource.bit_depth is missing or wrong value kind")
            })?;
        let bit_depth = bit_depth as u8;

        let (channels, channels_version) =
            get_number_attribute(attributes, AUDIO_RESOURCE_CHANNELS).ok_or_else(|| {
                anyhow::anyhow!("AudioResource.channels is missing or wrong value kind")
            })?;
        let channels = channels as u8;

        Ok(Self {
            location,
            hash,
            size,
            duration,
            codec,
            bitrate,
            sample_rate,
            bit_depth,
            channels,

            location_version,
            hash_version,
            size_version,
            duration_version,
            codec_version,
            bitrate_version,
            sample_rate_version,
            bit_depth_version,
            channels_version,
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
                AUDIO_RESOURCE_DURATION,
                EntityAttributeValue {
                    value: Value::number_from_f64(self.duration as f64),
                    version: self.duration_version,
                },
            ),
            (
                AUDIO_RESOURCE_CODEC,
                EntityAttributeValue {
                    value: Value::Text(self.codec.clone()),
                    version: self.codec_version,
                },
            ),
            (
                AUDIO_RESOURCE_BITRATE,
                EntityAttributeValue {
                    value: Value::number_from_f64(self.bitrate as f64),
                    version: self.bitrate_version,
                },
            ),
            (
                AUDIO_RESOURCE_SAMPLE_RATE,
                EntityAttributeValue {
                    value: Value::number_from_f64(self.sample_rate as f64),
                    version: self.sample_rate_version,
                },
            ),
            (
                AUDIO_RESOURCE_BIT_DEPTH,
                EntityAttributeValue {
                    value: Value::number_from_f64(self.bit_depth as f64),
                    version: self.bit_depth_version,
                },
            ),
            (
                AUDIO_RESOURCE_CHANNELS,
                EntityAttributeValue {
                    value: Value::number_from_f64(self.channels as f64),
                    version: self.channels_version,
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
                AUDIO_RESOURCE_DURATION,
                AttributeSchema {
                    name: "Duration".to_string(),
                    value: ValueKind::Number,
                },
            ),
            (
                AUDIO_RESOURCE_CODEC,
                AttributeSchema {
                    name: "Codec".to_string(),
                    value: ValueKind::Text,
                },
            ),
            (
                AUDIO_RESOURCE_BITRATE,
                AttributeSchema {
                    name: "Bitrate".to_string(),
                    value: ValueKind::Number,
                },
            ),
            (
                AUDIO_RESOURCE_SAMPLE_RATE,
                AttributeSchema {
                    name: "Sample Rate".to_string(),
                    value: ValueKind::Number,
                },
            ),
            (
                AUDIO_RESOURCE_BIT_DEPTH,
                AttributeSchema {
                    name: "Bit Depth".to_string(),
                    value: ValueKind::Number,
                },
            ),
            (
                AUDIO_RESOURCE_CHANNELS,
                AttributeSchema {
                    name: "Channels".to_string(),
                    value: ValueKind::Number,
                },
            ),
        ]),
    }
}

fn get_text_attribute(
    attributes: &HashMap<AttributeKind, EntityAttributeValue>,
    attribute: AttributeKind,
) -> Option<(String, Version)> {
    let EntityAttributeValue { value, version } = attributes.get(&attribute)?;
    let Value::Text(value) = value else {
        return None;
    };
    Some((value.clone(), *version))
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
    use crate::audio::{AudioResource, AudioResourceLocation};
    use hegel::{TestCase, generators as gs};
    use stellar_graph::entity::hegel::{gen_author_id, gen_value_number, gen_version};

    #[hegel::composite]
    pub fn gen_audio_resource(tc: TestCase) -> AudioResource {
        let size = tc.draw(
            gs::integers()
                // Very large sizes aren't roundtripped correctly since they're converted to floats.
                // Assume each resource is smaller than 1 TB.
                .max_value(1_000_000_000_000),
        );

        AudioResource {
            location: tc.draw(gen_audio_resource_location()),
            hash: tc.draw(gs::arrays(gs::integers())),
            size,
            duration: tc.draw(gen_value_number()),
            codec: tc.draw(gs::text()), // TODO
            bitrate: tc.draw(gs::integers()),
            sample_rate: tc.draw(gs::integers()),
            bit_depth: tc.draw(gs::integers()),
            channels: tc.draw(gs::integers()),

            location_version: tc.draw(gen_version()),
            hash_version: tc.draw(gen_version()),
            size_version: tc.draw(gen_version()),
            duration_version: tc.draw(gen_version()),
            codec_version: tc.draw(gen_version()),
            bitrate_version: tc.draw(gen_version()),
            sample_rate_version: tc.draw(gen_version()),
            bit_depth_version: tc.draw(gen_version()),
            channels_version: tc.draw(gen_version()),
        }
    }

    #[hegel::composite]
    pub fn gen_audio_resource_location(tc: TestCase) -> AudioResourceLocation {
        AudioResourceLocation {
            device: tc.draw(gen_author_id()),
            path: tc.draw(gs::text()).into(), // TODO: generate paths instead of text
        }
    }
}
