use serde::{Deserialize, Serialize};
use siphasher::sip::SipHasher;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    hash::Hasher,
};
use stellar_graph::{
    entity::{EntityId, Version},
    store::EntityData,
};
use stellar_riblt::{CodedSymbol, PeelableResult, RatelessIBLT, UnmanagedRatelessIBLT};
use tokio_util::bytes::Bytes;
use tracing::warn;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntitySymbol {
    id: EntityId,
    hash: u64,
}

impl EntitySymbol {
    fn new(id: EntityId, data: &EntityData, key0: u64, key1: u64) -> Self {
        let mut hasher = SipHasher::new_with_keys(key0, key1);

        // Hash deleted version
        write_version(&mut hasher, data.metadata.deleted_version);

        // Sort attributes by kind
        let mut sorted_attributes = data.attributes.iter().collect::<Vec<_>>();
        sorted_attributes.sort_by_key(|(attribute, _value)| attribute.inner());

        // Hash attributes
        for (attribute, value) in sorted_attributes {
            hasher.write_u128(attribute.inner().as_u128());
            write_version(&mut hasher, value.version);
        }

        let hash = hasher.finish();

        Self { id, hash }
    }
}

impl stellar_riblt::Symbol for EntitySymbol {
    const BYTE_ARRAY_LENGTH: usize = 24;

    fn encode_to_bytes(&self) -> Vec<u8> {
        let mut buffer = vec![0u8; Self::BYTE_ARRAY_LENGTH];
        buffer[0..16].copy_from_slice(&self.id.inner().as_u128().to_le_bytes());
        buffer[16..24].copy_from_slice(&self.hash.to_le_bytes());
        buffer
    }

    fn decode_from_bytes(bytes: &[u8]) -> Self {
        let id = u128::from_le_bytes(bytes[0..16].try_into().unwrap());
        let hash = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
        Self {
            id: EntityId::new(Uuid::from_u128(id)),
            hash,
        }
    }

    fn hash(&self) -> u64 {
        let mut hasher = SipHasher::new_with_keys(123, 456);
        hasher.write_u128(self.id.inner().as_u128());
        hasher.write_u64(self.hash);
        hasher.finish()
    }
}

fn write_version(hasher: &mut SipHasher, version: Version) {
    hasher.write_u64(version.timestamp().inner());
    hasher.write(&version.author().inner());
}

pub struct SyncClientProtocol {
    entities: HashMap<EntityId, EntityData>,
    riblt: RatelessIBLT<EntitySymbol, Vec<EntitySymbol>>,
    next_index: usize,
    sent_all: bool,
    outbox: VecDeque<SyncClientMessage>,
    result: Option<HashMap<EntityId, EntityData>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SyncClientMessage {
    CodedSymbols {
        coded_symbols: Vec<CodedSymbol<EntitySymbol>>,
    },
    Difference {
        server_difference: HashMap<EntityId, EntityData>,
    },
}

impl SyncClientProtocol {
    const BATCH_CODED_SYMBOLS: usize = 10;

    pub fn new(entities: HashMap<EntityId, EntityData>) -> Self {
        let symbols = entities
            .iter()
            .map(|(entity, data)| EntitySymbol::new(*entity, data, 123, 456))
            .collect::<Vec<_>>();

        let riblt = RatelessIBLT::new(symbols);

        Self {
            entities,
            riblt,
            next_index: 0,
            sent_all: false,
            outbox: VecDeque::new(),
            result: None,
        }
    }

    pub fn handle_message(&mut self, message: SyncServerMessage) {
        match message {
            SyncServerMessage::Difference {
                client_difference,
                server_missing,
            } => {
                self.result = Some(client_difference);

                let server_difference = server_missing.into_iter().filter_map(|entity| {
                    let Some(data) = self.entities.get(&entity) else {
                        warn!(
                            "SyncServerMessage::Difference server_missing entity not in client entities"
                        );
                        return None;
                    };
                    Some((entity, data.clone()))
                }).collect();

                self.outbox
                    .push_back(SyncClientMessage::Difference { server_difference });
            }
        }
    }

