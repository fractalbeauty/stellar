use crate::{
    entity::{AttributeKind, EntityId, EntityKind, Value},
    store::{EntityAttributeValue, EntityMetadataValue, Store},
};
use fjall::Slice;
use std::{collections::HashMap, iter::Peekable};

struct ExecutionContext {
    store: Store,
    slots: Vec<Option<SlotValue>>,
}

impl ExecutionContext {
    fn new(store: Store, num_slots: u16) -> Self {
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

    fn get_slot(&mut self, slot: SlotIndex) -> &Option<SlotValue> {
        assert!(slot.0 < self.num_slots(), "slot index out of bounds");

        &self.slots[slot.0 as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SlotIndex(u16);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum SlotValue {
    EntityId(EntityId),
    Value(Value),
}

trait Op {
    fn next(&mut self, ctx: &mut ExecutionContext) -> Option<()>;
}

/// Scans entities by kind, returning (id, deleted, ...attrs).
struct ScanEntityKindOp {
    metadata: Box<dyn Iterator<Item = (EntityId, Slice)>>,
    attribute: Peekable<Box<dyn Iterator<Item = (EntityId, AttributeKind, Slice)>>>,

    id_slot: SlotIndex,
    deleted_slot: SlotIndex,
    attribute_slots: HashMap<AttributeKind, SlotIndex>,
}

impl ScanEntityKindOp {
    fn new(
        store: &Store,
        entity: EntityKind,
        id_slot: SlotIndex,
        deleted_slot: SlotIndex,
        attribute_slots: HashMap<AttributeKind, SlotIndex>,
    ) -> Self {
        Self {
            metadata: Box::new(store.scan_entity_metadata_by_kind(entity)),
            attribute: (Box::new(store.scan_entity_attribute_by_kind(entity))
                as Box<dyn Iterator<Item = (EntityId, AttributeKind, Slice)>>)
                .peekable(),

            id_slot,
            deleted_slot,
            attribute_slots,
        }
    }
}

impl Op for ScanEntityKindOp {
    fn next(&mut self, ctx: &mut ExecutionContext) -> Option<()> {
        let Some((entity, metadata_bytes)) = self.metadata.next() else {
            return None;
        };

        // TODO: don't copy
        let metadata = match postcard::from_bytes::<EntityMetadataValue>(metadata_bytes.as_ref()) {
            Ok(metadata) => metadata,
            Err(e) => {
                tracing::error!(?e, "error parsing entity metadata");
                return self.next(ctx);
            }
        };

        // Advance attribute iterator to be positioned at start of entity attributes
        while self
            .attribute
            .peek()
            .is_some_and(|(attribute_entity, _, _)| attribute_entity.as_bytes() < entity.as_bytes())
        {
            self.attribute.next();
        }

        ctx.set_slot(self.id_slot, SlotValue::EntityId(entity));

        ctx.set_slot(
            self.deleted_slot,
            SlotValue::Value(Value::Boolean(metadata.deleted)),
        );

        while self
            .attribute
            .peek()
            .is_some_and(|(attribute_entity, _, _)| *attribute_entity == entity)
        {
            let (_, attribute_kind, value_bytes) =
                self.attribute.next().expect("peek returned Some");

            if let Some(slot) = self.attribute_slots.get(&attribute_kind) {
                match postcard::from_bytes::<EntityAttributeValue>(value_bytes.as_ref()) {
                    Ok(value) => ctx.set_slot(*slot, SlotValue::Value(value.value)),
                    Err(e) => {
                        tracing::error!(?e, "error parsing entity attribute")
                    }
                }
            }
        }

        return Some(());
    }
}

/// Filters tuples where a slot has a given value.
///
/// TODO: remove this for a more general Filter op
struct FilterEqOp {
    inner: Box<dyn Op>,
    slot: SlotIndex,
    eq: SlotValue,
}

impl FilterEqOp {
    fn new(inner: Box<dyn Op>, slot: SlotIndex, eq: SlotValue) -> Self {
        Self { inner, slot, eq }
    }
}

impl Op for FilterEqOp {
    fn next(&mut self, ctx: &mut ExecutionContext) -> Option<()> {
        let Some(_) = self.inner.next(ctx) else {
            return None;
        };

        dbg!(&ctx.get_slot(self.slot));

        let Some(value) = ctx.get_slot(self.slot) else {
            // slot is None, filter doesn't match
            return self.next(ctx);
        };

        if *value == self.eq {
            Some(())
        } else {
            // TODO!!: replace recursion with inner loop/continue
            self.next(ctx)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        entity::{AttributeKind, AuthorId, EntityId, EntityKind, Timestamp, Value, Version},
        query::exec::{ExecutionContext, FilterEqOp, Op, ScanEntityKindOp, SlotIndex, SlotValue},
        store::{EntityAttributeValue, EntityMetadataValue, Store},
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
            Some(SlotValue::EntityId(entity_id))
        );
        assert_eq!(
            *ctx.get_slot(SlotIndex(1)),
            Some(SlotValue::Value(Value::Boolean(false)))
        );
        assert_eq!(
            *ctx.get_slot(SlotIndex(2)),
            Some(SlotValue::Value(attribute_value))
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
            SlotValue::Value(Value::Boolean(false)),
        );

        let mut result = HashSet::new();
        while let Some(()) = filter_op.next(&mut ctx) {
            match ctx.get_slot(SlotIndex(0)) {
                Some(SlotValue::EntityId(entity)) => result.insert(*entity),
                _ => panic!("expected SlotValue::EntityId in SlotIndex(0)"),
            };
        }

        assert_eq!(result, HashSet::from([entity1, entity3]));
    }
}
