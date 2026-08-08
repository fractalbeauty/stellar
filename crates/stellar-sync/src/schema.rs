use anyhow::Context;
use automerge::{AutoCommit, ROOT};
use automorph::Automorph;
use pin_project_lite::pin_project;
use std::{
    any::Any,
    path::{Path, PathBuf},
    pin::Pin,
    time::Duration,
};
use stellar_graph::{entity::AuthorId, schema::Schema};
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::Sleep,
};
use tokio_util::sync::CancellationToken;

/// Handle to the schema store task.
#[derive(Debug, Clone)]
pub struct SchemaStoreTask {
    schema_rx: watch::Receiver<Option<Schema>>,
    event_tx: mpsc::UnboundedSender<SchemaStoreEvent>,
}

impl SchemaStoreTask {
    /// `dir` is the directory the schema file should be persisted to.
    pub fn spawn(
        cancellation_token: CancellationToken,
        dir: impl AsRef<Path>,
        author: AuthorId,
    ) -> Result<Self, anyhow::Error> {
        let store_path = dir.as_ref().join("schema_doc");

        let (schema_tx, schema_rx) = watch::channel(None);
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        tokio::spawn({
            async move {
                let mut store = SchemaStore { store_path };

                let result = store
                    .run(author, schema_tx, event_rx, cancellation_token)
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
            event_tx,
        })
    }

    pub async fn modify<F, R>(&self, operation: F) -> Result<R, anyhow::Error>
    where
        F: FnOnce(&mut Schema) -> R + Send + 'static,
        R: Send + 'static,
    {
        let operation = wrap_modify_closure(operation);

        let (result_tx, result_rx) = oneshot::channel();

        self.event_tx
            .send(SchemaStoreEvent::Modify {
                operation,
                result_tx,
            })
            .map_err(|e| anyhow::anyhow!("Failed to send SchemaStoreEvent::Modify: {e:?}"))?;

        let result_any = result_rx
            .await
            .context("Failed to get SchemaStoreEvent::Modify result")?
            .ok_or_else(|| anyhow::anyhow!("SchemaStoreEvent::Modify failed"))?;

        let result = result_any
            .downcast::<R>()
            .map_err(|_| anyhow::anyhow!("Failed to downcast SchemaStoreEvent::Modify result"))?;
        Ok(*result)
    }

    pub fn watch_schema(&self) -> watch::Receiver<Option<Schema>> {
        self.schema_rx.clone()
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
        mut event_rx: mpsc::UnboundedReceiver<SchemaStoreEvent>,
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
                result = event_rx.recv() => {
                    match result {
                        Some(event) => {
                            if let Err(e) = self.handle_event(&mut doc, &mut batch, event) {
                                tracing::error!("SchemaStore failed to handle event: {e:#}");
                            }
                        },
                        None => {
                            tracing::debug!("SchemaStore event rx closed");
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

    fn handle_event(
        &mut self,
        doc: &mut AutoCommit,
        batch: &mut Pin<&mut Option<SchemaEditBatch>>,
        event: SchemaStoreEvent,
    ) -> Result<(), anyhow::Error> {
        match event {
            SchemaStoreEvent::Modify {
                operation,
                result_tx,
            } => {
                match batch.as_mut().as_pin_mut() {
                    Some(batch) => {
                        let projection = batch.project();

                        let result = operation(projection.schema);
                        let _ = result_tx.send(Some(result));

                        tracing::debug!("SchemaStore applied SchemaStoreEvent::Modify")
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
                            "SchemaStore started SchemaEditBatch and applied SchemaStoreEvent::Modify"
                        );
                    }
                }

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

enum SchemaStoreEvent {
    Modify {
        operation: Box<dyn FnOnce(&mut Schema) -> Box<dyn Any + Send> + Send>,
        result_tx: oneshot::Sender<Option<Box<dyn Any + Send>>>,
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
