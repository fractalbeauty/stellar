use crate::{
    entity::{AttributeKind, EntityId, EntityKind, RelationId, RelationKind, Value},
    store::{EntityAttributeValue, EntityMetadataValue, RawValue, RelationIndexValue, Store},
};
use std::{collections::HashMap, fmt::Debug, iter::Peekable};

pub struct ExecutionContext {
    store: Store,
    slots: Vec<Option<SlotValue>>,
}

impl ExecutionContext {
    pub fn new(store: Store, num_slots: u16) -> Self {
        let mut slots = Vec::with_capacity(num_slots as usize);
        slots.resize_with(num_slots as usize, || None);

        Self { store, slots }
    }

    fn num_slots(&self) -> u16 {
        self.slots.len() as u16
    }

    fn set_slot(&mut self, slot: SlotIndex, value: SlotValue) {
        assert!(slot.0 < self.num_slots(), "slot index out of bounds");

        self.slots[slot.0 as usize] = Some(value);
    }

    fn clear_slot(&mut self, slot: SlotIndex) {
        assert!(slot.0 < self.num_slots(), "slot index out of bounds");

        self.slots[slot.0 as usize] = None;
    }

    pub fn get_slot(&self, slot: SlotIndex) -> &Option<SlotValue> {
        assert!(slot.0 < self.num_slots(), "slot index out of bounds");

        &self.slots[slot.0 as usize]
    }

    /// Snapshots all slots to be restored later by [`Self::restore`].
    ///
    /// Used by [`Grouped`] to peek the next row without clobbering the previous row's slots.
    fn snapshot(&self) -> SlotsSnapshot {
        SlotsSnapshot(self.slots.clone())
    }

    /// Restores all slots from a snapshot taken by [`Self::snapshot`].
    fn restore(&mut self, snapshot: SlotsSnapshot) {
        assert!(snapshot.0.len() == self.num_slots() as usize);

        self.slots = snapshot.0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlotIndex(pub u16);

/// The `SV` prefix for some variants prevents issues with UniFFI codegen when
/// an enum field type name collides with an enum variant name.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SlotValue {
    SVEntityId(EntityId),
    SVRelationId(RelationId),
    SVValue(Value),
    EntityValues(HashMap<EntityId, Value>),
    RelationValues(HashMap<RelationId, Value>),
}

#[derive(Debug, Clone)]
struct SlotsSnapshot(Vec<Option<SlotValue>>);

pub trait Op: std::fmt::Debug {
    fn next(&mut self, ctx: &mut ExecutionContext) -> Option<()>;
}

/// Scans entities by kind, returning (id, deleted, ...attrs).
pub struct ScanEntityKindOp {
    metadata: Box<dyn Iterator<Item = (EntityId, RawValue<EntityMetadataValue>)>>,
    attribute: Peekable<
        Box<dyn Iterator<Item = (EntityId, AttributeKind, RawValue<EntityAttributeValue>)>>,
    >,

    id_slot: SlotIndex,
    deleted_slot: SlotIndex,
    attribute_slots: HashMap<AttributeKind, SlotIndex>,
}

impl ScanEntityKindOp {
    pub fn new(
        store: &Store,

        entity: EntityKind,

        id_slot: SlotIndex,
        deleted_slot: SlotIndex,
        attribute_slots: HashMap<AttributeKind, SlotIndex>,
    ) -> Self {
        Self {
            metadata: Box::new(store.scan_entity_metadata_by_kind(entity)),
            attribute: (Box::new(store.scan_entity_attribute_by_kind(entity))
                as Box<
                    dyn Iterator<Item = (EntityId, AttributeKind, RawValue<EntityAttributeValue>)>,
                >)
                .peekable(),

            id_slot,
            deleted_slot,
            attribute_slots,
        }
    }
}

impl Op for ScanEntityKindOp {
    fn next(&mut self, ctx: &mut ExecutionContext) -> Option<()> {
        loop {
            let Some((entity, metadata)) = self.metadata.next() else {
                return None;
            };

            // TODO: don't copy
            let metadata = match metadata.decode() {
                Ok(metadata) => metadata,
                Err(e) => {
                    tracing::error!(?e, "error parsing entity metadata");
                    continue;
                }
            };

            // Advance attribute iterator to be positioned at start of entity attributes
            while self
                .attribute
                .peek()
                .is_some_and(|(attribute_entity, _, _)| {
                    attribute_entity.as_bytes() < entity.as_bytes()
                })
            {
                self.attribute.next();
            }

            ctx.set_slot(self.id_slot, SlotValue::SVEntityId(entity));

            ctx.set_slot(
                self.deleted_slot,
                SlotValue::SVValue(Value::Bool(metadata.deleted)),
            );

            while self
                .attribute
                .peek()
                .is_some_and(|(attribute_entity, _, _)| *attribute_entity == entity)
            {
                let (_, attribute_kind, attribute_value) =
                    self.attribute.next().expect("peek returned Some");

                if let Some(slot) = self.attribute_slots.get(&attribute_kind) {
                    match attribute_value.decode() {
                        Ok(value) => ctx.set_slot(*slot, SlotValue::SVValue(value.value)),
                        Err(e) => {
                            tracing::error!(?e, "error parsing entity attribute")
                        }
                    }
                }
            }

            return Some(());
        }
    }
}

impl Debug for ScanEntityKindOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScanEntityKindOp")
            .field("metadata", &"<metadata>")
            .field("attribute", &"<attribute>")
            .field("id_slot", &self.id_slot)
            .field("deleted_slot", &self.deleted_slot)
            .field("attribute_slots", &self.attribute_slots)
            .finish()
    }
}

