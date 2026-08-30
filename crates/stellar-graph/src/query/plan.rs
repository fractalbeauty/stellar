use crate::{
    entity::{AttributeKind, EntityKind, RelationKind},
    query::exec::{
        CollectRelationAttributesOp, ExecutionContext, Op, RelationJoinDirection,
        RelationMergeJoinOp, RelationNestedLoopJoinOp, ScanEntityKindOp, SlotIndex, SlotValue,
    },
    store::Store,
};
use std::collections::{HashMap, HashSet};

// TODO: choose merge join vs nested loop join using selectivity heuristic
const USE_MERGE_JOIN: bool = true;

#[derive(uniffi::Record)]
pub struct TableQuery {
    pub entity: EntityKind,

    /// The scanned entity's own id
    pub id: Option<OutputIndex>,
    pub attributes: HashMap<AttributeKind, OutputIndex>,

    /// Attributes on outgoing relations (this entity is the source)
    pub outgoing_relation_attributes: HashMap<RelationKind, HashMap<AttributeKind, OutputIndex>>,
    /// Attributes on the targets of outgoing relations (this entity is the source)
    pub outgoing_relation_entity_attributes:
        HashMap<RelationKind, HashMap<AttributeKind, OutputIndex>>,
    /// Maps each outgoing relation ID to its target entity ID.
    pub outgoing_relation_others: HashMap<RelationKind, OutputIndex>,
    /// Attributes on incoming relations (this entity is the target)
    pub incoming_relation_attributes: HashMap<RelationKind, HashMap<AttributeKind, OutputIndex>>,
    /// Attributes on incoming relations (this entity is the target)
    pub incoming_relation_entity_attributes:
        HashMap<RelationKind, HashMap<AttributeKind, OutputIndex>>,
    /// Maps each incoming relation ID to its source entity ID.
    pub incoming_relation_others: HashMap<RelationKind, OutputIndex>,
    // filter: Option<FilterPredicate>
    // sort: Option<Vec<(SortKey, SortDir)>>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutputIndex(pub u16);

uniffi::custom_newtype!(OutputIndex, u16);

impl TableQuery {
    #[tracing::instrument(name = "table_query", skip_all)]
    pub fn execute(&self, store: Store) -> Vec<Vec<Option<SlotValue>>> {
        let planning_span = tracing::debug_span!("planning").entered();

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

        if let Some(id_output) = self.id {
            output_slots.insert(id_output, entity_id);
        }

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
            .chain(self.outgoing_relation_others.keys())
            .collect::<HashSet<_>>();
        let incoming_relations = self
            .incoming_relation_attributes
            .keys()
            .chain(self.incoming_relation_entity_attributes.keys())
            .chain(self.incoming_relation_others.keys())
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

            let others_slot = self.outgoing_relation_others.get(relation).map(|&output| {
                let slot = slot();
                output_slots.insert(output, slot);
                slot
            });

            let join_op: Box<dyn Op> = if USE_MERGE_JOIN {
                Box::new(RelationMergeJoinOp::new(
                    store.clone(),
                    prev_op,
                    self.entity,
                    *relation,
                    RelationJoinDirection::Outgoing,
                    entity_id,
                    relation_slot,
                    other_slot,
                    deleted_slot,
                ))
            } else {
                Box::new(RelationNestedLoopJoinOp::new(
                    store.clone(),
                    prev_op,
                    *relation,
                    RelationJoinDirection::Outgoing,
                    entity_id,
                    relation_slot,
                    other_slot,
                    deleted_slot,
                ))
            };

            prev_op = Box::new(CollectRelationAttributesOp::new(
                store.clone(),
                join_op,
                entity_id,
                relation_slot,
                other_slot,
                outgoing_relation_attributes
                    .get(relation)
                    .cloned()
                    .unwrap_or_default(),
                outgoing_relation_entity_attributes
                    .get(relation)
                    .cloned()
                    .unwrap_or_default(),
                others_slot,
            ));
        }
        for relation in incoming_relations {
            let relation_slot = slot();
            let other_slot = slot();
            let deleted_slot = slot();

            relation_ids.insert(relation, relation_slot);
            relation_others.insert(relation, other_slot);
            relation_deleteds.insert(relation, deleted_slot);

            let others_slot = self.incoming_relation_others.get(relation).map(|&output| {
                let slot = slot();
                output_slots.insert(output, slot);
                slot
            });

            let join_op: Box<dyn Op> = if USE_MERGE_JOIN {
                Box::new(RelationMergeJoinOp::new(
                    store.clone(),
                    prev_op,
                    self.entity,
                    *relation,
                    RelationJoinDirection::Incoming,
                    entity_id,
                    relation_slot,
                    other_slot,
                    deleted_slot,
                ))
            } else {
                Box::new(RelationNestedLoopJoinOp::new(
                    store.clone(),
                    prev_op,
                    *relation,
                    RelationJoinDirection::Incoming,
                    entity_id,
                    relation_slot,
                    other_slot,
                    deleted_slot,
                ))
            };

            prev_op = Box::new(CollectRelationAttributesOp::new(
                store.clone(),
                join_op,
                entity_id,
                relation_slot,
                other_slot,
                incoming_relation_attributes
                    .get(relation)
                    .cloned()
                    .unwrap_or_default(),
                incoming_relation_entity_attributes
                    .get(relation)
                    .cloned()
                    .unwrap_or_default(),
                others_slot,
            ));
        }