    pub fn poll_message(&mut self) -> Option<SyncClientMessage> {
        // queued messages first
        if let Some(message) = self.outbox.pop_front() {
            return Some(message);
        }

        // don't produce symbols if already done
        if self.result.is_some() || self.sent_all {
            return None;
        }

        // produce symbols
        let mut coded_symbols = Vec::new();
        for _ in 0..Self::BATCH_CODED_SYMBOLS {
            let index = self.next_index;
            self.next_index += 1;

            let coded_symbol = self.riblt.get_coded_symbol(index);
            if coded_symbol.is_empty() {
                // TODO: this is not actually correct, could be empty without being done
                self.sent_all = true;
            }
            coded_symbols.push(coded_symbol);

            if self.sent_all {
                break;
            }
        }

        if !coded_symbols.is_empty() {
            Some(SyncClientMessage::CodedSymbols { coded_symbols })
        } else {
            None
        }
    }

    pub fn is_finished(&self) -> bool {
        self.result.is_some() && self.outbox.is_empty()
    }

    /// Finishes syncing, returning the difference to apply locally.
    ///
    /// `is_finished` must return true before calling `finish`
    pub fn finish(self) -> HashMap<EntityId, EntityData> {
        debug_assert!(self.is_finished());
        self.result.expect("should be finished")
    }
}

impl SyncClientMessage {
    pub fn encode(message: &Self) -> Bytes {
        postcard::to_stdvec(&message)
            .expect("Failed to serialize message")
            .into()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        postcard::from_bytes(bytes)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize message: {e:?}"))
    }
}

pub struct SyncServerProtocol {
    entities: HashMap<EntityId, EntityData>,
    riblt: RatelessIBLT<EntitySymbol, Vec<EntitySymbol>>,
    received: UnmanagedRatelessIBLT<EntitySymbol>,
    sent_difference: bool,
    outbox: VecDeque<SyncServerMessage>,
    result: Option<HashMap<EntityId, EntityData>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SyncServerMessage {
    Difference {
        client_difference: HashMap<EntityId, EntityData>,
        server_missing: HashSet<EntityId>,
    },
}

impl SyncServerProtocol {
    pub fn new(entities: HashMap<EntityId, EntityData>) -> Self {
        let symbols = entities
            .iter()
            .map(|(entity, data)| EntitySymbol::new(*entity, data, 123, 456))
            .collect::<Vec<_>>();

        let riblt = RatelessIBLT::new(symbols);

        Self {
            entities,
            riblt,
            received: UnmanagedRatelessIBLT::new(),
            outbox: VecDeque::new(),
            sent_difference: false,
            result: None,
        }
    }

    pub fn handle_message(&mut self, message: SyncClientMessage) {
        match message {
            SyncClientMessage::CodedSymbols { coded_symbols } => {
                if self.sent_difference {
                    return;
                }

                for coded_symbol in coded_symbols {
                    self.received.add_coded_symbol(&coded_symbol);
                }

                let mut collapsed = self.riblt.collapse(&self.received);
                let peeled = collapsed.peel_all_symbols();
                if collapsed.is_empty() {
                    let mut client_missing = HashSet::new();
                    let mut server_missing = HashSet::new();
                    for symbol in peeled {
                        match symbol {
                            PeelableResult::Local(symbol) => {
                                client_missing.insert(symbol.id);
                            }
                            PeelableResult::Remote(symbol) => {
                                server_missing.insert(symbol.id);
                            }
                            PeelableResult::NotPeelable => {
                                // TODO: update stellar-riblt to remove this case
                                unreachable!("peel_all_symbols only returns peeled symbols")
                            }
                        }
                    }

                    let client_difference = client_missing
                        .into_iter()
                        .filter_map(|entity| {
                            let Some(data) = self.entities.get(&entity) else {
                                warn!(
                                    "SyncServerProtocol decoded but client_missing entity not in server entities"
                                );
                                return None;
                            };
                            Some((entity, data.clone()))
                        })
                        .collect();

                    self.outbox.push_back(SyncServerMessage::Difference {
                        client_difference,
                        server_missing,
                    });
                    self.sent_difference = true;
                }
            }
            SyncClientMessage::Difference { server_difference } => {
                self.result = Some(server_difference);
            }
        }
    }

