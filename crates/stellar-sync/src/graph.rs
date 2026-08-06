use crate::{peers::PeersDatabasePort, protocol::StreamHeader};
use anyhow::Context;
use futures::{Sink, SinkExt as _, StreamExt as _};
use iroh::endpoint::Connection;
use serde::{Deserialize, Serialize};
use siphasher::sip::SipHasher;
use std::{
    collections::{HashMap, HashSet},
    hash::Hasher,
    pin::Pin,
    sync::Arc,
};
use stellar_graph::{
    entity::{EntityId, Version},
    store::EntityData,
};
use stellar_riblt::{CodedSymbol, PeelableResult, RatelessIBLT, UnmanagedRatelessIBLT};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_util::{
    bytes::Bytes,
    codec::{FramedRead, FramedWrite, LengthDelimitedCodec},
};
use uuid::Uuid;

/// Handle for a peer sync client task
pub struct PeerSyncClientTask {}

impl PeerSyncClientTask {
    pub fn spawn(
        database: Arc<dyn PeersDatabasePort>,
        sync_manager: SyncManager,
        connection: Connection,
    ) -> Self {
        tokio::spawn({
            async move {
                let result = Self::run(database, sync_manager, connection).await;

                if let Err(error) = result {
                    tracing::error!("Peer sync client task errored: {error}");
                } else {
                    tracing::debug!("Peer sync client task finished");
                }
            }
        });

        PeerSyncClientTask {}
    }

    async fn run(
        database: Arc<dyn PeersDatabasePort>,
        sync_manager: SyncManager,
        connection: Connection,
    ) -> Result<(), anyhow::Error> {
        tracing::debug!("PeerSyncClient starting");

        let entities = database.get_entities().context("Failed to get entities")?;
        let mut protocol = SyncClientProtocol::new(entities);

        let _permit = sync_manager
            .acquire()
            .await
            .context("Failed to acquire sync permit")?;

        let (tx, rx) = connection.open_bi().await?;

        let mut tx = FramedWrite::new(tx, LengthDelimitedCodec::new());
        let rx = FramedRead::new(rx, LengthDelimitedCodec::new());

        let stream_header = StreamHeader::encode(&StreamHeader::Sync);
        tx.send(stream_header)
            .await
            .context("Failed to send stream header")?;

        let mut rx = rx.map(|result| match result {
            Ok(bytes) => SyncServerMessage::decode(&bytes),
            Err(e) => Err(anyhow::anyhow!("Failed to read from stream: {e:?}")),
        });

        while let Some(message) = rx.next().await {
            let message = message.context("Failed to receive")?;
            tracing::trace!("PeerSyncClient received {message:?}");

            protocol.handle_message(message);
            if protocol.is_finished() {
                break;
            }
        }

        let _ = tx.get_mut().reset(0u8.into());
        let _ = rx.get_mut().get_mut().stop(0u8.into());

        let (client_missing, server_missing) = protocol.finish();

        // TODO: exchange difference
        dbg!(client_missing, server_missing);

        tracing::debug!("PeerSyncClient finished");

        Ok(())
    }
}

/// Handle for a peer sync server task
pub struct PeerSyncServerTask {}

impl PeerSyncServerTask {
    pub fn spawn(
        database: Arc<dyn PeersDatabasePort>,
        sync_manager: SyncManager,
        tx: Pin<Box<dyn Sink<SyncServerMessage, Error = std::io::Error> + Send>>,
    ) -> Self {
        tokio::spawn({
            async move {
                let result = Self::run(database, sync_manager, tx).await;

                if let Err(error) = result {
                    tracing::error!("Peer sync server task errored: {error}");
                } else {
                    tracing::debug!("Peer sync server task finished");
                }
            }
        });

        PeerSyncServerTask {}
    }

    async fn run(
        database: Arc<dyn PeersDatabasePort>,
        sync_manager: SyncManager,
        mut tx: Pin<Box<dyn Sink<SyncServerMessage, Error = std::io::Error> + Send>>,
    ) -> Result<(), anyhow::Error> {
        tracing::info!("PeerSyncServer starting");

        let entities = database.get_entities().context("Failed to get entities")?;
        let mut protocol = SyncServerProtocol::new(entities);

        let _permit = sync_manager
            .acquire()
            .await
            .context("Failed to acquire sync permit")?;

        loop {
            let Some(message) = protocol.poll_message() else {
                break;
            };

            tracing::trace!("PeerSyncServer sending {message:?}");
            tx.send(message).await.context("Failed to send")?;
        }

        tracing::debug!("PeerSyncServer finished");

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SyncManager {
    semaphore: Arc<Semaphore>,
}

impl SyncManager {
    pub fn new() -> Self {
        SyncManager {
            semaphore: Arc::new(Semaphore::new(1)),
        }
    }

    /// Acquires a permit to sync.
    async fn acquire(&self) -> Result<SyncPermit, anyhow::Error> {
        let permit = self.semaphore.clone().acquire_owned().await?;
        Ok(SyncPermit { inner: permit })
    }
}

impl Default for SyncManager {
    fn default() -> Self {
        Self::new()
    }
}

struct SyncPermit {
    #[allow(unused)]
    inner: OwnedSemaphorePermit,
}

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
    received: UnmanagedRatelessIBLT<EntitySymbol>,
    result: Option<(HashSet<EntityId>, HashSet<EntityId>)>,
}

impl SyncClientProtocol {
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
            result: None,
        }
    }

    pub fn handle_message(&mut self, message: SyncServerMessage) {
        match message {
            SyncServerMessage::CodedSymbols { coded_symbols } => {
                if self.result.is_some() {
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
                                server_missing.insert(symbol.id);
                            }
                            PeelableResult::Remote(symbol) => {
                                client_missing.insert(symbol.id);
                            }
                            PeelableResult::NotPeelable => {
                                // TODO: update stellar-riblt to remove this case
                                unreachable!("peel_all_symbols only returns peeled symbols")
                            }
                        }
                    }

                    self.result = Some((client_missing, server_missing))
                }
            }
        }
    }

    pub fn is_finished(&self) -> bool {
        self.result.is_some()
    }

    /// Finishes syncing, returning (client_missing, server_missing).
    ///
    /// `is_finished` must return true before calling `finish`
    pub fn finish(self) -> (HashSet<EntityId>, HashSet<EntityId>) {
        debug_assert!(self.is_finished());
        self.result.expect("should be finished")
    }
}

