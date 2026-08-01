use crate::{
    devices::Device,
    graph::{SyncClientMessage, SyncClientProtocol, SyncServerMessage, SyncServerProtocol},
};
use anyhow::Context;
use futures::{Sink, SinkExt as _, Stream, StreamExt};
use iroh::{Endpoint, EndpointId, SecretKey, endpoint::Connection};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    dbg,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch};
use tokio_util::{
    bytes::Bytes,
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
        devices_rx: watch::Receiver<Vec<Device>>,
    ) -> Result<Self, anyhow::Error> {
        let secret_key = SecretKey::generate();
        let endpoint_id = secret_key.public();

        tokio::spawn({
            let cancellation_token = cancellation_token.clone();
            async move {
                let mut peers = Peers {
                    peer_tasks: HashMap::new(),
                };

                let result = peers.run(devices_rx, cancellation_token, secret_key).await;

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
        secret_key: SecretKey,
    ) -> Result<(), anyhow::Error> {
        tracing::debug!("Peers starting");

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
                        if !device_endpoint_ids.contains(&endpoint_id) {
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

                    let peer = PeerTask::spawn(side, cancellation_token.child_token(), sync_manager.clone(), peer_exited_tx.clone(), connection_id, connection);
                    self.peer_tasks.insert(peer.endpoint_id(), peer);
                }

                // Remove exited peer tasks
                Some(connection_id) = peer_exited_rx.recv() => {
                    if self.peer_tasks.get(&connection_id.endpoint_id).is_some_and(|peer_task| peer_task.connection_id == connection_id) {
                        tracing::debug!("Received peer exited for {:?}, removing from peer tasks", connection_id);
                        self.peer_tasks.remove(&connection_id.endpoint_id);
                    } else if self.peer_tasks.get(&connection_id.endpoint_id).is_some() {
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
                    .run(side, cancellation_token, sync_manager, connection)
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
            let sync_manager = sync_manager.clone();
            let connection = connection.clone();
            PeerSyncClientTask::spawn(sync_manager, connection);
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

                            let tx = Box::pin(tx.with(|message| {
                                futures::future::ready(Ok::<_, std::io::Error>(SyncServerMessage::encode(&message)))
                            }));
                            let rx = Box::pin(rx.map(|result| match result {
                                Ok(bytes) => SyncClientMessage::decode(&bytes),
                                Err(e) => Err(anyhow::anyhow!("Failed to read from stream: {e:?}")),
                            }));

                            match stream_header {
                                StreamHeader::Sync => {
                                    PeerSyncServerTask::spawn(sync_manager.clone(), tx, rx);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PeerSide {
    Incoming,
    Outgoing,
}

/// Handle for a peer sync client task
struct PeerSyncClientTask {}

impl PeerSyncClientTask {
    fn spawn(sync_manager: SyncManager, connection: Connection) -> Self {
        tokio::spawn({
            async move {
                // let entities = HashMap::from([(
                //     stellar_graph::entity::EntityId::random(),
                //     stellar_graph::store::EntityData {
                //         metadata: stellar_graph::store::EntityMetadataValue {
                //             kind: stellar_graph::entity::EntityKind::random(),
                //             deleted: false,
                //             deleted_version: stellar_graph::entity::Version::new(
                //                 stellar_graph::entity::Timestamp::now(),
                //                 stellar_graph::entity::AuthorId::new([0u8; 32]),
                //             ),
                //         },
                //         attributes: HashMap::new(),
                //     },
                // )]);
                let entities = HashMap::new();
                let protocol = Arc::new(Mutex::new(SyncClientProtocol::new(entities)));

                let result = Self::run(protocol, sync_manager, connection).await;

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
        protocol: Arc<Mutex<SyncClientProtocol>>,
        sync_manager: SyncManager,
        connection: Connection,
    ) -> Result<(), anyhow::Error> {
        tracing::debug!("PeerSyncClient starting");

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

        let mut tx = tx.with(|message| {
            futures::future::ready(Ok::<_, std::io::Error>(SyncClientMessage::encode(&message)))
        });
        let mut rx = rx.map(|result| match result {
            Ok(bytes) => SyncServerMessage::decode(&bytes),
            Err(e) => Err(anyhow::anyhow!("Failed to read from stream: {e:?}")),
        });

        let tx_future = {
            let protocol = protocol.clone();
            async move {
                loop {
                    let message = {
                        let mut protocol = protocol.lock().map_err(|e| {
                            anyhow::anyhow!("SyncClientProtocol mutex poisoned: {e:?}")
                        })?;
                        if protocol.is_finished() {
                            break;
                        }
                        protocol.poll_message()
                    };

                    if let Some(message) = message {
                        tracing::trace!("PeerSyncClient sending {message:?}");
                        tx.send(message).await.context("Failed to send")?;
                    } else {
                        tracing::trace!("PeerSyncClient nothing to send");
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }

                Ok::<(), anyhow::Error>(())
            }
        };

        let rx_future = {
            let protocol = protocol.clone();
            async move {
                while let Some(message) = rx.next().await {
                    let message = message.context("Failed to receive")?;
                    tracing::trace!("PeerSyncClient received {message:?}");

                    let mut protocol = protocol
                        .lock()
                        .map_err(|e| anyhow::anyhow!("SyncClientProtocol mutex poisoned: {e:?}"))?;
                    protocol.handle_message(message);
                    if protocol.is_finished() {
                        break;
                    }
                }

                Ok::<(), anyhow::Error>(())
            }
        };

        let (tx_result, rx_result) = tokio::join!(tx_future, rx_future);
        tx_result?;
        rx_result?;

        let Ok(protocol) = Arc::try_unwrap(protocol) else {
            anyhow::bail!("Failed to unwrap SyncClientProtocol Arc");
        };
        let protocol = protocol
            .into_inner()
            .map_err(|e| anyhow::anyhow!("SyncClientProtocol mutex poisoned: {e:?}"))?;

        let result = protocol.finish();

        // TODO: apply changes

        tracing::debug!("PeerSyncClient finished");

        Ok(())
    }
}

/// Handle for a peer sync server task
struct PeerSyncServerTask {}

impl PeerSyncServerTask {
    fn spawn(
        sync_manager: SyncManager,
        tx: Pin<Box<dyn Sink<SyncServerMessage, Error = std::io::Error> + Send>>,
        rx: Pin<Box<dyn Stream<Item = Result<SyncClientMessage, anyhow::Error>> + Send>>,
    ) -> Self {
        tokio::spawn({
            async move {
                // TODO: entities
                let protocol = Arc::new(Mutex::new(SyncServerProtocol::new(HashMap::new())));

                let result = Self::run(protocol, sync_manager, tx, rx).await;

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
        protocol: Arc<Mutex<SyncServerProtocol>>,
        sync_manager: SyncManager,
        mut tx: Pin<Box<dyn Sink<SyncServerMessage, Error = std::io::Error> + Send>>,
        mut rx: Pin<Box<dyn Stream<Item = Result<SyncClientMessage, anyhow::Error>> + Send>>,
    ) -> Result<(), anyhow::Error> {
        tracing::info!("PeerSyncServer starting");

        let _permit = sync_manager
            .acquire()
            .await
            .context("Failed to acquire sync permit")?;

        let tx_future = {
            let protocol = protocol.clone();
            async move {
                loop {
                    let message = {
                        let mut protocol = protocol.lock().map_err(|e| {
                            anyhow::anyhow!("SyncServerProtocol mutex poisoned: {e:?}")
                        })?;
                        if protocol.is_finished() {
                            break;
                        }
                        protocol.poll_message()
                    };

                    if let Some(message) = message {
                        tracing::trace!("PeerSyncServer sending {message:?}");
                        tx.send(message).await.context("Failed to send")?;
                    } else {
                        tracing::trace!("PeerSyncServer nothing to send");
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }

                Ok::<(), anyhow::Error>(())
            }
        };

        let rx_future = {
            let protocol = protocol.clone();
            async move {
                while let Some(message) = rx.next().await {
                    let message = message.context("Failed to receive")?;
                    tracing::trace!("PeerSyncServer received {message:?}");

                    let mut protocol = protocol
                        .lock()
                        .map_err(|e| anyhow::anyhow!("SyncServerProtocol mutex poisoned: {e:?}"))?;
                    protocol.handle_message(message);
                    if protocol.is_finished() {
                        break;
                    }
                }

                Ok::<(), anyhow::Error>(())
            }
        };

        let (tx_result, rx_result) = tokio::join!(tx_future, rx_future);
        tx_result?;
        rx_result?;

        let Ok(protocol) = Arc::try_unwrap(protocol) else {
            anyhow::bail!("Failed to unwrap SyncServerProtocol Arc");
        };
        let protocol = protocol
            .into_inner()
            .map_err(|e| anyhow::anyhow!("SyncServerProtocol mutex poisoned: {e:?}"))?;

        let result = protocol.finish();

        // TODO: apply changes

        tracing::debug!("PeerSyncServer finished");

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum StreamHeader {
    Sync,
}

impl StreamHeader {
    fn encode(message: &Self) -> Bytes {
        postcard::to_stdvec(&message)
            .expect("Failed to serialize message")
            .into()
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, anyhow::Error> {
        postcard::from_bytes(bytes)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize message: {e:?}"))
    }
}

#[derive(Debug, Clone)]
struct SyncManager {
    semaphore: Arc<Semaphore>,
}

impl SyncManager {
    fn new() -> Self {
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

struct SyncPermit {
    #[allow(unused)]
    inner: OwnedSemaphorePermit,
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
