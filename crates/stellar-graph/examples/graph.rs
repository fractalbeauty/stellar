use std::{cell::RefCell, collections::HashMap};
use stellar_graph::{
    entity::{
        AttributeKind, AuthorId, EntityId, EntityKind, RelationKind, Store, StoreError, Timestamp,
        Value, ValueKind, Version,
    },
    schema::{AttributeSchema, EntitySchema, RelationSchema, Schema},
};

fn main() {
    let store = MemoryStore::new();

    let v1 = Version::new(Timestamp::now(), AuthorId::new([1u8; 32]));

    let k1 = EntityKind::random();
    let a1 = AttributeKind::random();
    let a2 = AttributeKind::random();

    let k2 = EntityKind::random();
    let a3 = AttributeKind::random();

    let e1 = store.create_entity(k1, v1).unwrap();
    store
        .set_entity_attribute(e1, a1, Value::Text("meow".to_string()), v1)
        .unwrap();
    store
        .set_entity_attribute(e1, a2, Value::Number(1234.0), v1)
        .unwrap();

    let e2 = store.create_entity(k1, v1).unwrap();
    store
        .set_entity_attribute(e2, a1, Value::Text("woof".to_string()), v1)
        .unwrap();
    store
        .set_entity_attribute(e2, a2, Value::Number(5678.0), v1)
        .unwrap();

    let e3 = store.create_entity(k2, v1).unwrap();
    store
        .set_entity_attribute(e3, a3, Value::Number(999.0), v1)
        .unwrap();

    store.create_relation(e1, e3, v1).unwrap();

    let s1 = EntitySchema {
        name: "s1".to_string(),
        attributes: HashMap::from([
            (
                a1,
                AttributeSchema {
                    name: "a1".to_string(),
                    value: ValueKind::Text,
                },
            ),
            (
                a2,
                AttributeSchema {
                    name: "a2".to_string(),
                    value: ValueKind::Number,
                },
            ),
        ]),
    };
    let s2 = EntitySchema {
        name: "s2".to_string(),
        attributes: HashMap::from([(
            a3,
            AttributeSchema {
                name: "a3".to_string(),
                value: ValueKind::Number,
            },
        )]),
    };
    let r1 = RelationSchema {
        name: "k1 to k2".to_string(),
        attributes: HashMap::new(),
    };

    let schema = Schema {
        entities: HashMap::from([(k1, s1), (k2, s2)]),
        relations: HashMap::from([(RelationKind::new(k1, k2), r1)]),
    };

    dbg!(schema);
}

struct MemoryStore {
    data: RefCell<MemoryStoreData>,
}

struct MemoryStoreData {
    entities: HashMap<EntityId, MemoryStoreEntity>,
    relations: HashMap<(EntityId, EntityId), MemoryStoreRelation>,
}

struct MemoryStoreEntity {
    kind: EntityKind,
    attributes: HashMap<AttributeKind, Value>,
}

struct MemoryStoreRelation {
    attributes: HashMap<AttributeKind, Value>,
}

impl MemoryStore {
    fn new() -> Self {
        let data = MemoryStoreData {
            entities: HashMap::new(),
            relations: HashMap::new(),
        };
        Self {
            data: RefCell::new(data),
        }
    }
}

impl Store for MemoryStore {
    fn create_entity(&self, kind: EntityKind, version: Version) -> Result<EntityId, StoreError> {
        let id = EntityId::random();

        let mut data = self.data.borrow_mut();
        data.entities.insert(
            id,
            MemoryStoreEntity {
                kind,
                attributes: HashMap::new(),
            },
        );

        Ok(id)
    }

    fn get_entities(&self) -> Result<Vec<EntityId>, StoreError> {
        let data = self.data.borrow();
        Ok(data.entities.keys().copied().collect())
    }

    fn get_entity_attribute(
        &self,
        entity: EntityId,
        attribute: AttributeKind,
    ) -> Result<Option<Value>, StoreError> {
        let data = self.data.borrow();
        Ok(data
            .entities
            .get(&entity)
            .and_then(|entity| entity.attributes.get(&attribute).cloned()))
    }

    fn set_entity_attribute(
        &self,
        entity: EntityId,
        attribute: AttributeKind,
        value: Value,
        version: Version,
    ) -> Result<(), StoreError> {
        let mut data = self.data.borrow_mut();
        let entity = data.entities.get_mut(&entity);

        // TODO
        let entity = entity.unwrap();

        entity.attributes.insert(attribute, value);
        Ok(())
    }

    fn create_relation(
        &self,
        a: EntityId,
        b: EntityId,
        version: Version,
    ) -> Result<(), StoreError> {
        let a_kind = self.get_entity_kind(a).expect("TODO");
        let b_kind = self.get_entity_kind(b).expect("TODO");
        let key = sort_relation_key(a, b, a_kind, b_kind);

        let mut data = self.data.borrow_mut();
        data.relations.insert(
            key,
            MemoryStoreRelation {
                attributes: HashMap::new(),
            },
        );

        Ok(())
    }

    fn get_relation_attribute(
        &self,
        a: EntityId,
        b: EntityId,
        attribute: AttributeKind,
    ) -> Result<Option<Value>, StoreError> {
        let a_kind = self.get_entity_kind(a).expect("TODO");
        let b_kind = self.get_entity_kind(b).expect("TODO");
        let key = sort_relation_key(a, b, a_kind, b_kind);

        let data = self.data.borrow();
        Ok(data
            .relations
            .get(&key)
            .and_then(|relation| relation.attributes.get(&attribute).cloned()))
    }

    fn set_relation_attribute(
        &self,
        a: EntityId,
        b: EntityId,
        attribute: AttributeKind,
        value: Value,
        version: Version,
    ) -> Result<(), StoreError> {
        let a_kind = self.get_entity_kind(a).expect("TODO");
        let b_kind = self.get_entity_kind(b).expect("TODO");
        let key = sort_relation_key(a, b, a_kind, b_kind);

        let mut data = self.data.borrow_mut();
        let relation = data.relations.get_mut(&key);

        // TODO
        let relation = relation.unwrap();

        relation.attributes.insert(attribute, value);
        Ok(())
    }
}

impl MemoryStore {
    fn get_entity_kind(&self, entity: EntityId) -> Option<EntityKind> {
        let data = self.data.borrow();
        data.entities.get(&entity).map(|entity| entity.kind)
    }
}

fn sort_relation_key(
    a: EntityId,
    b: EntityId,
    a_kind: EntityKind,
    b_kind: EntityKind,
) -> (EntityId, EntityId) {
    if a_kind == b_kind {
        if a.inner() < b.inner() {
            (a, b)
        } else {
            (b, a)
        }
    } else if a_kind.inner() < b_kind.inner() {
        (a, b)
    } else {
        (b, a)
    }
}