/// Filters tuples where a slot has a given value.
///
/// TODO: remove this for a more general Filter op
pub struct FilterEqOp {
    inner: Box<dyn Op>,
    slot: SlotIndex,
    eq: SlotValue,
}

impl FilterEqOp {
    pub fn new(inner: Box<dyn Op>, slot: SlotIndex, eq: SlotValue) -> Self {
        Self { inner, slot, eq }
    }
}

impl Op for FilterEqOp {
    fn next(&mut self, ctx: &mut ExecutionContext) -> Option<()> {
        loop {
            self.inner.next(ctx)?;

            let Some(value) = ctx.get_slot(self.slot) else {
                // slot is None, filter doesn't match
                continue;
            };

            if *value != self.eq {
                continue;
            }

            return Some(());
        }
    }
}

impl Debug for FilterEqOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilterEqOp")
            .field("inner", &self.inner)
            .field("slot", &self.slot)
            .field("eq", &self.eq)
            .finish()
    }
}

/// Assuming RelationJoinDirection::Outgoing:
/// - There is a relation R from entity A (source) to entity B (target)
/// - `outer` produces A IDs in `start_slot` (used as source for outgoing).
/// - The join loops using `inner` to scan the source->target index for A ID + R kind
/// - `inner` produces R IDs in `relation_slot`, R deleteds in `deleted_slot`, and B IDs in `other_slot`
pub struct RelationJoinOp {
    store: Store,
    outer: Box<dyn Op>,
    inner: Option<Box<dyn Iterator<Item = (RelationId, RawValue<RelationIndexValue>)>>>,

    relation: RelationKind,
    direction: RelationJoinDirection,

