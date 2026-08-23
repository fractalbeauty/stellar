use crate::{
    evaluator::{Evaluator, EvaluatorFile},
    ports::{ImportDatabasePort, ImportSchemaPort},
};
use lofty::file::TaggedFileExt;
use rayon::iter::{ParallelBridge, ParallelIterator};
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use stellar_graph::entity::AuthorId;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Foreign trait for receiving import events.
#[uniffi::export(with_foreign)]
pub trait ImportEventHandler: Send + Sync {
    fn on_pending_file(&self, path: String);
    fn on_scanned_file(&self, file: ImportEventScannedFile);
    fn on_scan_finished(&self);
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ImportEventScannedFile {
    pub path: String,
    pub tags: HashMap<String, String>,
}

/// Handle to an import task.
#[derive(Debug, Clone)]
pub struct ImportTask {
    cancellation_token: CancellationToken,
    message_tx: mpsc::UnboundedSender<ImportMessage>,
}

impl ImportTask {
    pub fn spawn(
        cancellation_token: CancellationToken,
        database: Arc<dyn ImportDatabasePort>,
        schema: Arc<dyn ImportSchemaPort>,
        event_handler: Arc<dyn ImportEventHandler>,
        roots: Vec<PathBuf>,
        author: AuthorId,
    ) -> Result<Self, anyhow::Error> {
        let (message_tx, message_rx) = mpsc::unbounded_channel();

        tokio::spawn({
            let cancellation_token = cancellation_token.clone();
            let message_tx = message_tx.clone();
            async move {
                let mut import = match Import::init(database, schema, author) {
                    Ok(import) => import,
                    Err(e) => {
                        tracing::error!("Import task failed to init: {e:?}");
                        return;
                    }
                };

                let result = import
                    .run(
                        cancellation_token,
                        event_handler,
                        roots,
                        message_tx,
                        message_rx,
                    )
                    .await;

                if let Err(error) = result {
                    tracing::error!("Import task errored: {error:?}");
                } else {
                    tracing::debug!("Import task finished");
                }
            }
        });

        Ok(Self {
            cancellation_token,
            message_tx,
        })
    }

    // Cancel the import task. This must be called when the UI is done with the import,
    // or the import task will leak.
    //
    // We can't really do this automatically on drop because the import task holds a reference
    // to the host object via ImportEventHandler.
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    /// Import.
    pub fn import(&self) {
        let _ = self.message_tx.send(ImportMessage::Import);
    }
}

struct Import {
    database: Arc<dyn ImportDatabasePort>,
    schema: Arc<dyn ImportSchemaPort>,
    author: AuthorId,

    files: Vec<EvaluatorFile>,
}

impl Import {
    fn init(
        database: Arc<dyn ImportDatabasePort>,
        schema: Arc<dyn ImportSchemaPort>,
        author: AuthorId,
    ) -> Result<Self, anyhow::Error> {
        Ok(Self {
            database,
            schema,
            author,

            files: Vec::new(),
        })
    }

