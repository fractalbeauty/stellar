use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::{collections::HashMap, hint::black_box};
use stellar_graph::{
    entity::{
        AttributeKind, AuthorId, EntityId, EntityKind, RelationId, RelationKind, Timestamp, Value,
        Version,
    },
    query::plan::{OutputIndex, TableQuery},
    store::{
        EntityAttributeValue, EntityMetadataValue, RelationAttributeValue, RelationMetadataValue,
        Store,
    },
};
use uuid::Uuid;

const SONGS_PER_ALBUM: usize = 10;
const ALBUMS_PER_ARTIST: usize = 4;

fn version() -> Version {
    Version::new(Timestamp::new(0), AuthorId::from_bytes([0u8; 32]))
}

fn set_entity(store: &Store, entity: EntityId) {
    store
        .merge_entity_metadata(
            entity,
            EntityMetadataValue {
                deleted: false,
                deleted_version: version(),
            },
        )
        .expect("should update");
}

fn set_entity_attribute(store: &Store, entity: EntityId, attribute: AttributeKind, value: Value) {
    store
        .merge_entity_attribute(
            entity,
            attribute,
            EntityAttributeValue {
                value,
                version: version(),
            },
        )
        .expect("should update");
}

fn set_relation(store: &Store, relation: RelationId, source: EntityId, target: EntityId) {
    store
        .merge_relation_metadata(
            relation,
            RelationMetadataValue {
                source,
                target,
                deleted: false,
                deleted_version: version(),
            },
        )
        .expect("should update");
}

fn set_relation_attribute(
    store: &Store,
    relation: RelationId,
    attribute: AttributeKind,
    value: Value,
) {
    store
        .merge_relation_attribute(
            relation,
            attribute,
            RelationAttributeValue {
                value,
                version: version(),
            },
        )
        .expect("should update");
}

fn build_dataset(song_count: usize) -> (Store, TableQuery) {
    let store = Store::open(
        std::env::temp_dir()
            .join("stellar-graph-bench")
            .join(Uuid::new_v4().to_string()),
    )
    .expect("should open store");

    let song = EntityKind::random();
    let album = EntityKind::random();
    let artist = EntityKind::random();

    let song_title = AttributeKind::random();
    let album_title = AttributeKind::random();
    let artist_name = AttributeKind::random();

    let album_song = RelationKind::random();
    let album_track_number = AttributeKind::random();
    let album_artist = RelationKind::random();
    let song_artist = RelationKind::random();

    let album_count = song_count.div_ceil(SONGS_PER_ALBUM);
    let artist_count = album_count.div_ceil(ALBUMS_PER_ARTIST);

    let artists = (0..artist_count)
        .map(|i| {
            let artist_id = EntityId::random(artist);
            set_entity(&store, artist_id);
            set_entity_attribute(
                &store,
                artist_id,
                artist_name,
                Value::Text(format!("artist {i}")),
            );
            artist_id
        })
        .collect::<Vec<_>>();

    for a in 0..album_count {
        let album_id = EntityId::random(album);
        set_entity(&store, album_id);
        set_entity_attribute(
            &store,
            album_id,
            album_title,
            Value::Text(format!("album {a}")),
        );

        let album_artist_target = artists[a % artist_count];

        let album_artist_id = RelationId::random(album_artist);
        set_relation(&store, album_artist_id, album_id, album_artist_target);

        for t in 0..SONGS_PER_ALBUM {
            let song_id = EntityId::random(song);
            set_entity(&store, song_id);
            set_entity_attribute(
                &store,
                song_id,
                song_title,
                Value::Text(format!("album {a} song {t}")),
            );

            let album_song_id = RelationId::random(album_song);
            set_relation(&store, album_song_id, album_id, song_id);
            set_relation_attribute(
                &store,
                album_song_id,
                album_track_number,
                Value::number_from_f64(t as f64),
            );

            let song_artist_id = RelationId::random(song_artist);
            set_relation(&store, song_artist_id, song_id, album_artist_target);
        }
    }

    let query = TableQuery {
        entity: song,
        id: None,
        attributes: HashMap::from([(song_title, OutputIndex(0))]),
        outgoing_relation_attributes: HashMap::from([(song_artist, HashMap::new())]),
        outgoing_relation_entity_attributes: HashMap::from([(
            song_artist,
            HashMap::from([(artist_name, OutputIndex(1))]),
        )]),
        outgoing_relation_others: HashMap::new(),
        incoming_relation_attributes: HashMap::from([(
            album_song,
            HashMap::from([(album_track_number, OutputIndex(2))]),
        )]),
        incoming_relation_entity_attributes: HashMap::from([(
            album_song,
            HashMap::from([(album_title, OutputIndex(3))]),
        )]),
        incoming_relation_others: HashMap::new(),
    };

    (store, query)
}

fn bench_table_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("table_query");

    for song_count in [1_000, 10_000, 100_000] {
        let (store, query) = build_dataset(song_count);

        group.bench_with_input(
            BenchmarkId::from_parameter(song_count),
            &song_count,
            |b, _| {
                b.iter(|| black_box(query.execute(store.clone())));
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_table_query);
criterion_main!(benches);