    start_slot: SlotIndex,
    relation_slot: SlotIndex,
    other_slot: SlotIndex,
    deleted_slot: SlotIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationJoinDirection {
    Incoming,
    Outgoing,
}

impl RelationJoinOp {
    pub fn new(
        store: Store,
        outer: Box<dyn Op>,

        relation: RelationKind,
        direction: RelationJoinDirection,

        start_slot: SlotIndex,
        relation_slot: SlotIndex,
        other_slot: SlotIndex,
        deleted_slot: SlotIndex,
    ) -> Self {
        Self {
            store,
            outer,
            inner: None,

            relation,
            direction,

            start_slot,
            relation_slot,
            other_slot,
            deleted_slot,
        }
    }
}

impl Op for RelationJoinOp {
    fn next(&mut self, ctx: &mut ExecutionContext) -> Option<()> {
        loop {
            // If not finished, resume inner scan
            if let Some(iter) = &mut self.inner {
                'inner: loop {
                    // Advance inner scan
                    match iter.next() {
                        Some((relation_id, relation)) => {
                            let relation = match relation.decode() {
                                Ok(relation) => relation,
                                Err(e) => {
                                    tracing::error!(?e, "error parsing relation index");
                                    continue 'inner;
                                }
                            };

                            ctx.set_slot(self.relation_slot, SlotValue::SVRelationId(relation_id));
                            ctx.set_slot(self.other_slot, SlotValue::SVEntityId(relation.other));
                            ctx.set_slot(
                                self.deleted_slot,
                                SlotValue::SVValue(Value::Bool(relation.deleted)),
                            );

                            // Emit tuple
                            return Some(());
                        }
                        None => {
                            // Inner finished, continue outer op
                            self.inner = None;
                            break 'inner;
                        }
                    }
                }
            }

            'outer: loop {
                // Advance outer op
                self.outer.next(ctx)?;

                // Get start entity for inner scan
                let Some(start) = ctx.get_slot(self.start_slot) else {
                    // Start slot is None, continue outer op
                    tracing::warn!("RelationJoinOp start slot is None");
                    continue 'outer;
                };
                let SlotValue::SVEntityId(start) = start else {
                    // Start slot is not EntityId, continue outer op
                    tracing::warn!("RelationJoinOp start slot is not EntityId");
                    continue 'outer;
                };

                // Start inner scan
                self.inner = Some(match self.direction {
                    RelationJoinDirection::Incoming => {
                        // Incoming: start is target, other is source
                        Box::new(
                            self.store
                                .scan_relation_index_by_target_and_kind(*start, self.relation),
                        )
                    }
                    RelationJoinDirection::Outgoing => {
                        // Outgoing: start is source, other is target
                        Box::new(
                            self.store
                                .scan_relation_index_by_source_and_kind(*start, self.relation),
                        )
                    }
                });

                break 'outer;
            }

            // Loop and resume inner scan
        }
    }
}

impl Debug for RelationJoinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelationJoinOp")
            .field("store", &"<store>")
            .field("outer", &self.outer)
            .field("inner", &"<inner>")
            .field("relation", &self.relation)
            .field("start_slot", &self.start_slot)
            .field("relation_slot", &self.relation_slot)
            .field("other_slot", &self.other_slot)
            .field("deleted_slot", &self.deleted_slot)
            .finish()
    }
}

/// - ScanEntityKind emits (entity ID)
/// - RelationJoinOp emits (relation ID, other ID)
/// - CollectRelationAttributesOp collects all (relation ID, other ID)
/// - CollectRelationAttributesOp gets multiple other entity attributes and relation attributes
/// - CollectRelationAttributesOp emits EntityValues and RelationValues, both Map<E/R ID, Value>
pub struct CollectRelationAttributesOp {
    store: Store,

    grouped: Grouped<EntityId, (RelationId, EntityId)>,

    key_slot: SlotIndex,
    relation_attribute_slots: HashMap<AttributeKind, SlotIndex>,
    other_attribute_slots: HashMap<AttributeKind, SlotIndex>,
}

impl CollectRelationAttributesOp {
    pub fn new(
        store: Store,

        inner: Box<dyn Op>,

        key_slot: SlotIndex,
        relation_slot: SlotIndex,
        other_slot: SlotIndex,
        relation_attribute_slots: HashMap<AttributeKind, SlotIndex>,
        other_attribute_slots: HashMap<AttributeKind, SlotIndex>,
    ) -> Self {
        let grouped = Grouped::new(inner, move |ctx| {
            let Some(key) = ctx.get_slot(key_slot) else {
                tracing::warn!("CollectRelationAttributesOp read() key slot is None");
                return None;
            };
            let SlotValue::SVEntityId(key) = key else {
                tracing::warn!("CollectRelationAttributesOp read() key slot is not EntityId");
                return None;
            };

            let Some(relation) = ctx.get_slot(relation_slot) else {
                tracing::warn!("CollectRelationAttributesOp read() relation slot is None");
                return None;
            };
            let SlotValue::SVRelationId(relation) = relation else {
                tracing::warn!(
                    "CollectRelationAttributesOp read() relation slot is not RelationId"
                );
                return None;
            };

            let Some(other) = ctx.get_slot(other_slot) else {
                tracing::warn!("CollectRelationAttributesOp read() other slot is None");
                return None;
            };
            let SlotValue::SVEntityId(other) = other else {
                tracing::warn!("CollectRelationAttributesOp read() other slot is not EntityId");
                return None;
            };

            Some((*key, (*relation, *other)))
        });

        Self {
            store,

            grouped,

            key_slot,
            relation_attribute_slots,
            other_attribute_slots,
        }
    }
}

