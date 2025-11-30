use std::path::PathBuf;

use directories::ProjectDirs;
use tracing::subscriber::set_global_default;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_log::LogTracer;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, Registry};

pub fn init_subscriber(name: &str, env_filter: &str) -> WorkerGuard {
    LogTracer::init().expect("failed to initialize log tracer bridge");

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));

    let formatting_layer = fmt::layer().with_target(false).pretty();

    let log_dir = ProjectDirs::from("com", "gaveloc", "gaveloc")
        .map(|d| d.data_local_dir().join("logs"))
        .unwrap_or_else(|| PathBuf::from("logs"));

    let file_appender = tracing_appender::rolling::daily(log_dir, format!("{}.log", name));
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = fmt::layer().with_ansi(false).with_writer(non_blocking);

    let subscriber = Registry::default()
        .with(env_filter)
        .with(formatting_layer)
        .with(file_layer);

    set_global_default(subscriber).expect("failed to set global tracing subscriber");

    guard
}

#[cfg(test)]
mod tests {
    use super::*;

    // This test must be run with --test-threads=1 to prevent conflicts with other tests
    // that might also try to initialize a global subscriber.
    #[test]
    fn test_init_subscriber() {
        // We only expect this to not panic and return a guard.
        // Further verification would require more complex integration tests (e.g., checking log files).
        let _guard = init_subscriber("test_app", "info");
        // The guard ensures the appender flushes logs when it goes out of scope.
        // In a real application, this guard would be held for the lifetime of the app.
    }
}