    async fn run(
        &mut self,
        cancellation_token: CancellationToken,
        event_handler: Arc<dyn ImportEventHandler>,
        roots: Vec<PathBuf>,
        message_tx: mpsc::UnboundedSender<ImportMessage>,
        mut message_rx: mpsc::UnboundedReceiver<ImportMessage>,
    ) -> Result<(), anyhow::Error> {
        let (path_tx, path_rx) = std::sync::mpsc::channel();

        tokio::task::spawn_blocking({
            let cancellation_token = cancellation_token.clone();
            let event_handler = event_handler.clone();
            move || {
                dua_core::walk_roots(
                    roots.into_iter().enumerate(),
                    // TODO
                    4,
                    dua_core::Order::Completion,
                    move |_, _| {
                        // Stop descending if cancelled
                        !cancellation_token.is_cancelled()
                    },
                )
                .filter_map(|(_, entry)| {
                    let path = match entry {
                        dua_core::RootEvent::Entry(entry) => match entry {
                            Ok(entry) => {
                                if !entry.file_type.is_file() {
                                    return None;
                                }
                                entry.path()
                            }
                            Err(e) => {
                                // TODO: report error to UI
                                tracing::error!("Error while walking : {e:?}");
                                return None;
                            }
                        },
                        dua_core::RootEvent::Finished => return None,
                    };

                    // Skip if the file has no extension or the extension is not in Lofty's list of common audio extensions
                    if path
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_none_or(|extension| !lofty::file::EXTENSIONS.contains(&extension))
                    {
                        return None;
                    }

                    // TODO: batch?
                    event_handler.on_pending_file(path.to_string_lossy().to_string());

                    Some(path)
                })
                .for_each(|path| {
                    let _ = path_tx.send(path);
                });

                tracing::info!("Import walk finished");
            }
        });

        tokio::task::spawn_blocking({
            let cancellation_token = cancellation_token.clone();
            let message_tx = message_tx.clone();
            move || {
                path_rx.into_iter().par_bridge().for_each(|path| {
                    // Stop processing if cancelled
                    if cancellation_token.is_cancelled() {
                        return;
                    }

                    let file = match lofty::read_from_path(&path) {
                        Ok(file) => file,
                        Err(e) => {
                            // TODO: report error to UI
                            tracing::error!(?path, "Failed to read file: {e:#}");
                            return;
                        }
                    };

                    let tags = file.primary_tag().or_else(|| file.first_tag());

                    let event_tags = match tags {
                        Some(tag) => {
                            let mut event_tags = HashMap::new();

                            for item in tag.items() {
                                let key = format!("{:?}", item.key());
                                let value = item
                                    .value()
                                    // TODO
                                    .clone()
                                    .into_string()
                                    .unwrap_or_else(|| "<bytes>".to_string());

                                event_tags
                                    .entry(key)
                                    .and_modify(|existing: &mut String| {
                                        existing.push_str(", ");
                                        existing.push_str(&value);
                                    })
                                    .or_insert(value);
                            }

                            event_tags
                        }
                        None => {
                            tracing::debug!(?path, "File has no tags");
                            HashMap::new()
                        }
                    };

                    event_handler.on_scanned_file(ImportEventScannedFile {
                        path: path.to_string_lossy().to_string(),
                        tags: event_tags,
                    });

                    let _ = message_tx.send(ImportMessage::ScannedFile(EvaluatorFile {
                        path,
                        tags: tags.cloned(),
                    }));
                });

                tracing::info!("Import scan finished");

                event_handler.on_scan_finished();
            }
        });

        loop {
            tokio::select! {
                result = message_rx.recv() => {
                    match result {
                        Some(message) => {
                            if let Err(e) = self.handle_message(message).await {
                                tracing::error!("Import failed to handle message: {e:#}");
                            }
                        },
                        None => {
                            tracing::debug!("Import message rx closed");
                        },
                    }
                }

                _ = cancellation_token.cancelled() => {
                    tracing::debug!("Import task cancelled");
                    break;
                }
            }
        }

        Ok(())
    }

    async fn handle_message(&mut self, message: ImportMessage) -> Result<(), anyhow::Error> {
        match message {
            ImportMessage::Import => self.handle_import(),
            ImportMessage::ScannedFile(file) => {
                self.files.push(file);
                Ok(())
            }
        }
    }

    fn handle_import(&mut self) -> Result<(), anyhow::Error> {
        let Some((graph, rules)) = self.schema.watch_schema().borrow().clone() else {
            anyhow::bail!("Schema watcher is not initialized");
        };

        let changes = Evaluator::run(
            &rules,
            &self.database,
            todo!(),
            todo!(),
            self.author,
            &self.files,
        )?;
        self.database.apply_changes(changes, self.author)?;

        Ok(())
    }
}

enum ImportMessage {
    Import,

    ScannedFile(EvaluatorFile),
}