impl Op for CollectRelationAttributesOp {
    fn next(&mut self, ctx: &mut ExecutionContext) -> Option<()> {
        let (_key, rows) = self.grouped.next_group(ctx)?;

        let mut relation_attribute_values = self
            .relation_attribute_slots
            .values()
            .map(|&slot| (slot, HashMap::new()))
            .collect::<HashMap<_, _>>();
        let mut entity_attribute_values = self
            .other_attribute_slots
            .values()
            .map(|&slot| (slot, HashMap::new()))
            .collect::<HashMap<_, _>>();

        for (relation, other) in rows {
            self.store
                .scan_relation_attribute_by_id(relation)
                .for_each(|(attribute, value)| {
                    let Some(slot) = self.relation_attribute_slots.get(&attribute).copied() else {
                        return;
                    };

                    let value = match value.decode() {
                        Ok(value) => value,
                        Err(e) => {
                            tracing::error!(?e, "error parsing relation attribute value");
                            return;
                        }
                    };

                    relation_attribute_values
                        .entry(slot)
                        .or_default()
                        .insert(relation, value.value);
                });

            self.store
                .scan_entity_attribute_by_id(other)
                .for_each(|(attribute, value)| {
                    let Some(slot) = self.other_attribute_slots.get(&attribute).copied() else {
                        return;
                    };

                    let value = match value.decode() {
                        Ok(value) => value,
                        Err(e) => {
                            tracing::error!(?e, "error parsing entity attribute value");
                            return;
                        }
                    };

                    entity_attribute_values
                        .entry(slot)
                        .or_default()
                        .insert(other, value.value);
                });
        }

        for (slot, values) in relation_attribute_values {
            ctx.set_slot(slot, SlotValue::RelationValues(values));
        }
        for (slot, values) in entity_attribute_values {
            ctx.set_slot(slot, SlotValue::EntityValues(values));
        }

        Some(())
    }
}

impl Debug for CollectRelationAttributesOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollectRelationAttributesOp")
            .field("store", &"<store>")
            .field("grouped", &self.grouped)
            .field("key_slot", &self.key_slot)
            .field("relation_attribute_slots", &self.relation_attribute_slots)
            .field("other_attribute_slots", &self.other_attribute_slots)
            .finish()
    }
}

/// Advances the inner op, using `read(ctx) -> (K, R)` to group rows into `K, Vec<R>`.
///
/// `read` is called after the inner op is advanced, and should read K/R from filled slots.
struct Grouped<K, R> {
    inner: Box<dyn Op>,
    read: Box<dyn Fn(&ExecutionContext) -> Option<(K, R)>>, // None = skip this row, don't extract, keep going
    pending: Option<(K, R, SlotsSnapshot)>,
}

impl<K: Eq, R> Grouped<K, R> {
    fn new(
        inner: Box<dyn Op>,
        read: impl Fn(&ExecutionContext) -> Option<(K, R)> + 'static,
    ) -> Self {
        Self {
            inner,
            read: Box::new(read),
            pending: None,
        }
    }

    fn next_group(&mut self, ctx: &mut ExecutionContext) -> Option<(K, Vec<R>)> {
        // Take the pending new group's first row and restore its snapshot
        let (group_key, first_row, group_snapshot) = match self.pending.take() {
            Some((key, row, snapshot)) => {
                ctx.restore(snapshot.clone());
                (key, row, snapshot)
            }
            None => {
                // First iteration, read and snapshot the first group's first row
                let (key, row) = self.read_row(ctx)?;
                (key, row, ctx.snapshot())
            }
        };

        let mut rows = vec![first_row];

        loop {
            // Read the next row
            match self.read_row(ctx) {
                None => {
                    // Inner finished, emit group
                    break;
                }
                Some((key, row)) => {
                    // Check if group changed
                    if key != group_key {
                        // Stash row in pending without adding to group
                        // Also snapshot the new group's first row
                        self.pending = Some((key, row, ctx.snapshot()));

                        // Emit group
                        break;
                    } else {
                        // Add to group
                        rows.push(row);
                    }
                }
            }
        }

        // Restore the snapshot from the first row of the group since peeking
        // the first row of the next group may have clobbered some slots.
        ctx.restore(group_snapshot);

        Some((group_key, rows))
    }

