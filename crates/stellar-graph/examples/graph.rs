use std::{cell::RefCell, collections::HashMap};

use stellar_graph::entity::{AttributeId, EntityId, EntityKind, Store, StoreError, Value};

fn main() {
    let store = MemoryStore::new();

    let k1 = EntityKind::new();

    let a1 = AttributeId::new();
    let a2 = AttributeId::new();

    let e1 = store.create_entity(k1).unwrap();
    store
        .set_entity_attribute(e1, a1, Value::Text("meow".to_string()))
        .unwrap();
    store
        .set_entity_attribute(e1, a2, Value::Number(1234.0))
        .unwrap();

    let e2 = store.create_entity(k1).unwrap();
    store
        .set_entity_attribute(e2, a1, Value::Text("woof".to_string()))
        .unwrap();
    store
        .set_entity_attribute(e2, a2, Value::Number(5678.0))
        .unwrap();
}

struct MemoryStore {
    data: RefCell<MemoryStoreData>,
}

struct MemoryStoreData {
    entities: HashMap<EntityId, MemoryStoreEntity>,
}

struct MemoryStoreEntity {
    kind: EntityKind,
    attributes: HashMap<AttributeId, Value>,
}

impl MemoryStore {
    fn new() -> Self {
        let data = MemoryStoreData {
            entities: HashMap::new(),
        };
        Self {
            data: RefCell::new(data),
        }
    }
}

impl Store for MemoryStore {
    fn create_entity(&self, kind: EntityKind) -> Result<EntityId, StoreError> {
        let id = EntityId::new();
        self.data.borrow_mut().entities.insert(
            id,
            MemoryStoreEntity {
                kind,
                attributes: HashMap::new(),
            },
        );
        Ok(id)
    }

    fn get_entities(&self) -> Result<Vec<EntityId>, StoreError> {
        Ok(self.data.borrow().entities.keys().copied().collect())
    }

    fn get_entity_attribute(
        &self,
        entity: EntityId,
        attribute: AttributeId,
    ) -> Result<Option<Value>, StoreError> {
        Ok(self
            .data
            .borrow_mut()
            .entities
            .get(&entity)
            .and_then(|entity| entity.attributes.get(&attribute).cloned()))
    }

    fn set_entity_attribute(
        &self,
        entity: EntityId,
        attribute: AttributeId,
        value: Value,
    ) -> Result<(), StoreError> {
        let mut data = self.data.borrow_mut();
        let entity = data.entities.get_mut(&entity);

        // TODO
        let entity = entity.unwrap();

        entity.attributes.insert(attribute, value);
        Ok(())
    }
}
