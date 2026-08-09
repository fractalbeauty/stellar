use anyhow::Context;
use automerge::{AutoCommit, ROOT, sync::SyncDoc};
use automorph::Automorph;
use futures::{Sink, SinkExt as _, Stream, StreamExt};
use iroh::endpoint::Connection;
use pin_project_lite::pin_project;
use serde::{Deserialize, Serialize};
use std::{
    any::Any,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    time::Duration,
};
use stellar_graph::{entity::AuthorId, schema::Schema};
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::Sleep,
};
use tokio_util::{
    bytes::Bytes,
    codec::{FramedRead, FramedWrite, LengthDelimitedCodec},
    sync::CancellationToken,
};

use crate::{peers::PeersSchemaPort, protocol::StreamHeader};

/// Handle to the schema store task.
#[derive(Debug, Clone)]
pub struct SchemaStoreTask {
    schema_rx: watch::Receiver<Option<Schema>>,
    message_tx: mpsc::UnboundedSender<SchemaStoreMessage>,
}

impl SchemaStoreTask {
    /// `data_dir` is the directory the schema file should be persisted to.
    pub fn spawn(
        cancellation_token: CancellationToken,
        data_dir: impl AsRef<Path>,
        author: AuthorId,
    ) -> Result<Self, anyhow::Error> {
        let store_path = data_dir.as_ref().join("schema_doc");

        let (schema_tx, schema_rx) = watch::channel(None);
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        tokio::spawn({
            async move {
                let mut store = SchemaStore { store_path };

                let result = store
                    .run(author, schema_tx, message_rx, cancellation_token)
                    .await;

                if let Err(error) = result {
                    tracing::error!("SchemaStore task errored: {error}");
                } else {
                    tracing::debug!("SchemaStore task finished");
                }
            }
        });

        Ok(Self {
            schema_rx,
            message_tx,
        })
    }

    pub async fn modify<F, R>(&self, operation: F) -> Result<R, anyhow::Error>
    where
        F: FnOnce(&mut Schema) -> R + Send + 'static,
        R: Send + 'static,
    {
        let operation = wrap_modify_closure(operation);

        let (result_tx, result_rx) = oneshot::channel();

        self.message_tx
            .send(SchemaStoreMessage::Modify {
                operation,
                result_tx,
            })
            .map_err(|e| anyhow::anyhow!("Failed to send SchemaStoreMessage::Modify: {e:?}"))?;

        let result_any = result_rx
            .await
            .context("Failed to get SchemaStoreMessage::Modify result")?
            .ok_or_else(|| anyhow::anyhow!("SchemaStoreMessage::Modify failed"))?;

        let result = result_any
            .downcast::<R>()
            .map_err(|_| anyhow::anyhow!("Failed to downcast SchemaStoreMessage::Modify result"))?;
        Ok(*result)
    }

    pub fn watch_schema(&self) -> watch::Receiver<Option<Schema>> {
        self.schema_rx.clone()
    }

    pub async fn fork_doc_for_sync(&self) -> Result<AutoCommit, anyhow::Error> {
        let (result_tx, result_rx) = oneshot::channel();

        self.message_tx
            .send(SchemaStoreMessage::ForkDocForSync { result_tx })
            .map_err(|_| anyhow::anyhow!("Failed to send SchemaStoreMessage::ForkDocForSync"))?;

        let result = result_rx
            .await
            .context("Failed to get SchemaStoreMessage::ForkDocForSync result")?
            .context("SchemaStoreMessage::ForkDocForSync failed")?;

        Ok(result)
    }

    pub async fn merge_doc_for_sync(&self, doc: AutoCommit) -> Result<(), anyhow::Error> {
        let (result_tx, result_rx) = oneshot::channel();

        self.message_tx
            .send(SchemaStoreMessage::MergeDocForSync {
                doc: Box::new(doc),
                result_tx,
            })
            .map_err(|_| anyhow::anyhow!("Failed to send SchemaStoreMessage::MergeDocForSync"))?;

        result_rx
            .await
            .context("Failed to get SchemaStoreMessage::MergeDocForSync result")?
            .context("SchemaStoreMessage::MergeDocForSync failed")?;

        Ok(())
    }
}