    /// Reads the next row, skipping when read() returns None.
    ///
    /// Returns None when the inner op is finished.
    fn read_row(&mut self, ctx: &mut ExecutionContext) -> Option<(K, R)> {
        loop {
            self.inner.next(ctx)?;
            match (self.read)(ctx) {
                Some(row) => return Some(row),
                None => {
                    tracing::warn!("Grouped read() retuned None, skipping row");
                    continue;
                }
            }
        }
    }
}

impl<K, R> Debug for Grouped<K, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grouped")
            .field("inner", &self.inner)
            .field("read", &"<read>")
            .field("pending", &"<pending>")
            .finish()
    }
}

#[cfg(test)]
mod test {
    use crate::{
        entity::{
            AttributeKind, AuthorId, EntityId, EntityKind, RelationId, RelationKind, Timestamp,
            Value, Version,
        },
        query::exec::{
            CollectRelationAttributesOp, ExecutionContext, FilterEqOp, Op, RelationJoinDirection,
            RelationJoinOp, ScanEntityKindOp, SlotIndex, SlotValue,
        },
        store::{
            EntityAttributeValue, EntityMetadataValue, RelationAttributeValue,
            RelationMetadataValue, Store,
        },
    };
    use std::collections::{HashMap, HashSet};
    use uuid::Uuid;

    #[test]
    fn scan_entity_metadata() {
        let store = Store::open(
            testdir::testdir!()
                .join(Uuid::new_v4().to_string())
                .join("store"),
        )
        .expect("should open");

        let entity = EntityKind::random();
        let entity_id = EntityId::random(entity);

        let attribute = AttributeKind::random();
        let attribute_value = Value::Text("test".to_string());

        let version = Version::new(Timestamp::now(), AuthorId::from_bytes([0u8; 32]));

        store
            .merge_entity_metadata(
                entity_id,
                EntityMetadataValue {
                    deleted: false,
                    deleted_version: version,
                },
            )
            .expect("should update");
        store
            .merge_entity_attribute(
                entity_id,
                attribute,
                EntityAttributeValue {
                    value: attribute_value.clone(),
                    version,
                },
            )
            .expect("should update");

        let mut ctx = ExecutionContext::new(store, 3);

        let mut op = ScanEntityKindOp::new(
            &ctx.store,
            entity,
            SlotIndex(0),
            SlotIndex(1),
            HashMap::from([(attribute, SlotIndex(2))]),
        );

        assert_eq!(*ctx.get_slot(SlotIndex(0)), None);
        assert_eq!(*ctx.get_slot(SlotIndex(1)), None);
        assert_eq!(*ctx.get_slot(SlotIndex(2)), None);

        assert_eq!(op.next(&mut ctx), Some(()));
        assert_eq!(
            *ctx.get_slot(SlotIndex(0)),
            Some(SlotValue::SVEntityId(entity_id))
        );
        assert_eq!(
            *ctx.get_slot(SlotIndex(1)),
            Some(SlotValue::SVValue(Value::Bool(false)))
        );
        assert_eq!(
            *ctx.get_slot(SlotIndex(2)),
            Some(SlotValue::SVValue(attribute_value))
        );

        assert_eq!(op.next(&mut ctx), None);
    }

    #[test]
    fn filter_eq() {
        let store = Store::open(
            testdir::testdir!()
                .join(Uuid::new_v4().to_string())
                .join("store"),
        )
        .expect("should open");

        let entity = EntityKind::random();
        let entity1 = EntityId::random(entity);
        let entity2 = EntityId::random(entity);
        let entity3 = EntityId::random(entity);

        let version = Version::new(Timestamp::now(), AuthorId::from_bytes([0u8; 32]));

        // 1 and 3 not deleted, 2 deleted
        store
            .merge_entity_metadata(
                entity1,
                EntityMetadataValue {
                    deleted: false,
                    deleted_version: version,
                },
            )
            .expect("should update");
        store
            .merge_entity_metadata(
                entity2,
                EntityMetadataValue {
                    deleted: true,
                    deleted_version: version,
                },
            )
            .expect("should update");
        store
            .merge_entity_metadata(
                entity3,
                EntityMetadataValue {
                    deleted: false,
                    deleted_version: version,
                },
            )
            .expect("should update");

        let mut ctx = ExecutionContext::new(store, 3);

        let scan_op = ScanEntityKindOp::new(
            &ctx.store,
            entity,
            SlotIndex(0),
            SlotIndex(1),
            HashMap::new(),
        );
        let mut filter_op = FilterEqOp::new(
            Box::new(scan_op),
            SlotIndex(1),
            SlotValue::SVValue(Value::Bool(false)),
        );

        let mut result = HashSet::new();
        while let Some(()) = filter_op.next(&mut ctx) {
            match ctx.get_slot(SlotIndex(0)) {
                Some(SlotValue::SVEntityId(entity)) => result.insert(*entity),
                _ => panic!("expected SlotValue::EntityId in SlotIndex(0)"),
            };
        }

        assert_eq!(result, HashSet::from([entity1, entity3]));
    }

