use std::{iter, path::Path};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    EnvFilter, Registry,
    fmt::{self, format::FmtSpan},
    prelude::*,
};

/// Initialize logging.
///
/// If `log_dir` is provided, logs will be written to files, rotated at 5 MB per file with 5 files
/// retained and gzip compression. A guard will be returned that should be kept alive until the
/// application exits to flush logs on drop.
///
/// On Android and iOS, logs will also be forwarded to the platform logging systems. On other
/// platforms, logs will also be written to stderr.
pub fn init(log_dir: Option<&Path>) -> anyhow::Result<Option<LogGuard>> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_env_filter()));

    let guard = if let Some(log_dir) = log_dir {
        if let Err(e) = std::fs::create_dir_all(log_dir) {
            anyhow::bail!(
                "Failed to create log directory at {}: {e:#}",
                log_dir.display()
            );
        }

        let roller = logroller::LogRollerBuilder::new(log_dir, Path::new("stellar.log"))
            .rotation(logroller::Rotation::SizeBased(logroller::RotationSize::MB(
                5,
            )))
            .max_keep_files(5)
            .compression(logroller::Compression::Gzip)
            .build()?;
        let (non_blocking, guard) = tracing_appender::non_blocking(roller);
        let roller_layer = fmt::Layer::new()
            .with_writer(non_blocking)
            .with_span_events(FmtSpan::CLOSE)
            .with_ansi(false);

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let subscriber = Registry::default().with(filter).with(roller_layer).with(
                fmt::Layer::new()
                    .with_writer(std::io::stderr)
                    .with_span_events(FmtSpan::CLOSE),
            );
            let _ = tracing::subscriber::set_global_default(subscriber);
        }

        #[cfg(target_os = "android")]
        {
            let subscriber = Registry::default().with(filter).with(roller_layer).with(
                tracing_android::layer("stellar").expect("Failed to init android tracing layer"),
            );
            let _ = tracing::subscriber::set_global_default(subscriber);
        }

        #[cfg(target_os = "ios")]
        {
            let subscriber = Registry::default().with(filter).with(roller_layer).with(
                tracing_oslog::OsLogger::new("net.trillia.stellar", "default"),
            );
            let _ = tracing::subscriber::set_global_default(subscriber);
        }

        Some(LogGuard { _guard: guard })
    } else {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let subscriber = Registry::default().with(filter).with(
                fmt::Layer::new()
                    .with_writer(std::io::stderr)
                    .with_span_events(FmtSpan::CLOSE),
            );
            let _ = tracing::subscriber::set_global_default(subscriber);
        }

        #[cfg(target_os = "android")]
        {
            let subscriber = Registry::default().with(filter).with(
                tracing_android::layer("stellar").expect("Failed to init android tracing layer"),
            );
            let _ = tracing::subscriber::set_global_default(subscriber);
        }

        #[cfg(target_os = "ios")]
        {
            let subscriber = Registry::default()
                .with(filter)
                .with(tracing_oslog::OsLogger::new(
                    "net.trillia.stellar",
                    "default",
                ));
            let _ = tracing::subscriber::set_global_default(subscriber);
        }

        None
    };

    // Forward `log` records to `tracing`
    let _ = tracing_log::LogTracer::init();

    // Log on panic
    log_panics::init();

    Ok(guard)
}

/// Guard that should be kept alive until the application exits to flush logs on drop.
pub struct LogGuard {
    _guard: WorkerGuard,
}

/// Default filter:
/// - `warn` by default
/// - `stellar_*=debug`: include debug logs from us
/// - `iroh=error`: suppress noisy warnings from Iroh
fn default_env_filter() -> String {
    let stellar_crate_filters = STELLAR_CRATES
        .iter()
        .map(|c| format!("{}=debug", c))
        .collect::<Vec<_>>();

    iter::once("warn")
        .chain(stellar_crate_filters.iter().map(|f| f.as_str()))
        .chain(["iroh=error"])
        .collect::<Vec<_>>()
        .join(",")
}

pub const STELLAR_CRATES: &[&str] = &[
    "stellar",
    "stellar_graph",
    "stellar_import",
    "stellar_log",
    "stellar_resources",
    "stellar_riblt",
    "stellar_sync",
    "stellar_tui",
    "stellar_uniffi",
];

#[cfg(test)]
mod test {
    use crate::STELLAR_CRATES;
    use std::path::PathBuf;

    #[test]
    fn matches_workspace_crates() {
        let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let crates_dir = crate_dir.join("..");

        let mut crates = std::fs::read_dir(crates_dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_dir()))
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .map(|dir_name| dir_name.replace("-", "_"))
            .collect::<Vec<_>>();
        crates.sort();

        assert_eq!(
            STELLAR_CRATES, crates,
            "STELLAR_CRATES should contain all workspace crates"
        )
    }
}
