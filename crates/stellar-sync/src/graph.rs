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
    encoder: riblt::Encoder<EntitySymbol>,
    missing: Option<HashSet<EntityId>>,
}

pub enum SyncClientMessage {
    CodedSymbols(Vec<CodedSymbol<EntitySymbol>>),
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
            encoder,
            missing: None,
        }
    }

    pub fn handle_message(&mut self, message: SyncServerMessage) {
        match message {
            SyncServerMessage::Done { missing } => {
                self.missing = Some(missing);
            }
        }
    }

    pub fn poll_message(&mut self) -> Option<SyncClientMessage> {
        // already done
        if self.missing.is_some() {
            return None;
        }

        // produce symbols
        let coded_symbols = iter::repeat_with(|| self.encoder.produce_next_coded_symbol())
            .take(Self::BATCH_CODED_SYMBOLS)
            .collect::<Vec<_>>();
        Some(SyncClientMessage::CodedSymbols(coded_symbols))
    }

    pub fn is_finished(&self) -> bool {
        self.missing.is_some()
    }

    // `is_finished` must return true before calling `finish`
    pub fn finish(self) -> HashSet<EntityId> {
        debug_assert!(self.is_finished());
        self.missing.expect("should be finished")
    }
}

pub struct SyncServerProtocol {
    decoder: riblt::Decoder<EntitySymbol>,
    outbox: VecDeque<SyncServerMessage>,
    missing: Option<HashSet<EntityId>>,
}

pub enum SyncServerMessage {
    Done { missing: HashSet<EntityId> },
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
            decoder,
            outbox: VecDeque::new(),
            missing: None,
        }
    }

    pub fn handle_message(&mut self, message: SyncClientMessage) {
        match message {
            SyncClientMessage::CodedSymbols(coded_symbols) => {
                for coded_symbol in coded_symbols {
                    self.decoder.add_coded_symbol(&coded_symbol);
                }

                let _ = self.decoder.try_decode(); // TODO
                if self.decoder.decoded() {
                    let local_missing = self
                        .decoder
                        .get_remote_symbols()
                        .into_iter()
                        .map(|hashed| hashed.symbol.id)
                        .collect();
                    let remote_missing = self
                        .decoder
                        .get_local_symbols()
                        .into_iter()
                        .map(|hashed| hashed.symbol.id)
                        .collect();

                    self.missing = Some(local_missing);
                    self.outbox.push_back(SyncServerMessage::Done {
                        missing: remote_missing,
                    });
                }
            }
        }
    }

    pub fn poll_message(&mut self) -> Option<SyncServerMessage> {
        self.outbox.pop_front()
    }

    pub fn is_finished(&self) -> bool {
        self.missing.is_some()
    }

    /// `is_finished` must return true before calling `finish`
    pub fn finish(self) -> HashSet<EntityId> {
        debug_assert!(self.is_finished());
        self.missing.expect("should be finished")
    }
}

#[cfg(test)]
mod test {
    use crate::graph::{SyncClientProtocol, SyncServerProtocol};
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
            .map(|entity| (entity, make_empty_entity_data()))
            .collect();
        let server_data = server_entities
            .iter()
            .copied()
            .map(|entity| (entity, make_empty_entity_data()))
            .collect();

        let mut client = SyncClientProtocol::new(client_data);
        let mut server = SyncServerProtocol::new(server_data);
        run_client_and_server(&mut client, &mut server);

        // client should be missing server-only entities
        let server_only_entities = server_entities
            .difference(&client_entities)
            .copied()
            .collect();
        let client_missing = client.finish();
        assert_eq!(
            client_missing, server_only_entities,
            "client should be missing server-only entities"
        );

        // server should be missing client-only entities
        let client_only_entities = client_entities
            .difference(&server_entities)
            .copied()
            .collect();
        let server_missing = server.finish();
        assert_eq!(
            server_missing, client_only_entities,
            "server should be missing client-only entities"
        );
    }

    fn run_client_and_server(client: &mut SyncClientProtocol, server: &mut SyncServerProtocol) {
        let max_iterations = 1000;

        for _ in 0..max_iterations {
            let client_message = client.poll_message();
            if let Some(message) = client_message {
                server.handle_message(message);
            }

            let server_message = server.poll_message();
            if let Some(message) = server_message {
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
