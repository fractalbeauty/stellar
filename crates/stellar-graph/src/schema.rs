use crate::entity::{AttributeKind, EntityKind, RelationKind, ValueKind};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, automorph::Automorph, uniffi::Record)]
pub struct Schema {
    pub entities: HashMap<EntityKind, EntitySchema>,
    pub relations: HashMap<RelationKind, RelationSchema>,
}

#[derive(Debug, Clone, automorph::Automorph, uniffi::Record)]
pub struct EntitySchema {
    pub name: String,
    pub attributes: HashMap<AttributeKind, AttributeSchema>,
}

#[derive(Debug, Clone, automorph::Automorph, uniffi::Record)]
pub struct RelationSchema {
    pub name: String,
    pub source: EntityKind,
    pub target: EntityKind,
    pub attributes: HashMap<AttributeKind, AttributeSchema>,
}

#[derive(Debug, Clone, automorph::Automorph, uniffi::Record)]
pub struct AttributeSchema {
    pub name: String,
    pub value: ValueKind,
}

impl Schema {
    /// Creates a new schema with the default configuration.
    pub fn new_default() -> Self {
        let song = EntityKind::random();
        let song_schema = EntitySchema {
            name: "Song".to_string(),
            attributes: HashMap::from([(
                AttributeKind::random(),
                AttributeSchema {
                    name: "Title".to_string(),
                    value: ValueKind::Text,
                },
            )]),
        };

        let album = EntityKind::random();
        let album_schema = EntitySchema {
            name: "Album".to_string(),
            attributes: HashMap::from([(
                AttributeKind::random(),
                AttributeSchema {
                    name: "Title".to_string(),
                    value: ValueKind::Text,
                },
            )]),
        };

        let artist = EntityKind::random();
        let artist_schema = EntitySchema {
            name: "Artist".to_string(),
            attributes: HashMap::from([(
                AttributeKind::random(),
                AttributeSchema {
                    name: "Name".to_string(),
                    value: ValueKind::Text,
                },
            )]),
        };

        let album_song = RelationKind::random();
        let album_song_schema = RelationSchema {
            name: "Track".to_string(),
            source: album,
            target: song,
            attributes: HashMap::from([(
                AttributeKind::random(),
                AttributeSchema {
                    name: "Track Number".to_string(),
                    value: ValueKind::Number,
                },
            )]),
        };

        let album_artist = RelationKind::random();
        let album_artist_schema = RelationSchema {
            name: "Album Artist".to_string(),
            source: album,
            target: artist,
            attributes: HashMap::new(),
        };

        let song_artist = RelationKind::random();
        let song_artist_schema = RelationSchema {
            name: "Song Artist".to_string(),
            source: song,
            target: artist,
            attributes: HashMap::new(),
        };

        Self {
            entities: HashMap::from([
                (song, song_schema),
                (album, album_schema),
                (artist, artist_schema),
            ]),
            relations: HashMap::from([
                (album_song, album_song_schema),
                (album_artist, album_artist_schema),
                (song_artist, song_artist_schema),
            ]),
        }
    }
}