fn wrap_modify_closure<F, R>(
    operation: F,
) -> Box<dyn FnOnce(&mut Schema) -> Box<dyn Any + Send> + Send>
where
    F: FnOnce(&mut Schema) -> R + Send + 'static,
    R: Send + 'static,
{
    Box::new(move |schema| -> Box<dyn Any + Send> {
        let result = operation(schema);
        Box::new(result)
    })
}

/// Owned state for the schema store.
struct SchemaStore {
    store_path: PathBuf,
}

impl SchemaStore {
    async fn run(
        &mut self,
        author: AuthorId,
        schema_tx: watch::Sender<Option<Schema>>,
        mut message_rx: mpsc::UnboundedReceiver<SchemaStoreMessage>,
        cancellation_token: CancellationToken,
    ) -> Result<(), anyhow::Error> {
        tracing::debug!("SchemaStore starting");

        let mut doc = match tokio::fs::try_exists(&self.store_path).await {
            Ok(true) => {
                tracing::debug!("Schema store path exists, reading existing doc");

                let doc_bytes = tokio::fs::read(&self.store_path)
                    .await
                    .context("Failed to read schema store path")?;
                AutoCommit::load(&doc_bytes)
                    .context("Schema store was read but failed to load")?
                    .with_actor(author.inner().into())
            }
            _ => {
                tracing::debug!(
                    "Schema store file does not exist or is inaccessible, creating new doc"
                );

                let mut doc = AutoCommit::new().with_actor(author.inner().into());

                let initial_schema = Schema::default();
                initial_schema
                    .save(&mut doc, &ROOT, "schema")
                    .context("Failed to save initial schema to doc")?;

                self.save_doc(&mut doc).await?;

                doc
            }
        };

        {
            let schema =
                Schema::load(&doc, &ROOT, "schema").context("Failed to load schema from doc")?;

            schema_tx.send_replace(Some(schema));
        }

        let batch = None;
        tokio::pin!(batch);

        loop {
            tokio::select! {
                result = message_rx.recv() => {
                    match result {
                        Some(message) => {
                            if let Err(e) = self.handle_message(&schema_tx, &mut doc, &mut batch, message).await {
                                tracing::error!("SchemaStore failed to handle message: {e:#}");
                            }
                        },
                        None => {
                            tracing::debug!("SchemaStore message rx closed");
                        },
                    }
                }

                _ = SchemaEditBatch::save_timeout(&mut batch) => {
                    tracing::debug!("SchemaEditBatch timed out, applying changes");
                    if let Err(e) = self.handle_batch_save_timeout(&mut doc, &mut batch, &schema_tx).await {
                        tracing::error!("SchemaStore failed to handle batch save timeout: {e:#}");
                    }
                }

                _ = cancellation_token.cancelled() => {
                    tracing::debug!("SchemaStore task cancelled");
                    break;
                }
            }
        }

        Ok(())
    }