    #[test]
    fn relation_source_join() {
        let store = Store::open(
            testdir::testdir!()
                .join(Uuid::new_v4().to_string())
                .join("store"),
        )
        .expect("should open");

        let entity = EntityKind::random();
        let entity1 = EntityId::random(entity);
        let entity2 = EntityId::random(entity);
        let entity3 = EntityId::random(entity);
        let entity4 = EntityId::random(entity);

        let relation = RelationKind::random();
        let relation1 = RelationId::random(relation);
        let relation2 = RelationId::random(relation);

        let version = Version::new(Timestamp::now(), AuthorId::from_bytes([0u8; 32]));

        store
            .merge_entity_metadata(
                entity1,
                EntityMetadataValue {
                    deleted: false,
                    deleted_version: version,
                },
            )
            .expect("should update");
        store
            .merge_entity_metadata(
                entity2,
                EntityMetadataValue {
                    deleted: false,
                    deleted_version: version,
                },
            )
            .expect("should update");
        store
            .merge_entity_metadata(
                entity3,
                EntityMetadataValue {
                    deleted: false,
                    deleted_version: version,
                },
            )
            .expect("should update");
        store
            .merge_entity_metadata(
                entity4,
                EntityMetadataValue {
                    deleted: false,
                    deleted_version: version,
                },
            )
            .expect("should update");

        store
            .merge_relation_metadata(
                relation1,
                RelationMetadataValue {
                    source: entity1,
                    target: entity2,
                    deleted: false,
                    deleted_version: version,
                },
            )
            .expect("should update");
        store
            .merge_relation_metadata(
                relation2,
                RelationMetadataValue {
                    source: entity3,
                    target: entity4,
                    deleted: false,
                    deleted_version: version,
                },
            )
            .expect("should update");

        let mut ctx = ExecutionContext::new(store, 5);

        let scan_op = ScanEntityKindOp::new(
            &ctx.store,
            entity,
            SlotIndex(0),
            SlotIndex(1),
            HashMap::new(),
        );
        let mut join_op = RelationJoinOp::new(
            ctx.store.clone(),
            Box::new(scan_op),
            relation,
            RelationJoinDirection::Outgoing,
            SlotIndex(0),
            SlotIndex(2),
            SlotIndex(3),
            SlotIndex(4),
        );

        let mut result = HashSet::new();
        while let Some(()) = join_op.next(&mut ctx) {
            let source = match ctx.get_slot(SlotIndex(0)) {
                Some(SlotValue::SVEntityId(entity)) => *entity,
                _ => panic!("expected SlotValue::EntityId in SlotIndex(0)"),
            };
            let relation = match ctx.get_slot(SlotIndex(2)) {
                Some(SlotValue::SVRelationId(relation)) => *relation,
                _ => panic!("expected SlotValue::RelationId in SlotIndex(2)"),
            };
            let target = match ctx.get_slot(SlotIndex(3)) {
                Some(SlotValue::SVEntityId(entity)) => *entity,
                _ => panic!("expected SlotValue::EntityId in SlotIndex(3)"),
            };
            result.insert((source, relation, target));
        }

        assert_eq!(
            result,
            HashSet::from([(entity1, relation1, entity2), (entity3, relation2, entity4),])
        );
    }

