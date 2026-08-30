use crate::{
    entity::{AttributeKind, EntityKind, RelationKind},
    query::exec::{
        CollectRelationAttributesOp, ExecutionContext, Op, RelationJoinDirection, RelationJoinOp,
        ScanEntityKindOp, SlotIndex, SlotValue,
    },
    store::Store,
};
use std::collections::{HashMap, HashSet};

pub struct TableQuery {
    entity: EntityKind,

    attributes: HashMap<AttributeKind, OutputIndex>,

    /// Attributes on outgoing relations (this entity is the source)
    outgoing_relation_attributes: HashMap<RelationKind, HashMap<AttributeKind, OutputIndex>>,
    /// Attributes on the targets of outgoing relations (this entity is the source)
    outgoing_relation_entity_attributes: HashMap<RelationKind, HashMap<AttributeKind, OutputIndex>>,
    /// Attributes on incoming relations (this entity is the target)
    incoming_relation_attributes: HashMap<RelationKind, HashMap<AttributeKind, OutputIndex>>,
    /// Attributes on incoming relations (this entity is the target)
    incoming_relation_entity_attributes: HashMap<RelationKind, HashMap<AttributeKind, OutputIndex>>,
    // filter: Option<FilterPredicate>
    // sort: Option<Vec<(SortKey, SortDir)>>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutputIndex(pub u16);

impl TableQuery {
    pub fn execute(&self, store: Store) -> Vec<Vec<Option<SlotValue>>> {
        // Generate increasing SlotIndexes
        let mut next_slot = 0;
        let mut slot = || {
            let slot = next_slot;
            next_slot += 1;
            SlotIndex(slot)
        };

        // Accumulate map of OutputIndex -> SlotIndex for collecting outputs from slots
        let mut output_slots = HashMap::new();

        // Allocate slots for entity metadata for scanning entities
        let entity_id = slot();
        let entity_deleted = slot();

        // Build map of AttributeKind -> SlotIndex for scanning entity attributes
        let entity_attributes = self
            .attributes
            .iter()
            .map(|(&attribute, &output)| {
                let slot = slot();
                output_slots.insert(output, slot);
                (attribute, slot)
            })
            .collect::<HashMap<_, _>>();

        // Collect all relations
        let outgoing_relations = self
            .outgoing_relation_attributes
            .keys()
            .chain(self.outgoing_relation_entity_attributes.keys())
            .collect::<HashSet<_>>();
        let incoming_relations = self
            .incoming_relation_attributes
            .keys()
            .chain(self.incoming_relation_entity_attributes.keys())
            .collect::<HashSet<_>>();

        // Build maps of RelationKind -> AttributeKind -> SlotIndex for collecting relation attributes
        let outgoing_relation_attributes = self
            .outgoing_relation_attributes
            .iter()
            .map(|(relation, attributes)| {
                (
                    relation,
                    attributes
                        .iter()
                        .map(|(&attribute, &output)| {
                            let slot = slot();
                            output_slots.insert(output, slot);
                            (attribute, slot)
                        })
                        .collect::<HashMap<_, _>>(),
                )
            })
            .collect::<HashMap<_, _>>();
        let outgoing_relation_entity_attributes = self
            .outgoing_relation_entity_attributes
            .iter()
            .map(|(relation, attributes)| {
                (
                    relation,
                    attributes
                        .iter()
                        .map(|(&attribute, &output)| {
                            let slot = slot();
                            output_slots.insert(output, slot);
                            (attribute, slot)
                        })
                        .collect::<HashMap<_, _>>(),
                )
            })
            .collect::<HashMap<_, _>>();
        let incoming_relation_attributes = self
            .incoming_relation_attributes
            .iter()
            .map(|(relation, attributes)| {
                (
                    relation,
                    attributes
                        .iter()
                        .map(|(&attribute, &output)| {
                            let slot = slot();
                            output_slots.insert(output, slot);
                            (attribute, slot)
                        })
                        .collect::<HashMap<_, _>>(),
                )
            })
            .collect::<HashMap<_, _>>();
        let incoming_relation_entity_attributes = self
            .incoming_relation_entity_attributes
            .iter()
            .map(|(relation, attributes)| {
                (
                    relation,
                    attributes
                        .iter()
                        .map(|(&attribute, &output)| {
                            let slot = slot();
                            output_slots.insert(output, slot);
                            (attribute, slot)
                        })
                        .collect::<HashMap<_, _>>(),
                )
            })
            .collect::<HashMap<_, _>>();

        // Accumulate maps of RelationKind -> SlotIndex for storing relation metadata
        let mut relation_ids = HashMap::new();
        let mut relation_others = HashMap::new();
        let mut relation_deleteds = HashMap::new();

        // Build base entity scan op
        let entity_scan = ScanEntityKindOp::new(
            &store,
            self.entity,
            entity_id,
            entity_deleted,
            entity_attributes,
        );
        let mut prev_op: Box<dyn Op> = Box::new(entity_scan);

        // Chain relation join and collect ops
        for relation in outgoing_relations {
            let relation_slot = slot();
            let other_slot = slot();
            let deleted_slot = slot();

            relation_ids.insert(relation, relation_slot);
            relation_others.insert(relation, other_slot);
            relation_deleteds.insert(relation, deleted_slot);

            prev_op = Box::new(CollectRelationAttributesOp::new(
                store.clone(),
                Box::new(RelationJoinOp::new(
                    store.clone(),
                    prev_op,
                    *relation,
                    RelationJoinDirection::Outgoing,
                    entity_id,
                    relation_slot,
                    other_slot,
                    deleted_slot,
                )),
                entity_id,
                relation_slot,
                other_slot,
                outgoing_relation_attributes.get(relation).unwrap().clone(),
                outgoing_relation_entity_attributes
                    .get(relation)
                    .unwrap()
                    .clone(),
            ));
        }
        for relation in incoming_relations {
            let relation_slot = slot();
            let other_slot = slot();
            let deleted_slot = slot();

            relation_ids.insert(relation, relation_slot);
            relation_others.insert(relation, other_slot);
            relation_deleteds.insert(relation, deleted_slot);

            prev_op = Box::new(CollectRelationAttributesOp::new(
                store.clone(),
                Box::new(RelationJoinOp::new(
                    store.clone(),
                    prev_op,
                    *relation,
                    RelationJoinDirection::Incoming,
                    entity_id,
                    relation_slot,
                    other_slot,
                    deleted_slot,
                )),
                entity_id,
                relation_slot,
                other_slot,
                incoming_relation_attributes.get(relation).unwrap().clone(),
                incoming_relation_entity_attributes
                    .get(relation)
                    .unwrap()
                    .clone(),
            ));
        }

        dbg!(&prev_op);
        dbg!(&output_slots);

        let num_outputs = output_slots
            .keys()
            .map(|output| output.0 + 1)
            .max()
            .unwrap_or_default();

        let num_slots = next_slot;

        // TODO: split plan/execute
        let mut ctx = ExecutionContext::new(store, num_slots);
        let mut all_outputs = Vec::new();
        loop {
            let Some(()) = prev_op.next(&mut ctx) else {
                break;
            };

            let mut outputs = Vec::with_capacity(num_outputs as usize);
            outputs.resize_with(num_outputs as usize, || None);

            for (&output, &slot) in &output_slots {
                outputs[output.0 as usize] = ctx.get_slot(slot).clone();
            }

            all_outputs.push(outputs);
        }

        all_outputs
    }
}

#[cfg(test)]
mod test {
    use crate::{
        entity::{
            AttributeKind, AuthorId, EntityId, EntityKind, RelationId, RelationKind, Timestamp,
            Value, Version,
        },
        query::{
            exec::SlotValue,
            plan::{OutputIndex, TableQuery},
        },
        store::{
            EntityAttributeValue, EntityMetadataValue, RelationAttributeValue,
            RelationMetadataValue, Store,
        },
    };
    use std::collections::HashMap;
    use uuid::Uuid;

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