    async fn handle_message(
        &mut self,
        schema_tx: &watch::Sender<Option<Schema>>,
        doc: &mut AutoCommit,
        batch: &mut Pin<&mut Option<SchemaEditBatch>>,
        message: SchemaStoreMessage,
    ) -> Result<(), anyhow::Error> {
        match message {
            SchemaStoreMessage::Modify {
                operation,
                result_tx,
            } => {
                match batch.as_mut().as_pin_mut() {
                    Some(batch) => {
                        let projection = batch.project();

                        let result = operation(projection.schema);
                        let _ = result_tx.send(Some(result));

                        tracing::debug!("SchemaStore applied SchemaStoreMessage::Modify")
                    }
                    None => {
                        let mut schema = match Schema::load(doc, &ROOT, "schema")
                            .context("Failed to load schema from doc")
                        {
                            Ok(schema) => schema,
                            Err(e) => {
                                let _ = result_tx.send(None);
                                return Err(e);
                            }
                        };

                        let result = operation(&mut schema);
                        let _ = result_tx.send(Some(result));

                        batch.set(Some(SchemaEditBatch {
                            schema,
                            save_timeout: tokio::time::sleep(Duration::from_secs(1)),
                        }));

                        tracing::debug!(
                            "SchemaStore started SchemaEditBatch and applied SchemaStoreMessage::Modify"
                        );
                    }
                }

                Ok(())
            }
            SchemaStoreMessage::ForkDocForSync { result_tx } => {
                let _ = result_tx.send(Ok(doc.fork()));
                Ok(())
            }
            SchemaStoreMessage::MergeDocForSync {
                doc: mut incoming_doc,
                result_tx,
            } => {
                match doc.merge(&mut incoming_doc) {
                    Ok(_) => {}
                    Err(e) => {
                        let _ = result_tx.send(Err(anyhow::anyhow!("Failed to merge doc: {e:?}")));
                        return Ok(());
                    }
                }

                let schema = match Schema::load(doc, &ROOT, "schema")
                    .context("Failed to load schema from doc")
                {
                    Ok(schema) => schema,
                    Err(e) => {
                        let _ = result_tx.send(Err(anyhow::anyhow!(
                            "Failed to load doc after merge: {e:?}"
                        )));
                        return Err(e);
                    }
                };

                schema_tx.send_replace(Some(schema));

                self.save_doc(doc).await?;

                // TODO: sync?

                let _ = result_tx.send(Ok(()));
                Ok(())
            }
        }
    }

    async fn handle_batch_save_timeout(
        &mut self,
        doc: &mut AutoCommit,
        batch: &mut Pin<&mut Option<SchemaEditBatch>>,
        schema_tx: &watch::Sender<Option<Schema>>,
    ) -> Result<(), anyhow::Error> {
        match batch.as_mut().as_pin_mut() {
            Some(batch) => {
                let projection = batch.project();

                projection
                    .schema
                    .save(doc, &ROOT, "schema")
                    .context("Failed to save schema to doc")?;

                schema_tx.send_replace(Some(projection.schema.clone()));

                self.save_doc(doc).await?;

                // TODO: sync
            }
            None => {
                tracing::error!("SchemaStore handle_batch_save_timeout called but batch is None");
            }
        }

        batch.set(None);

        Ok(())
    }

    async fn save_doc(&mut self, doc: &mut AutoCommit) -> Result<(), anyhow::Error> {
        let doc_bytes = doc.save();
        tokio::fs::write(&self.store_path, doc_bytes)
            .await
            .context("Failed to write schema store path")?;
        tracing::debug!("SchemaStore saved doc to file");
        Ok(())
    }
}

enum SchemaStoreMessage {
    Modify {
        operation: Box<dyn FnOnce(&mut Schema) -> Box<dyn Any + Send> + Send>,
        result_tx: oneshot::Sender<Option<Box<dyn Any + Send>>>,
    },
    ForkDocForSync {
        result_tx: oneshot::Sender<Result<AutoCommit, anyhow::Error>>,
    },
    MergeDocForSync {
        doc: Box<AutoCommit>,
        result_tx: oneshot::Sender<Result<(), anyhow::Error>>,
    },
}

pin_project! {
    struct SchemaEditBatch {
        schema: Schema,
        #[pin]
        save_timeout: Sleep,
    }
}

impl SchemaEditBatch {
    async fn save_timeout(batch: &mut Pin<&mut Option<Self>>) {
        if let Some(batch) = batch.as_mut().as_pin_mut() {
            let projection = batch.project();
            projection.save_timeout.await;
        } else {
            std::future::pending::<()>().await;
        }
    }
}

/// Handle for a peer schema sync client task
pub struct PeerSchemaClientTask {}