    pub fn poll_message(&mut self) -> Option<SyncServerMessage> {
        self.outbox.pop_front()
    }

    pub fn is_finished(&self) -> bool {
        self.result.is_some() && self.outbox.is_empty()
    }

    /// Finishes syncing, returning the difference to apply locally.
    ///
    /// `is_finished` must return true before calling `finish`
    pub fn finish(self) -> HashMap<EntityId, EntityData> {
        debug_assert!(self.is_finished());
        self.result.expect("should be finished")
    }
}

impl SyncServerMessage {
    pub fn encode(message: &Self) -> Bytes {
        postcard::to_stdvec(&message)
            .expect("Failed to serialize message")
            .into()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        postcard::from_bytes(bytes)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize message: {e:?}"))
    }
}

#[cfg(test)]
mod test {
    use crate::graph::{
        EntitySymbol, SyncClientMessage, SyncClientProtocol, SyncServerMessage, SyncServerProtocol,
    };
    use hegel::{
        Generator, TestCase,
        generators::{self as gs},
    };
    use std::collections::{HashMap, HashSet};
    use stellar_graph::{
        entity::{
            AuthorId, EntityId, EntityKind, Timestamp, Version,
            hegel::{gen_entity_id, gen_version},
        },
        store::{EntityData, EntityMetadataValue, hegel::gen_entity_data},
    };
    use stellar_riblt::Symbol;
    use uuid::Uuid;

    #[hegel::test]
    fn sync(tc: TestCase) {
        let all_entities = tc.draw(gs::hashsets(gen_entity_id()));

        // client and server have a random prefix of all entities
        let client_entities = all_entities
            .iter()
            .copied()
            .take(tc.draw(gs::integers().min_value(0).max_value(all_entities.len())))
            .collect::<HashSet<_>>();
        let server_entities = all_entities
            .iter()
            .copied()
            .take(tc.draw(gs::integers().min_value(0).max_value(all_entities.len())))
            .collect::<HashSet<_>>();

        tc.note(&format!(
            "client = {client_entities:?}, server = {server_entities:?}"
        ));

        // make empty entities to keep the test small
        let client_data = client_entities
            .iter()
            .copied()
            .map(|entity: EntityId| (entity, make_empty_entity_data()))
            .collect::<HashMap<_, _>>();
        let server_data = server_entities
            .iter()
            .copied()
            .map(|entity| (entity, make_empty_entity_data()))
            .collect::<HashMap<_, _>>();

        let client_only_data = client_data
            .clone()
            .into_iter()
            .filter(|(entity, _data)| !server_entities.contains(entity))
            .collect::<HashMap<_, _>>();
        let server_only_data = server_data
            .clone()
            .into_iter()
            .filter(|(entity, _data)| !client_entities.contains(entity))
            .collect::<HashMap<_, _>>();

        let mut client = SyncClientProtocol::new(client_data);
        let mut server = SyncServerProtocol::new(server_data);
        run_client_and_server(&mut client, &mut server);

        // client should be missing server-only data
        let client_difference = client.finish();
        assert_eq!(
            client_difference, server_only_data,
            "client should be missing server-only data"
        );

        // server should be missing client-only entities
        let server_difference = server.finish();
        assert_eq!(
            server_difference, client_only_data,
            "server should be missing client-only data"
        );
    }

