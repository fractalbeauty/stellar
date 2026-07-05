use riblt::{CodedSymbol, HashedSymbol};
use siphasher::sip::SipHasher;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    hash::Hasher,
    iter,
};
use stellar_graph::{
    entity::{AttributeKind, EntityId, Value, Version},
    store::EntityData,
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct EntitySymbol {
    id: EntityId,
    hash: u64,
}

impl EntitySymbol {
    fn new(id: EntityId, data: EntityData, key0: u64, key1: u64) -> Self {
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

struct SyncClientProtocol {
    encoder: riblt::Encoder<EntitySymbol>,
    outbox: VecDeque<SyncClientMessage>,
    missing: Option<HashSet<EntityId>>,
}

enum SyncClientMessage {
    CodedSymbols(Vec<CodedSymbol<EntitySymbol>>),
}

impl SyncClientProtocol {
    const INITIAL_CODED_SYMBOLS: usize = 10;
    const BATCH_CODED_SYMBOLS: usize = 10;

    fn new_from_data(entities: HashMap<EntityId, EntityData>) -> Self {
        Self::new_from_symbols(
            entities
                .into_iter()
                .map(|(entity, data)| EntitySymbol::new(entity, data, 123, 456)),
        )
    }

    /// `symbols` should be unique
    fn new_from_symbols(symbols: impl IntoIterator<Item = EntitySymbol>) -> Self {
        let mut encoder = riblt::Encoder::new();
        for symbol in symbols {
            encoder.add_symbol(&symbol);
        }

        let initial_coded_symbols = iter::repeat_with(|| encoder.produce_next_coded_symbol())
            .take(Self::INITIAL_CODED_SYMBOLS)
            .collect::<Vec<_>>();

        let mut outbox = VecDeque::new();
        outbox.push_back(SyncClientMessage::CodedSymbols(initial_coded_symbols));

        Self {
            encoder,
            outbox,
            missing: None,
        }
    }

    fn handle_message(&mut self, message: SyncServerMessage) {
        match message {
            SyncServerMessage::More => {
                let initial_coded_symbols =
                    iter::repeat_with(|| self.encoder.produce_next_coded_symbol())
                        .take(Self::BATCH_CODED_SYMBOLS)
                        .collect::<Vec<_>>();

                self.outbox
                    .push_back(SyncClientMessage::CodedSymbols(initial_coded_symbols));
            }
            SyncServerMessage::Done { missing } => {
                self.missing = Some(missing);
            }
        }
    }

    fn poll_message(&mut self) -> Option<SyncClientMessage> {
        self.outbox.pop_front()
    }

    fn is_finished(&self) -> bool {
        self.missing.is_some()
    }

    fn poll_finish(&mut self) -> Option<HashSet<EntityId>> {
        self.missing.take()
    }
}

struct SyncServerProtocol {
    decoder: riblt::Decoder<EntitySymbol>,
    outbox: VecDeque<SyncServerMessage>,
    missing: Option<HashSet<EntityId>>,
}

enum SyncServerMessage {
    More,
    Done { missing: HashSet<EntityId> },
}

impl SyncServerProtocol {
    fn new_from_data(entities: HashMap<EntityId, EntityData>) -> Self {
        Self::new_from_symbols(
            entities
                .into_iter()
                .map(|(entity, data)| EntitySymbol::new(entity, data, 123, 456)),
        )
    }

    /// `symbols` should be unique
    fn new_from_symbols(symbols: impl IntoIterator<Item = EntitySymbol>) -> Self {
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

    fn handle_message(&mut self, message: SyncClientMessage) {
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
                } else {
                    self.outbox.push_back(SyncServerMessage::More);
                }
            }
        }
    }

    fn poll_message(&mut self) -> Option<SyncServerMessage> {
        self.outbox.pop_front()
    }

    fn is_finished(&self) -> bool {
        self.missing.is_some()
    }

    fn poll_finish(&mut self) -> Option<HashSet<EntityId>> {
        self.missing.take()
    }
}

#[cfg(test)]
mod test {
    use crate::graph::{
        EntitySymbol, SyncClientMessage, SyncClientProtocol, SyncServerMessage, SyncServerProtocol,
    };
    use hegel::{TestCase, generators as gs};
    use std::collections::HashSet;
    use stellar_graph::entity::EntityId;
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

        // make symbols with fixed versions
        let client_symbols = client_entities.iter().copied().map(|entity| EntitySymbol {
            id: entity,
            hash: 1234,
        });
        let server_symbols = server_entities.iter().copied().map(|entity| EntitySymbol {
            id: entity,
            hash: 1234,
        });

        let mut client = SyncClientProtocol::new_from_symbols(client_symbols);
        let mut server = SyncServerProtocol::new_from_symbols(server_symbols);
        run_client_and_server(&mut client, &mut server);

        // client should be missing server-only entities
        let server_only_entities = server_entities
            .difference(&client_entities)
            .copied()
            .collect();
        let client_missing = client.poll_finish().expect("should be finished");
        assert_eq!(
            client_missing, server_only_entities,
            "client should be missing server-only entities"
        );

        // server should be missing client-only entities
        let client_only_entities = client_entities
            .difference(&server_entities)
            .copied()
            .collect();
        let server_missing = server.poll_finish().expect("should be finished");
        assert_eq!(
            server_missing, client_only_entities,
            "server should be missing client-only entities"
        );
    }

    fn run_client_and_server(client: &mut SyncClientProtocol, server: &mut SyncServerProtocol) {
        let max_iterations = 1000;

        for _ in 0..max_iterations {
            let client_messages = drain_client_messages(client);

            for message in client_messages {
                server.handle_message(message);
            }

            let server_messages = drain_server_messages(server);

            for message in server_messages {
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

    fn drain_client_messages(client: &mut SyncClientProtocol) -> Vec<SyncClientMessage> {
        let mut messages = Vec::new();
        while let Some(message) = client.poll_message() {
            messages.push(message)
        }
        messages
    }

    fn drain_server_messages(server: &mut SyncServerProtocol) -> Vec<SyncServerMessage> {
        let mut messages = Vec::new();
        while let Some(message) = server.poll_message() {
            messages.push(message)
        }
        messages
    }

    #[hegel::composite]
    fn gen_entity_id(tc: TestCase) -> EntityId {
        EntityId::new(Uuid::from_u128(tc.draw(gs::integers().min_value(1))))
    }
}
