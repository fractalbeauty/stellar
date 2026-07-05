use riblt::CodedSymbol;
use siphasher::sip::SipHasher;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    hash::Hasher,
    iter,
};
use stellar_graph::{
    entity::{EntityId, Version},
    store::EntityData,
};
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

impl riblt::Symbol for EntitySymbol {
    fn zero() -> Self {
        Self {
            id: EntityId::new(Uuid::nil()),
            hash: 0,
        }
    }

    fn xor(&self, other: &Self) -> Self {
        Self {
            id: EntityId::new(Uuid::from_u128(
                self.id.inner().as_u128() ^ other.id.inner().as_u128(),
            )),
            hash: self.hash ^ other.hash,
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
    encoder: riblt::Encoder<EntitySymbol>,
    outbox: VecDeque<SyncClientMessage>,
    result: Option<HashMap<EntityId, EntityData>>,
}

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
            .map(|(entity, data)| EntitySymbol::new(*entity, data, 123, 456));

        let mut encoder = riblt::Encoder::new();
        for symbol in symbols {
            encoder.add_symbol(&symbol);
        }

        Self {
            entities,
            encoder,
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
        if self.result.is_some() {
            return None;
        }

        // produce symbols
        let coded_symbols = iter::repeat_with(|| self.encoder.produce_next_coded_symbol())
            .take(Self::BATCH_CODED_SYMBOLS)
            .collect::<Vec<_>>();
        Some(SyncClientMessage::CodedSymbols { coded_symbols })
    }

    pub fn is_finished(&self) -> bool {
        self.result.is_some()
    }

    /// Finishes syncing, returning the difference to apply locally.
    ///
    /// `is_finished` must return true before calling `finish`
    pub fn finish(self) -> HashMap<EntityId, EntityData> {
        debug_assert!(self.is_finished());
        self.result.expect("should be finished")
    }
}

pub struct SyncServerProtocol {
    entities: HashMap<EntityId, EntityData>,
    decoder: riblt::Decoder<EntitySymbol>,
    outbox: VecDeque<SyncServerMessage>,
    result: Option<HashMap<EntityId, EntityData>>,
}

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
            .map(|(entity, data)| EntitySymbol::new(*entity, data, 123, 456));

        let mut decoder = riblt::Decoder::new();
        for symbol in symbols {
            decoder.add_symbol(&symbol);
        }

        Self {
            entities,
            decoder,
            outbox: VecDeque::new(),
            result: None,
        }
    }

    pub fn handle_message(&mut self, message: SyncClientMessage) {
        match message {
            SyncClientMessage::CodedSymbols { coded_symbols } => {
                for coded_symbol in coded_symbols {
                    self.decoder.add_coded_symbol(&coded_symbol);
                }

                let _ = self.decoder.try_decode(); // TODO
                if self.decoder.decoded() {
                    let server_missing = self
                        .decoder
                        .get_remote_symbols()
                        .into_iter()
                        .map(|hashed| hashed.symbol.id)
                        .collect::<HashSet<_>>();
                    let client_missing = self
                        .decoder
                        .get_local_symbols()
                        .into_iter()
                        .map(|hashed| hashed.symbol.id)
                        .collect::<HashSet<_>>();

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
        self.result.is_some()
    }

    /// Finishes syncing, returning the difference to apply locally.
    ///
    /// `is_finished` must return true before calling `finish`
    pub fn finish(self) -> HashMap<EntityId, EntityData> {
        debug_assert!(self.is_finished());
        self.result.expect("should be finished")
    }
}

#[cfg(test)]
mod test {
    use crate::graph::{
        SyncClientMessage, SyncClientProtocol, SyncServerMessage, SyncServerProtocol,
    };
    use hegel::{TestCase, generators as gs};
    use std::collections::{HashMap, HashSet};
    use stellar_graph::{
        entity::{AuthorId, EntityId, EntityKind, Timestamp, Version},
        store::{EntityData, EntityMetadataValue},
    };
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
                match &message {
                    SyncClientMessage::Difference { .. } => {
                        count_client_message_difference += 1;
                    }
                    _ => {}
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

    #[hegel::composite]
    fn gen_entity_id(tc: TestCase) -> EntityId {
        EntityId::new(Uuid::from_u128(tc.draw(gs::integers().min_value(1))))
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