pub struct SyncServerProtocol {
    entities: HashMap<EntityId, EntityData>,
    riblt: RatelessIBLT<EntitySymbol, Vec<EntitySymbol>>,
    next_index: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SyncServerMessage {
    CodedSymbols {
        coded_symbols: Vec<CodedSymbol<EntitySymbol>>,
    },
}

impl SyncServerProtocol {
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
            next_index: Some(0),
        }
    }

    /// TODO
    ///
    /// Returns `None` when the stream is finished.
    pub fn poll_message(&mut self) -> Option<SyncServerMessage> {
        let mut coded_symbols = Vec::new();
        for _ in 0..Self::BATCH_CODED_SYMBOLS {
            let Some(index) = self.next_index.as_mut().map(|next_index| {
                let index = *next_index;
                *next_index += 1;
                index
            }) else {
                break;
            };

            let coded_symbol = self.riblt.get_coded_symbol(index);
            if coded_symbol.is_empty() {
                // TODO: this is not actually correct, could be empty without being done
                // self.next_index = None;
            }
            coded_symbols.push(coded_symbol);
        }

        if !coded_symbols.is_empty() {
            Some(SyncServerMessage::CodedSymbols { coded_symbols })
        } else {
            None
        }
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

#[derive(Debug, Serialize, Deserialize)]
pub enum DifferenceClientMessage {
    Done,
    Difference {
        server_difference: HashMap<EntityId, EntityData>,
        client_missing: HashSet<EntityId>,
    },
}

// SyncServerMessage::Difference {
//                 client_difference,
//                 server_missing,
//             } => {
//                 self.result = Some(client_difference);

//                 let server_difference = server_missing.into_iter().filter_map(|entity| {
//                     let Some(data) = self.entities.get(&entity) else {
//                         warn!(
//                             "SyncServerMessage::Difference server_missing entity not in client entities"
//                         );
//                         return None;
//                     };
//                     Some((entity, data.clone()))
//                 }).collect();

//                 self.outbox
//                     .push_back(SyncClientMessage::Difference { server_difference });
//             }

#[derive(Debug, Serialize, Deserialize)]
pub enum DifferenceServerMessage {
    Difference {
        client_difference: HashMap<EntityId, EntityData>,
    },
}

// let client_difference = client_missing
//                         .into_iter()
//                         .filter_map(|entity| {
//                             let Some(data) = self.entities.get(&entity) else {
//                                 warn!(
//                                     "SyncServerProtocol decoded but client_missing entity not in server entities"
//                                 );
//                                 return None;
//                             };
//                             Some((entity, data.clone()))
//                         })
//                         .collect();

//                     self.outbox.push_back(SyncServerMessage::Difference {
//                         client_difference,
//                         server_missing,
//                     });
//                     self.sent_difference = true;

#[cfg(test)]
mod test {
    use crate::graph::{EntitySymbol, SyncClientProtocol, SyncServerProtocol};
    use hegel::{
        Generator, TestCase,
        generators::{self as gs},
    };
    use std::{
        collections::{HashMap, HashSet},
        unreachable,
    };
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

        let client_only_entities = client_entities
            .clone()
            .into_iter()
            .filter(|entity| !server_entities.contains(entity))
            .collect::<HashSet<_, _>>();
        let server_only_entities = server_entities
            .clone()
            .into_iter()
            .filter(|entity| !client_entities.contains(entity))
            .collect::<HashSet<_, _>>();

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

        let client = SyncClientProtocol::new(client_data);
        let server = SyncServerProtocol::new(server_data);
        let (client_missing, server_missing) = run_client_and_server(client, server);

        // client should be missing server-only entities
        assert_eq!(
            client_missing, server_only_entities,
            "client should be missing server-only entities"
        );

        // server should be missing client-only entities
        assert_eq!(
            server_missing, client_only_entities,
            "server should be missing client-only entities"
        );
    }

    fn run_client_and_server(
        mut client: SyncClientProtocol,
        mut server: SyncServerProtocol,
    ) -> (HashSet<EntityId>, HashSet<EntityId>) {
        let max_iterations = 1000;

        for _ in 0..max_iterations {
            let server_message = server.poll_message();
            if let Some(message) = server_message {
                client.handle_message(message);
            }

            // exit when finished
            if client.is_finished() {
                return client.finish();
            }
        }

        // should terminate
        assert!(
            client.is_finished(),
            "sync did not finish in {max_iterations} iterations"
        );
        unreachable!();
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
