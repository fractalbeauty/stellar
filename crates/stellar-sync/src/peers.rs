use crate::{
    devices::Device,
    graph::{
        DifferenceClientMessage, DifferenceServerMessage, PeerDifferenceServerTask,
        PeerSyncClientTask, PeerSyncServerTask, SyncManager, SyncServerMessage,
    },
    protocol::StreamHeader,
};
use anyhow::Context;
use futures::{SinkExt as _, StreamExt as _};
use iroh::{Endpoint, EndpointId, SecretKey, endpoint::Connection};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use stellar_graph::{database::Database, entity::EntityId, store::EntityData};
use tokio::sync::{mpsc, watch};
use tokio_util::{
    codec::{FramedRead, FramedWrite, LengthDelimitedCodec},
    sync::CancellationToken,
};

/// Handle to the peers task
#[derive(Debug, Clone)]
pub struct PeersTask {
    cancellation_token: CancellationToken,
    endpoint_id: EndpointId,
}

impl PeersTask {
    pub fn spawn(
        cancellation_token: CancellationToken,
        database: Arc<dyn PeersDatabasePort>,
        devices_rx: watch::Receiver<Vec<Device>>,
        secret_key: SecretKey,
    ) -> Result<Self, anyhow::Error> {
        let endpoint_id = secret_key.public();

        tokio::spawn({
            let cancellation_token = cancellation_token.clone();
            async move {
                let mut peers = Peers {
                    peer_tasks: HashMap::new(),
                };

                let result = peers
                    .run(devices_rx, cancellation_token, database, secret_key)
                    .await;

                if let Err(error) = result {
                    tracing::error!("Peers task errored: {error}");
                } else {
                    tracing::debug!("Peers task finished");
                }
            }
        });

        Ok(Self {
            cancellation_token,
            endpoint_id,
        })
    }

    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    pub fn endpoint_id(&self) -> EndpointId {
        self.endpoint_id
    }
}

/// Owned state for the peers task
#[derive(Debug)]
struct Peers {
    peer_tasks: HashMap<EndpointId, PeerTask>,
}