    #[test]
    fn collect_relation_attributes() {
        let store = Store::open(
            testdir::testdir!()
                .join(Uuid::new_v4().to_string())
                .join("store"),
        )
        .expect("should open");

        let entity = EntityKind::random();
        let entity1 = EntityId::random(entity);
        let entity2 = EntityId::random(entity);
        let entity3 = EntityId::random(entity);

        let relation = RelationKind::random();
        let relation1 = RelationId::random(relation);
        let relation2 = RelationId::random(relation);

        let attribute = AttributeKind::random();
        let entity2attribute = Value::Text("e2".to_string());
        let entity3attribute = Value::Text("e3".to_string());
        let relation1attribute = Value::Text("r1".to_string());
        let relation2attribute = Value::Text("r2".to_string());

        let version = Version::new(Timestamp::now(), AuthorId::from_bytes([0u8; 32]));

        store
            .merge_entity_metadata(
                entity1,
                EntityMetadataValue {
                    deleted: false,
                    deleted_version: version,
                },
            )
            .expect("should update");
        store
            .merge_entity_metadata(
                entity2,
                EntityMetadataValue {
                    deleted: false,
                    deleted_version: version,
                },
            )
            .expect("should update");
        store
            .merge_entity_metadata(
                entity3,
                EntityMetadataValue {
                    deleted: false,
                    deleted_version: version,
                },
            )
            .expect("should update");

        store
            .merge_relation_metadata(
                relation1,
                RelationMetadataValue {
                    source: entity1,
                    target: entity2,
                    deleted: false,
                    deleted_version: version,
                },
            )
            .expect("should update");
        store
            .merge_relation_metadata(
                relation2,
                RelationMetadataValue {
                    source: entity1,
                    target: entity3,
                    deleted: false,
                    deleted_version: version,
                },
            )
            .expect("should update");

        store
            .merge_entity_attribute(
                entity2,
                attribute,
                EntityAttributeValue {
                    value: entity2attribute.clone(),
                    version,
                },
            )
            .expect("should update");
        store
            .merge_entity_attribute(
                entity3,
                attribute,
                EntityAttributeValue {
                    value: entity3attribute.clone(),
                    version,
                },
            )
            .expect("should update");

        store
            .merge_relation_attribute(
                relation1,
                attribute,
                RelationAttributeValue {
                    value: relation1attribute.clone(),
                    version,
                },
            )
            .expect("should update");
        store
            .merge_relation_attribute(
                relation2,
                attribute,
                RelationAttributeValue {
                    value: relation2attribute.clone(),
                    version,
                },
            )
            .expect("should update");

        let mut ctx = ExecutionContext::new(store, 7);

        let scan_op = ScanEntityKindOp::new(
            &ctx.store,
            entity,
            SlotIndex(0),
            SlotIndex(1),
            HashMap::new(),
        );
        let join_op = RelationJoinOp::new(
            ctx.store.clone(),
            Box::new(scan_op),
            relation,
            RelationJoinDirection::Outgoing,
            SlotIndex(0),
            SlotIndex(2),
            SlotIndex(3),
            SlotIndex(4),
        );
        let mut collect_op = CollectRelationAttributesOp::new(
            ctx.store.clone(),
            Box::new(join_op),
            SlotIndex(0),
            SlotIndex(2),
            SlotIndex(3),
            HashMap::from([(attribute, SlotIndex(5))]),
            HashMap::from([(attribute, SlotIndex(6))]),
        );

        let mut result = Vec::new();
        while let Some(()) = collect_op.next(&mut ctx) {
            let source = match ctx.get_slot(SlotIndex(0)) {
                Some(SlotValue::SVEntityId(entity)) => *entity,
                _ => panic!("expected SlotValue::EntityId in SlotIndex(0)"),
            };
            let relation_values = match ctx.get_slot(SlotIndex(5)) {
                Some(SlotValue::RelationValues(values)) => values.clone(),
                _ => panic!("expected SlotValue::RelationValues in SlotIndex(5)"),
            };
            let other_values = match ctx.get_slot(SlotIndex(6)) {
                Some(SlotValue::EntityValues(values)) => values.clone(),
                _ => panic!("expected SlotValue::EntityValues in SlotIndex(6)"),
            };
            result.push((source, relation_values, other_values));
        }

        assert_eq!(
            result,
            vec![(
                entity1,
                HashMap::from([
                    (relation1, relation1attribute),
                    (relation2, relation2attribute),
                ]),
                HashMap::from([(entity2, entity2attribute), (entity3, entity3attribute)]),
            )]
        );
    }
}
