use lofty::file::TaggedFileExt;
use rayon::iter::{ParallelBridge, ParallelIterator};
use std::{path::PathBuf, sync::Arc};
use tokio_util::sync::CancellationToken;

/// Foreign trait for receiving import events.
#[uniffi::export(with_foreign)]
pub trait ImportEventHandler: Send + Sync {
    fn on_pending_file(&self, path: String);

    fn on_scanned_file(&self);
}

/// Handle to an import task.
#[derive(Debug, Clone)]
pub struct ImportTask {
    cancellation_token: CancellationToken,
}

impl ImportTask {
    pub fn spawn(
        cancellation_token: CancellationToken,
        event_handler: Arc<dyn ImportEventHandler>,
        roots: Vec<PathBuf>,
    ) -> Result<Self, anyhow::Error> {
        tokio::spawn({
            let cancellation_token = cancellation_token.clone();
            async move {
                let mut import = match Import::init() {
                    Ok(import) => import,
                    Err(e) => {
                        tracing::error!("Import task failed to init: {e:?}");
                        return;
                    }
                };

                let result = import.run(cancellation_token, event_handler, roots).await;

                if let Err(error) = result {
                    tracing::error!("Import task errored: {error:?}");
                } else {
                    tracing::debug!("Import task finished");
                }
            }
        });

        Ok(Self { cancellation_token })
    }

    // Cancel the import task. This must be called when the UI is done with the import,
    // or the import task will leak.
    //
    // We can't really do this automatically on drop because the import task holds a reference
    // to the host object via ImportEventHandler.
    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }
}

struct Import {
    files: Vec<ImportScanFile>,
}

impl Import {
    fn init() -> Result<Self, anyhow::Error> {
        Ok(Self { files: Vec::new() })
    }

    async fn run(
        &mut self,
        cancellation_token: CancellationToken,
        event_handler: Arc<dyn ImportEventHandler>,
        roots: Vec<PathBuf>,
    ) -> Result<(), anyhow::Error> {
        tokio::task::spawn_blocking(move || {
            dua_core::walk_roots(
                roots.into_iter().enumerate(),
                // TODO
                4,
                dua_core::Order::Completion,
                |_, _| true,
            )
            .inspect(|(_, entry)| {
                let path = match entry {
                    dua_core::RootEvent::Entry(Ok(entry)) => {
                        if !entry.file_type.is_file() {
                            return;
                        }
                        entry.path().to_string_lossy().to_string()
                    }
                    _ => return,
                };
                // TODO: batch?
                event_handler.on_pending_file(path);
            })
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

                Some(path)
            })
            .par_bridge()
            .for_each(|path| {
                let file = match lofty::read_from_path(&path) {
                    Ok(file) => file,
                    Err(e) => {
                        // TODO: report error to UI
                        tracing::error!(?path, "Failed to read file: {e:#}");
                        return;
                    }
                };

                let Some(tag) = file.primary_tag().or_else(|| file.first_tag()) else {
                    // TODO: report error to UI
                    tracing::error!(?path, "Failed to get file tags");
                    return;
                };

                println!(
                    "{:?} {} {:?}",
                    rayon::current_thread_index(),
                    path.display(),
                    tag.items().map(|item| item.key()).collect::<Vec<_>>()
                );
            });
        });

        loop {
            tokio::select! {
                _ = cancellation_token.cancelled() => {
                    tracing::debug!("Import task cancelled");
                    break;
                }
            }
        }

        Ok(())
    }
}

struct ImportScanFile {
    path: PathBuf,
}
