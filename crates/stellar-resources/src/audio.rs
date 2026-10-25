use anyhow::Context;
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
    pub size: u64,
    pub duration: f64,

    pub hash: Option<[u8; 32]>,
    pub codec: Option<String>,
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub channels: Option<u8>,

    pub location_version: Version,
    pub size_version: Version,
    pub duration_version: Version,

    pub hash_version: Option<Version>,
    pub codec_version: Option<Version>,
    pub bitrate_version: Option<Version>,
    pub sample_rate_version: Option<Version>,
    pub bit_depth_version: Option<Version>,
    pub channels_version: Option<Version>,
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

        let (hash, hash_version) = get_optional_bytes_attribute(attributes, AUDIO_RESOURCE_HASH)
            .context("AudioResource.hash is the wrong value kind")?;
        let hash = hash
            .map(|hash| {
                hash.try_into().map_err(|_| {
                    anyhow::anyhow!("Failed to deserialize AudioResource.hash: wrong length")
                })
            })
            .transpose()?;

        let (size, size_version) = get_number_attribute(attributes, AUDIO_RESOURCE_SIZE)
            .ok_or_else(|| anyhow::anyhow!("AudioResource.size is missing or wrong value kind"))?;
        let size: u64 = size as u64;

        let (duration, duration_version) =
            get_number_attribute(attributes, AUDIO_RESOURCE_DURATION).ok_or_else(|| {
                anyhow::anyhow!("AudioResource.duration is missing or wrong value kind")
            })?;

        let (codec, codec_version) = get_optional_text_attribute(attributes, AUDIO_RESOURCE_CODEC)
            .context("AudioResource.codec is the wrong value kind")?;

        let (bitrate, bitrate_version) =
            get_optional_number_attribute(attributes, AUDIO_RESOURCE_BITRATE)
                .context("AudioResource.bitrate is the wrong value kind")?;
        let bitrate = bitrate.map(|bitrate| bitrate as u32);

        let (sample_rate, sample_rate_version) =
            get_optional_number_attribute(attributes, AUDIO_RESOURCE_SAMPLE_RATE)
                .context("AudioResource.sample_rate is the wrong value kind")?;
        let sample_rate = sample_rate.map(|sample_rate| sample_rate as u32);

        let (bit_depth, bit_depth_version) =
            get_optional_number_attribute(attributes, AUDIO_RESOURCE_BIT_DEPTH)
                .context("AudioResource.bit_depth is the wrong value kind")?;
        let bit_depth = bit_depth.map(|bit_depth| bit_depth as u8);

        let (channels, channels_version) =
            get_optional_number_attribute(attributes, AUDIO_RESOURCE_CHANNELS)
                .context("AudioResource.channels is the wrong value kind")?;
        let channels = channels.map(|channels| channels as u8);

        Ok(Self {
            location,
            size,
            duration,
            hash,
            codec,
            bitrate,
            sample_rate,
            bit_depth,
            channels,

            location_version,
            size_version,
            duration_version,
            hash_version,
            codec_version,
            bitrate_version,
            sample_rate_version,
            bit_depth_version,
            channels_version,
        })
    }

    pub fn to_attributes(&self) -> HashMap<AttributeKind, EntityAttributeValue> {
        let mut attributes = HashMap::from([
            (
                AUDIO_RESOURCE_LOCATION,
                EntityAttributeValue {
                    value: Value::Bytes(AudioResourceLocation::encode(&self.location)),
                    version: self.location_version,
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
        ]);

        let optional_attributes = [
            (
                AUDIO_RESOURCE_HASH,
                self.hash.map(|hash| Value::Bytes(hash.to_vec())),
                self.hash_version,
            ),
            (
                AUDIO_RESOURCE_CODEC,
                self.codec.clone().map(Value::Text),
                self.codec_version,
            ),
            (
                AUDIO_RESOURCE_BITRATE,
                self.bitrate
                    .map(|bitrate| Value::number_from_f64(bitrate as f64)),
                self.bitrate_version,
            ),
            (
                AUDIO_RESOURCE_SAMPLE_RATE,
                self.sample_rate
                    .map(|sample_rate| Value::number_from_f64(sample_rate as f64)),
                self.sample_rate_version,
            ),
            (
                AUDIO_RESOURCE_BIT_DEPTH,
                self.bit_depth
                    .map(|bit_depth| Value::number_from_f64(bit_depth as f64)),
                self.bit_depth_version,
            ),
            (
                AUDIO_RESOURCE_CHANNELS,
                self.channels
                    .map(|channels| Value::number_from_f64(channels as f64)),
                self.channels_version,
            ),
        ];
        for (attribute, value, version) in optional_attributes {
            if let Some(version) = version {
                attributes.insert(
                    attribute,
                    EntityAttributeValue {
                        value: value.unwrap_or(Value::None),
                        version,
                    },
                );
            }
        }

        attributes
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

fn get_optional_number_attribute(
    attributes: &HashMap<AttributeKind, EntityAttributeValue>,
    attribute: AttributeKind,
) -> Result<(Option<f64>, Option<Version>), anyhow::Error> {
    match attributes.get(&attribute) {
        None => Ok((None, None)),
        Some(EntityAttributeValue {
            value: Value::None,
            version,
        }) => Ok((None, Some(*version))),
        Some(EntityAttributeValue {
            value: Value::Number(value),
            version,
        }) => Ok((Some(**value), Some(*version))),
        Some(_) => anyhow::bail!("Expected a number or none"),
    }
}

fn get_optional_text_attribute(
    attributes: &HashMap<AttributeKind, EntityAttributeValue>,
    attribute: AttributeKind,
) -> Result<(Option<String>, Option<Version>), anyhow::Error> {
    match attributes.get(&attribute) {
        None => Ok((None, None)),
        Some(EntityAttributeValue {
            value: Value::None,
            version,
        }) => Ok((None, Some(*version))),
        Some(EntityAttributeValue {
            value: Value::Text(value),
            version,
        }) => Ok((Some(value.clone()), Some(*version))),
        Some(_) => anyhow::bail!("Expected text or none"),
    }
}

fn get_optional_bytes_attribute(
    attributes: &HashMap<AttributeKind, EntityAttributeValue>,
    attribute: AttributeKind,
) -> Result<(Option<&[u8]>, Option<Version>), anyhow::Error> {
    match attributes.get(&attribute) {
        None => Ok((None, None)),
        Some(EntityAttributeValue {
            value: Value::None,
            version,
        }) => Ok((None, Some(*version))),
        Some(EntityAttributeValue {
            value: Value::Bytes(value),
            version,
        }) => Ok((Some(value.as_slice()), Some(*version))),
        Some(_) => anyhow::bail!("Expected bytes or none"),
    }
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
    use stellar_graph::entity::{
        Version,
        hegel::{gen_author_id, gen_value_number, gen_version},
    };

    #[hegel::composite]
    pub fn gen_audio_resource(tc: TestCase) -> AudioResource {
        let size = tc.draw(
            gs::integers()
                // Very large sizes aren't roundtripped correctly since they're converted to floats.
                // Assume each resource is smaller than 1 TB.
                .max_value(1_000_000_000_000),
        );

        let hash = tc.draw(gs::optional(gs::arrays(gs::integers())));
        let codec = tc.draw(gs::optional(gs::text())); // TODO
        let bitrate = tc.draw(gs::optional(gs::integers()));
        let sample_rate = tc.draw(gs::optional(gs::integers()));
        let bit_depth = tc.draw(gs::optional(gs::integers()));
        let channels = tc.draw(gs::optional(gs::integers()));

        let codec_version = draw_optional_version(&tc, codec.is_some());

        AudioResource {
            location: tc.draw(gen_audio_resource_location()),
            size,
            duration: tc.draw(gen_value_number()),
            hash,
            codec,
            bitrate,
            sample_rate,
            bit_depth,
            channels,

            location_version: tc.draw(gen_version()),
            size_version: tc.draw(gen_version()),
            duration_version: tc.draw(gen_version()),
            hash_version: draw_optional_version(&tc, hash.is_some()),
            codec_version,
            bitrate_version: draw_optional_version(&tc, bitrate.is_some()),
            sample_rate_version: draw_optional_version(&tc, sample_rate.is_some()),
            bit_depth_version: draw_optional_version(&tc, bit_depth.is_some()),
            channels_version: draw_optional_version(&tc, channels.is_some()),
        }
    }

    fn draw_optional_version(tc: &TestCase, has_value: bool) -> Option<Version> {
        (has_value || tc.draw(gs::booleans())).then(|| tc.draw(gen_version()))
    }

    #[hegel::composite]
    pub fn gen_audio_resource_location(tc: TestCase) -> AudioResourceLocation {
        AudioResourceLocation {
            device: tc.draw(gen_author_id()),
            path: tc.draw(gs::text()).into(), // TODO: generate paths instead of text
        }
    }
}