impl PeerSchemaClientTask {
    pub fn spawn(schema: Arc<dyn PeersSchemaPort>, connection: Connection) -> Self {
        tokio::spawn({
            async move {
                let result = Self::run(schema, connection).await;

                if let Err(error) = result {
                    tracing::error!("Peer schema client task errored: {error}");
                } else {
                    tracing::debug!("Peer schema client task finished");
                }
            }
        });

        Self {}
    }

    async fn run(
        schema: Arc<dyn PeersSchemaPort>,
        connection: Connection,
    ) -> Result<(), anyhow::Error> {
        tracing::debug!("PeerSchemaClient starting");

        let (tx, rx) = connection.open_bi().await?;

        let mut tx = FramedWrite::new(tx, LengthDelimitedCodec::new());
        let rx = FramedRead::new(rx, LengthDelimitedCodec::new());

        let stream_header = StreamHeader::encode(&StreamHeader::SchemaSync);
        tx.send(stream_header)
            .await
            .context("Failed to send stream header")?;

        let mut tx = Box::new(tx.with(|message| {
            futures::future::ready(Ok::<_, std::io::Error>(SchemaSyncClientMessage::encode(
                &message,
            )))
        }));
        let mut rx = Box::new(rx.map(|result| match result {
            Ok(bytes) => SchemaSyncServerMessage::decode(&bytes),
            Err(e) => Err(anyhow::anyhow!("Failed to read from stream: {e:?}")),
        }));

        let mut doc = schema.fork_doc().await?;
        let mut sync_doc = doc.sync();

        let mut state = automerge::sync::State::new();

        loop {
            let (client_message, client_done) = match sync_doc.generate_sync_message(&mut state) {
                Some(sync_message) => (
                    SchemaSyncClientMessage::Message(sync_message.encode()),
                    false,
                ),
                None => (SchemaSyncClientMessage::Done, true),
            };

            tx.send(client_message).await?;

            let server_message = rx
                .next()
                .await
                .ok_or_else(|| anyhow::anyhow!("SchemaSyncServerMessage stream closed"))??;

            let server_done = match server_message {
                SchemaSyncServerMessage::Message(encoded) => {
                    let sync_message = automerge::sync::Message::decode(&encoded)?;

                    sync_doc.receive_sync_message(&mut state, sync_message)?;

                    false
                }
                SchemaSyncServerMessage::Done => true,
            };

            if client_done && server_done {
                break;
            }
        }

        drop(sync_doc);
        schema.merge_doc(doc).await?;

        tracing::debug!("PeerSchemaClient finished");

        Ok(())
    }
}

/// Handle for a peer schema sync server task
pub struct PeerSchemaServerTask {}

impl PeerSchemaServerTask {
    pub fn spawn(
        schema: Arc<dyn PeersSchemaPort>,
        tx: Pin<Box<dyn Sink<SchemaSyncServerMessage, Error = std::io::Error> + Send>>,
        rx: Pin<Box<dyn Stream<Item = Result<SchemaSyncClientMessage, anyhow::Error>> + Send>>,
    ) -> Self {
        tokio::spawn({
            async move {
                let result = Self::run(schema, tx, rx).await;

                if let Err(error) = result {
                    tracing::error!("Peer schema server task errored: {error}");
                } else {
                    tracing::debug!("Peer schema server task finished");
                }
            }
        });

        Self {}
    }