        let num_outputs = output_slots
            .keys()
            .map(|output| output.0 + 1)
            .max()
            .unwrap_or_default();

        let num_slots = next_slot;

        drop(planning_span);

        // TODO: split plan/execute
        let executing_span = tracing::debug_span!("executing").entered();
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
        drop(executing_span);

        all_outputs
    }
}

#[cfg(test)]
mod test {
    use crate::{
        entity::{
            AttributeKind, AuthorId, EntityId, EntityKind, RelationId, RelationKind, Timestamp,
            Value, Version,
            hegel::{
                gen_attribute_kind, gen_entity_id_with_kind, gen_entity_kind,
                gen_relation_id_with_kind, gen_relation_kind, gen_value,
            },
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
    use hegel::{
        Generator, TestCase,
        generators::{self as gs},
    };
    use std::collections::{HashMap, HashSet};
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
            id: None,
            attributes: HashMap::from([(album_title, OutputIndex(0))]),
            outgoing_relation_attributes: HashMap::from([(
                track,
                HashMap::from([(track_number, OutputIndex(1))]),
            )]),
            outgoing_relation_entity_attributes: HashMap::from([(
                track,
                HashMap::from([(song_title, OutputIndex(2))]),
            )]),
            outgoing_relation_others: HashMap::new(),
            incoming_relation_attributes: HashMap::new(),
            incoming_relation_entity_attributes: HashMap::new(),
            incoming_relation_others: HashMap::new(),
        };

        let mut outputs = query.execute(store);
        outputs.sort_by_key(|outputs| match &outputs[0] {
            Some(SlotValue::SVValue(Value::Text(text))) => text.clone(),
            _ => "".to_string(),
        });

        assert_eq!(
            outputs,
            vec![
                vec![
                    Some(SlotValue::SVValue(album1_title)),
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
                    Some(SlotValue::SVValue(album2_title)),
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

    /// [`TableQuery`] should return the requested attributes from the store
    #[hegel::test]
    fn entity_attributes_match_store(tc: TestCase) {
        let store = Store::open(
            testdir::testdir!()
                .join(Uuid::new_v4().to_string())
                .join("store"),
        )
        .expect("should open");

        let entity_kind = tc.draw(gen_entity_kind());
        let attribute_kinds = tc
            .draw(gs::hashsets(gen_attribute_kind()).min_size(1).max_size(3))
            .into_iter()
            .collect::<Vec<_>>();

        let entities = tc.draw(
            gs::hashsets(gen_entity_id_with_kind(entity_kind))
                .min_size(1)
                .max_size(5),
        );

        tc.note(&format!(
            "entities = {entities:?}, attribute_kinds = {attribute_kinds:?}"
        ));

        // model: entity -> (attribute -> value), only for a random subset of attribute_kinds
        let mut model = HashMap::new();
        for &entity in &entities {
            set_entity(&store, entity);

            let mut attributes = HashMap::new();
            for &attribute in &attribute_kinds {
                if tc.draw(gs::booleans()) {
                    let value = tc.draw(gen_value());
                    set_entity_attribute(&store, entity, attribute, &value);
                    attributes.insert(attribute, value);
                }
            }
            model.insert(entity, attributes);
        }

        // output index 0 is the entity id, used to correlate each row back to the model
        let query = TableQuery {
            entity: entity_kind,
            id: Some(OutputIndex(0)),
            attributes: attribute_kinds
                .iter()
                .enumerate()
                .map(|(index, &attribute)| (attribute, OutputIndex(index as u16 + 1)))
                .collect(),
            outgoing_relation_attributes: HashMap::new(),
            outgoing_relation_entity_attributes: HashMap::new(),
            outgoing_relation_others: HashMap::new(),
            incoming_relation_attributes: HashMap::new(),
            incoming_relation_entity_attributes: HashMap::new(),
            incoming_relation_others: HashMap::new(),
        };

        let mut actual = HashMap::new();
        for mut row in query.execute(store) {
            let Some(SlotValue::SVEntityId(entity)) = row.remove(0) else {
                unreachable!("output index 0 should be the entity id");
            };
            let attributes = row
                .into_iter()
                .zip(&attribute_kinds)
                .filter_map(|(slot, &attribute)| match slot {
                    Some(SlotValue::SVValue(value)) => Some((attribute, value)),
                    None => None,
                    other => unreachable!("unexpected slot value: {other:?}"),
                })
                .collect::<HashMap<_, _>>();
            actual.insert(entity, attributes);
        }

        assert_eq!(actual, model);
    }

    /// [`TableQuery`] should return requested attributes for outgoing relations and target entities
    #[hegel::test]
    fn outgoing_relation_attributes_match_store(tc: TestCase) {
        let store = Store::open(
            testdir::testdir!()
                .join(Uuid::new_v4().to_string())
                .join("store"),
        )
        .expect("should open");

        let entity_kind = tc.draw(gen_entity_kind());
        let relation_kind = tc.draw(gen_relation_kind());
        let relation_attribute = tc.draw(gen_attribute_kind());
        let target_attribute = tc.draw(gen_attribute_kind());

        let source = tc.draw(gen_entity_id_with_kind(entity_kind));
        set_entity(&store, source);

        let targets = tc.draw(
            gs::hashsets(gen_entity_id_with_kind(entity_kind).filter(|&id| id != source))
                .min_size(1)
                .max_size(4),
        );

        tc.note(&format!("source = {source:?}, targets = {targets:?}"));

        // model: relation id -> (target id, relation attribute value, target attribute value)
        let mut used_relation_ids = HashSet::new();
        let mut model = HashMap::new();
        for &target in &targets {
            set_entity(&store, target);

            let relation = tc.draw(
                gen_relation_id_with_kind(relation_kind)
                    .filter(|id| !used_relation_ids.contains(id)),
            );
            used_relation_ids.insert(relation);
            set_relation(&store, relation, source, target);

            let relation_value = tc.draw(gs::booleans()).then(|| tc.draw(gen_value()));
            if let Some(value) = &relation_value {
                set_relation_attribute(&store, relation, relation_attribute, value);
            }

            let target_value = tc.draw(gs::booleans()).then(|| tc.draw(gen_value()));
            if let Some(value) = &target_value {
                set_entity_attribute(&store, target, target_attribute, value);
            }

            model.insert(relation, (target, relation_value, target_value));
        }

        let query = TableQuery {
            entity: entity_kind,
            id: None,
            attributes: HashMap::new(),
            outgoing_relation_attributes: HashMap::from([(
                relation_kind,
                HashMap::from([(relation_attribute, OutputIndex(0))]),
            )]),
            outgoing_relation_entity_attributes: HashMap::from([(
                relation_kind,
                HashMap::from([(target_attribute, OutputIndex(1))]),
            )]),
            outgoing_relation_others: HashMap::from([(relation_kind, OutputIndex(2))]),
            incoming_relation_attributes: HashMap::new(),
            incoming_relation_entity_attributes: HashMap::new(),
            incoming_relation_others: HashMap::new(),
        };

        let mut rows = query.execute(store);
        assert_eq!(rows.len(), 1, "source should produce exactly one row");
        let mut row = rows.remove(0);

        let Some(SlotValue::RelationOthers(relation_targets)) = row.remove(2) else {
            unreachable!("output index 2 should be relation targets");
        };
        let Some(SlotValue::EntityValues(target_values)) = row.remove(1) else {
            unreachable!("output index 1 should be entity values");
        };
        let Some(SlotValue::RelationValues(relation_values)) = row.remove(0) else {
            unreachable!("output index 0 should be relation values");
        };

        let actual = relation_targets
            .into_iter()
            .map(|(relation, target)| {
                let relation_value = relation_values.get(&relation).cloned();
                let target_value = target_values.get(&target).cloned();
                (relation, (target, relation_value, target_value))
            })
            .collect::<HashMap<_, _>>();

        assert_eq!(actual, model);
    }
}