    fn run_client_and_server(client: &mut SyncClientProtocol, server: &mut SyncServerProtocol) {
        let max_iterations = 1000;

        let mut count_client_message_difference = 0;
        let mut count_server_message_difference = 0;

        for _ in 0..max_iterations {
            let client_message = client.poll_message();
            if let Some(message) = client_message {
                if let SyncClientMessage::Difference { .. } = &message {
                    count_client_message_difference += 1;
                }

                server.handle_message(message);
            }

            let server_message = server.poll_message();
            if let Some(message) = server_message {
                match &message {
                    SyncServerMessage::Difference { .. } => {
                        count_server_message_difference += 1;
                    }
                }

                client.handle_message(message);
            }

            // early exit when finished
            if client.is_finished() && server.is_finished() {
                break;
            }
        }

        // should terminate
        assert!(
            client.is_finished(),
            "client did not finish in {max_iterations} iterations"
        );
        assert!(
            server.is_finished(),
            "server did not finish in {max_iterations} iterations"
        );

        // should only send one difference message
        assert!(count_client_message_difference <= 1);
        assert!(count_server_message_difference <= 1);
    }

    #[hegel::test]
    fn symbol_hash_deterministic(tc: TestCase) {
        let key0 = tc.draw(gs::integers());
        let key1 = tc.draw(gs::integers());

        let entity = tc.draw(gen_entity_id());
        let data = tc.draw(gen_entity_data());

        let symbol1 = EntitySymbol::new(entity, &data, key0, key1);
        let symbol2 = EntitySymbol::new(entity, &data, key0, key1);

        assert_eq!(symbol1, symbol2);
    }

    #[hegel::test]
    fn symbol_hash_uses_deleted_version(tc: TestCase) {
        let key0 = tc.draw(gs::integers());
        let key1 = tc.draw(gs::integers());

        let entity = tc.draw(gen_entity_id());
        let data = tc.draw(gen_entity_data());

        let symbol1 = EntitySymbol::new(entity, &data, key0, key1);

        let mut new_data = data.clone();
        new_data.metadata.deleted_version =
            tc.draw(gen_version().filter(|version| *version != data.metadata.deleted_version));

        tc.note(&format!("before / after = {data:?} / {new_data:?}"));

        let symbol2 = EntitySymbol::new(entity, &new_data, key0, key1);

        assert_ne!(
            symbol1, symbol2,
            "symbol should change when deleted version changes"
        );
    }

    #[hegel::test]
    fn symbol_hash_uses_attribute_version(tc: TestCase) {
        let key0 = tc.draw(gs::integers());
        let key1 = tc.draw(gs::integers());

        let entity = tc.draw(gen_entity_id());
        let data = tc.draw(gen_entity_data().filter(|data| !data.attributes.is_empty()));

        let symbol1 = EntitySymbol::new(entity, &data, key0, key1);

        let mut new_data = data.clone();
        let attribute_kind = new_data.attributes.keys().next().copied().unwrap();
        new_data
            .attributes
            .get_mut(&attribute_kind)
            .unwrap()
            .version =
            tc.draw(gen_version().filter(|version| {
                *version != data.attributes.get(&attribute_kind).unwrap().version
            }));

        tc.note(&format!("before / after = {data:?} / {new_data:?}"));

        let symbol2 = EntitySymbol::new(entity, &new_data, key0, key1);

        assert_ne!(
            symbol1, symbol2,
            "symbol should change when attribute version changes"
        );
    }

    #[hegel::test]
    fn symbol_encode_decode_roundtrip(tc: TestCase) {
        let key0 = tc.draw(gs::integers());
        let key1 = tc.draw(gs::integers());

        let entity = tc.draw(gen_entity_id());
        let data = tc.draw(gen_entity_data());

        let symbol = EntitySymbol::new(entity, &data, key0, key1);

        let decoded = EntitySymbol::decode_from_bytes(&symbol.encode_to_bytes());
        assert_eq!(decoded, symbol);
    }

    fn make_empty_entity_data() -> EntityData {
        EntityData {
            metadata: EntityMetadataValue {
                kind: EntityKind::new(Uuid::nil()),
                deleted: false,
                deleted_version: Version::new(Timestamp::new(0), AuthorId::new(Default::default())),
            },
            attributes: HashMap::new(),
        }
    }
}