    async fn run(
        schema: Arc<dyn PeersSchemaPort>,
        mut tx: Pin<Box<dyn Sink<SchemaSyncServerMessage, Error = std::io::Error> + Send>>,
        mut rx: Pin<Box<dyn Stream<Item = Result<SchemaSyncClientMessage, anyhow::Error>> + Send>>,
    ) -> Result<(), anyhow::Error> {
        tracing::debug!("PeerSchemaServer starting");

        let mut doc = schema.fork_doc().await?;
        let mut sync_doc = doc.sync();

        let mut state = automerge::sync::State::new();

        loop {
            let client_message = rx
                .next()
                .await
                .ok_or_else(|| anyhow::anyhow!("SchemaSyncClientMessage stream closed"))??;

            let client_done = match client_message {
                SchemaSyncClientMessage::Message(encoded) => {
                    let sync_message = automerge::sync::Message::decode(&encoded)?;

                    sync_doc.receive_sync_message(&mut state, sync_message)?;

                    false
                }
                SchemaSyncClientMessage::Done => true,
            };

            let (server_message, server_done) = match sync_doc.generate_sync_message(&mut state) {
                Some(sync_message) => (
                    SchemaSyncServerMessage::Message(sync_message.encode()),
                    false,
                ),
                None => (SchemaSyncServerMessage::Done, true),
            };

            tx.send(server_message).await?;

            if client_done && server_done {
                break;
            }
        }

        drop(sync_doc);
        schema.merge_doc(doc).await?;

        tracing::debug!("PeerSchemaServer finished");

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SchemaSyncClientMessage {
    Message(Vec<u8>),
    Done,
}

impl SchemaSyncClientMessage {
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
pub enum SchemaSyncServerMessage {
    Message(Vec<u8>),
    Done,
}

impl SchemaSyncServerMessage {
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

mod test {
    use crate::schema::SchemaStoreTask;
    use std::collections::HashMap;
    use stellar_graph::{
        entity::{AuthorId, EntityKind},
        schema::EntitySchema,
    };
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn modify() {
        tracing_subscriber::fmt::init();

        let cancellation_token = CancellationToken::new();
        let dir = testdir::testdir!();
        tracing::debug!(?dir, "testdir");
        let task =
            SchemaStoreTask::spawn(cancellation_token, dir, AuthorId::new([0u8; 32])).unwrap();

        let mut watch_schema = task.watch_schema();
        assert!(watch_schema.borrow().is_none());

        watch_schema.changed().await.unwrap();
        assert!(watch_schema.borrow().is_some());
        assert!(watch_schema.borrow().as_ref().unwrap().entities.is_empty());

        let entity_kind = EntityKind::random();
        task.modify(move |schema| {
            schema.entities.insert(
                entity_kind,
                EntitySchema {
                    name: "entity".to_string(),
                    attributes: HashMap::new(),
                },
            );
        })
        .await
        .unwrap();

        watch_schema.changed().await.unwrap();
        assert!(watch_schema.borrow().is_some());
        assert!(
            watch_schema
                .borrow()
                .as_ref()
                .unwrap()
                .entities
                .contains_key(&entity_kind)
        );
    }

    #[tokio::test]
    async fn modify_batches_operations() {
        tracing_subscriber::fmt::init();

        let cancellation_token = CancellationToken::new();
        let dir = testdir::testdir!();
        tracing::debug!(?dir, "testdir");
        let task =
            SchemaStoreTask::spawn(cancellation_token, dir, AuthorId::new([0u8; 32])).unwrap();

        let mut watch_schema = task.watch_schema();
        assert!(watch_schema.borrow().is_none());

        watch_schema.changed().await.unwrap();
        assert!(watch_schema.borrow().is_some());
        assert!(watch_schema.borrow().as_ref().unwrap().entities.is_empty());

        task.modify(move |schema| {
            schema.entities.insert(
                EntityKind::random(),
                EntitySchema {
                    name: "entity1".to_string(),
                    attributes: HashMap::new(),
                },
            );
        })
        .await
        .unwrap();
        task.modify(move |schema| {
            schema.entities.insert(
                EntityKind::random(),
                EntitySchema {
                    name: "entity2".to_string(),
                    attributes: HashMap::new(),
                },
            );
        })
        .await
        .unwrap();

        watch_schema.changed().await.unwrap();
        assert!(watch_schema.borrow().is_some());
        assert_eq!(watch_schema.borrow().as_ref().unwrap().entities.len(), 2);
    }
}