    fn set_entity_attribute(
        store: &Store,
        entity: EntityId,
        attribute: AttributeKind,
        value: &Value,
    ) {
        store
            .merge_entity_attribute(
                entity,
                attribute,
                EntityAttributeValue {
                    value: value.clone(),
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
        value: &Value,
    ) {
        store
            .merge_relation_attribute(
                relation,
                attribute,
                RelationAttributeValue {
                    value: value.clone(),
                    version: version(),
                },
            )
            .expect("should update");
    }

    #[test]
    fn table_query() {
        let store = Store::open(
            testdir::testdir!()
                .join(Uuid::new_v4().to_string())
                .join("store"),
        )
        .expect("should open");

        let album = EntityKind::random();
        let song = EntityKind::random();

        let album1 = EntityId::random(album);
        let album2 = EntityId::random(album);

        let album1song1 = EntityId::random(song);
        let album1song2 = EntityId::random(song);
        let album2song1 = EntityId::random(song);
        let album2song2 = EntityId::random(song);

        let track = RelationKind::random();

        let album1track1 = RelationId::random(track);
        let album1track2 = RelationId::random(track);
        let album2track1 = RelationId::random(track);
        let album2track2 = RelationId::random(track);

        let album_title = AttributeKind::random();
        let album1_title = Value::Text("a1".to_string());
        let album2_title = Value::Text("a2".to_string());

        let song_title = AttributeKind::random();
        let album1song1_title = Value::Text("a1s1".to_string());
        let album1song2_title = Value::Text("a1s2".to_string());
        let album2song1_title = Value::Text("a2s1".to_string());
        let album2song2_title = Value::Text("a2s2".to_string());

        let track_number = AttributeKind::random();
        let album1track1_number = Value::Text("a1t1".to_string());
        let album1track2_number = Value::Text("a1t2".to_string());
        let album2track1_number = Value::Text("a2t1".to_string());
        let album2track2_number = Value::Text("a2t2".to_string());

        set_entity(&store, album1);
        set_entity(&store, album2);

        set_entity_attribute(&store, album1, album_title, &album1_title);
        set_entity_attribute(&store, album2, album_title, &album2_title);

        set_entity(&store, album1song1);
        set_entity(&store, album1song2);
        set_entity(&store, album2song1);
        set_entity(&store, album2song2);

        set_entity_attribute(&store, album1song1, song_title, &album1song1_title);
        set_entity_attribute(&store, album1song2, song_title, &album1song2_title);
        set_entity_attribute(&store, album2song1, song_title, &album2song1_title);
        set_entity_attribute(&store, album2song2, song_title, &album2song2_title);

        set_relation(&store, album1track1, album1, album1song1);
        set_relation(&store, album1track2, album1, album1song2);
        set_relation(&store, album2track1, album2, album2song1);
        set_relation(&store, album2track2, album2, album2song2);

        set_relation_attribute(&store, album1track1, track_number, &album1track1_number);
        set_relation_attribute(&store, album1track2, track_number, &album1track2_number);
        set_relation_attribute(&store, album2track1, track_number, &album2track1_number);
        set_relation_attribute(&store, album2track2, track_number, &album2track2_number);

        // Albums with album title, song title, track number
        let query = TableQuery {
            entity: album,
            attributes: HashMap::from([(album_title, OutputIndex(0))]),
            outgoing_relation_attributes: HashMap::from([(
                track,
                HashMap::from([(track_number, OutputIndex(1))]),
            )]),
            outgoing_relation_entity_attributes: HashMap::from([(
                track,
                HashMap::from([(song_title, OutputIndex(2))]),
            )]),
            incoming_relation_attributes: HashMap::new(),
            incoming_relation_entity_attributes: HashMap::new(),
        };

        let mut outputs = query.execute(store);
        outputs.sort_by_key(|outputs| match &outputs[0] {
            Some(SlotValue::Value(Value::Text(text))) => text.clone(),
            _ => "".to_string(),
        });

        assert_eq!(
            outputs,
            vec![
                vec![
                    Some(SlotValue::Value(album1_title)),
                    Some(SlotValue::RelationValues(HashMap::from([
                        (album1track1, album1track1_number),
                        (album1track2, album1track2_number)
                    ]))),
                    Some(SlotValue::EntityValues(HashMap::from([
                        (album1song1, album1song1_title),
                        (album1song2, album1song2_title)
                    ])))
                ],
                vec![
                    Some(SlotValue::Value(album2_title)),
                    Some(SlotValue::RelationValues(HashMap::from([
                        (album2track1, album2track1_number),
                        (album2track2, album2track2_number)
                    ]))),
                    Some(SlotValue::EntityValues(HashMap::from([
                        (album2song1, album2song1_title),
                        (album2song2, album2song2_title)
                    ])))
                ]
            ]
        );
    }
}