impl Peers {
    async fn run(
        &mut self,
        mut devices_rx: watch::Receiver<Vec<Device>>,
        cancellation_token: CancellationToken,
        database: Arc<dyn PeersDatabasePort>,
        secret_key: SecretKey,
    ) -> Result<(), anyhow::Error> {
        tracing::debug!(
            endpoint_id = ?secret_key.public(), "Peers starting");

        let sync_manager = SyncManager::new();

        let endpoint = Endpoint::builder(iroh::endpoint::presets::N0)
            .secret_key(secret_key)
            .alpns(vec![ALPN.to_vec()])
            .bind()
            .await?;

        let (connection_tx, mut connection_rx) =
            tokio::sync::mpsc::unbounded_channel::<(PeerSide, Connection)>();
        let (peer_exited_tx, mut peer_exited_rx) =
            tokio::sync::mpsc::unbounded_channel::<ConnectionId>();

        let mut next_connection_id = 1;

        loop {
            tokio::select! {
                Some(incoming) = endpoint.accept() => {
                    tokio::task::spawn({
                        let connection_tx = connection_tx.clone();
                        async move {
                            let connection = match incoming.await {
                                Ok(connection) => connection,
                                Err(error) => {
                                    tracing::error!("Failed to accept incoming connection: {error}");
                                    return;
                                }
                            };

                            tracing::debug!("Accepted incoming connection from endpoint {}", connection.remote_id());

                            let _ = connection_tx.send((PeerSide::Incoming, connection));
                        }
                    });
                }

                _ = devices_rx.changed() => {
                    let devices = devices_rx.borrow().clone();
                    let device_endpoint_ids: Vec<EndpointId> = devices.iter().map(|device| device.endpoint_id).collect();

                    // Disconnect peers that are no longer in the devices list
                    for (endpoint_id, peer_task) in &self.peer_tasks {
                        if !device_endpoint_ids.contains(endpoint_id) {
                            tracing::debug!("Endpoint {} is connected but not in devices list, cancelling", endpoint_id);

                            peer_task.cancel();
                        }
                    }

                    // Connect to peers that are in the devices list but not connected
                    for device in &devices {
                        let remote_id = device.endpoint_id;

                        if !self.peer_tasks.contains_key(&remote_id) {
                            tracing::debug!("Endpoint {} is in the devices list but not connected, connecting", remote_id);

                            tokio::task::spawn({
                                let endpoint = endpoint.clone();
                                let connection_tx = connection_tx.clone();
                                async move {
                                    let connection = match endpoint.connect(remote_id, ALPN).await {
                                        Ok(connection) => connection,
                                        Err(error) => {
                                            tracing::error!("Failed to connect to peer {}: {error}", remote_id);
                                            return;
                                        }
                                    };

                                    tracing::debug!("Opened connection to endpoint {}", remote_id);

                                    let _ = connection_tx.send((PeerSide::Outgoing, connection));
                                }
                            });
                        }
                    }
                }

                // Spawn peer tasks for new connections
                Some((side, connection)) = connection_rx.recv() => {
                    let connection_id = ConnectionId {
                        id: next_connection_id,
                        endpoint_id: connection.remote_id(),
                    };
                    next_connection_id += 1;

                    if self.peer_tasks.contains_key(&connection_id.endpoint_id) {
                        tracing::debug!("Already connected to endpoint {}, cancelling existing connection", connection_id.endpoint_id);
                        self.peer_tasks.get(&connection_id.endpoint_id).unwrap().cancel();
                    }

                    let peer = PeerTask::spawn(side, cancellation_token.child_token(), database.clone(), sync_manager.clone(), peer_exited_tx.clone(), connection_id, connection);
                    self.peer_tasks.insert(peer.endpoint_id(), peer);
                }

                // Remove exited peer tasks
                Some(connection_id) = peer_exited_rx.recv() => {
                    if self.peer_tasks.get(&connection_id.endpoint_id).is_some_and(|peer_task| peer_task.connection_id == connection_id) {
                        tracing::debug!("Received peer exited for {:?}, removing from peer tasks", connection_id);
                        self.peer_tasks.remove(&connection_id.endpoint_id);
                    } else if self.peer_tasks.contains_key(&connection_id.endpoint_id) {
                        tracing::warn!("Received peer exited for {:?} but connection ID does not match, ignoring", connection_id);
                    } else {
                        tracing::warn!("Received peer exited for {:?} but no matching peer task found", connection_id);
                    }
                }

                _ = cancellation_token.cancelled() => {
                    tracing::debug!("Peers task cancelled");

                    let endpoint = endpoint.clone();
                    tokio::spawn(async move {
                        endpoint.close().await;
                    });
                }

                _ = endpoint.closed() => {
                    tracing::debug!("Endpoint closed");
                    break;
                }
            }
        }

        Ok(())
    }
}

/// Handle for a peer task
#[derive(Debug)]
struct PeerTask {
    cancellation_token: CancellationToken,
    connection_id: ConnectionId,
}

impl PeerTask {
    pub fn spawn(
        side: PeerSide,
        cancellation_token: CancellationToken,
        database: Arc<dyn PeersDatabasePort + Send + Sync>,
        sync_manager: SyncManager,
        peer_exited_tx: mpsc::UnboundedSender<ConnectionId>,
        connection_id: ConnectionId,
        connection: Connection,
    ) -> Self {
        tokio::spawn({
            let cancellation_token = cancellation_token.clone();
            async move {
                let mut peer = Peer {};

                let result = peer
                    .run(side, cancellation_token, database, sync_manager, connection)
                    .await;

                if let Err(error) = result {
                    tracing::error!("Peer task errored: {error}");
                } else {
                    tracing::debug!("Peer task finished");
                }

                let _ = peer_exited_tx.send(connection_id);
            }
        });

        Self {
            cancellation_token,
            connection_id,
        }
    }

    pub fn endpoint_id(&self) -> EndpointId {
        self.connection_id.endpoint_id
    }

    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }
}

/// Mutable state for a peer task
struct Peer {}

impl Peer {
    async fn run(
        &mut self,
        side: PeerSide,
        cancellation_token: CancellationToken,
        database: Arc<dyn PeersDatabasePort>,
        sync_manager: SyncManager,
        connection: Connection,
    ) -> Result<(), anyhow::Error> {
        tracing::debug!("Peer starting");

        let (mut tx, mut rx) = {
            match side {
                PeerSide::Incoming => connection.accept_bi().await?,
                PeerSide::Outgoing => connection.open_bi().await?,
            }
        };

        // Start syncing if we're the outgoing side
        if side == PeerSide::Outgoing {
            PeerSyncClientTask::spawn(database.clone(), sync_manager.clone(), connection.clone());
        }

        loop {
            tokio::select! {
                _ = cancellation_token.cancelled() => {
                    tracing::debug!("Peer task cancelled");
                    connection.close(1u8.into(), b"close");
                }

                _ = connection.closed() => {
                    tracing::debug!("Connection closed");
                    break;
                }

                // TODO: move to another task, so it doesn't block the Peer loop
                // the task will probably need have clones of everything to hand out
                result = connection.accept_bi() => {
                    match result {
                        Ok((tx, rx)) => {
                            let tx = FramedWrite::new(tx, LengthDelimitedCodec::new());
                            let mut rx = FramedRead::new(rx, LengthDelimitedCodec::new());

                            let Some(stream_header_result) = rx.next().await else {
                                tracing::error!("Stream closed before stream header was read");
                                continue;
                            };
                            let stream_header_bytes = stream_header_result.context("Error reading stream header")?;
                            let stream_header = StreamHeader::decode(&stream_header_bytes).context("Error decoding stream header")?;

                            match stream_header {
                                StreamHeader::Sync => {
                                    let tx = Box::pin(tx.with(|message| {
                                        futures::future::ready(Ok::<_, std::io::Error>(SyncServerMessage::encode(&message)))
                                    }));
                                    PeerSyncServerTask::spawn(database.clone(), sync_manager.clone(), tx);
                                },
                                StreamHeader::Difference => {
                                    let tx = Box::pin(tx.with(|message| {
                                        futures::future::ready(Ok::<_, std::io::Error>(DifferenceServerMessage::encode(&message)))
                                    }));
                                    let rx = Box::pin( rx.map(|result| match result {
                                        Ok(bytes) => DifferenceClientMessage::decode(&bytes),
                                        Err(e) => Err(anyhow::anyhow!("Failed to read from stream: {e:?}")),
                                    }));
                                    PeerDifferenceServerTask::spawn(database.clone(), tx, rx);
                                },
                            }
                        }
                        Err(e) => {
                            tracing::error!("Error accepting stream: {e:?}");
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

pub trait PeersDatabasePort: Send + Sync {
    fn get_entities(&self) -> Result<HashMap<EntityId, EntityData>, anyhow::Error>;

    fn get_entities_by_id(
        &self,
        entities: HashSet<EntityId>,
    ) -> Result<HashMap<EntityId, EntityData>, anyhow::Error>;

    fn upsert_entities(&self, entities: HashMap<EntityId, EntityData>)
    -> Result<(), anyhow::Error>;
}

pub struct PeersDatabaseAdapter {
    database: Database,
}

impl PeersDatabaseAdapter {
    pub fn new(database: Database) -> Arc<Self> {
        Arc::new(Self { database })
    }
}

impl PeersDatabasePort for PeersDatabaseAdapter {
    fn get_entities(&self) -> Result<HashMap<EntityId, EntityData>, anyhow::Error> {
        self.database.get_entities()
    }

    fn get_entities_by_id(
        &self,
        entities: HashSet<EntityId>,
    ) -> Result<HashMap<EntityId, EntityData>, anyhow::Error> {
        // TODO: optimize this
        let mut all_entities = self.get_entities()?;
        Ok(entities
            .into_iter()
            .filter_map(|entity| all_entities.remove(&entity).map(|data| (entity, data)))
            .collect())
    }

    fn upsert_entities(
        &self,
        entities: HashMap<EntityId, EntityData>,
    ) -> Result<(), anyhow::Error> {
        // TODO: batch this somewhere (maybe a level above this; inside peer, outside database)
        for (entity, data) in entities {
            self.database.upsert_entity(entity, data)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PeerSide {
    Incoming,
    Outgoing,
}

/// Connection ID used to distinguish between multiple connections to the same endpoint ID.
///
/// Also carries the endpoint ID for convenience.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ConnectionId {
    id: u32,
    endpoint_id: EndpointId,
}

const ALPN: &[u8] = b"stellar-sync/1";
