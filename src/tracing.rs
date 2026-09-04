use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    filter::LevelFilter, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

/// reads the filter from `LOG_LEVEL`, defaulting to `info` so that an unset variable
/// doesn't silence everything, error responses included
pub fn setup_tracing(log_path: PathBuf) -> WorkerGuard {
    let log_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .with_env_var("LOG_LEVEL")
        .from_env_lossy();
    #[allow(unused_variables)]
    let error_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::ERROR.into())
        .with_env_var("ERROR_LEVEL")
        .from_env_lossy();

    // normal logging
    let t = tracing_subscriber::registry().with(log_filter);

    // file
    let file_appender = tracing_appender::rolling::daily(&log_path, "app.log");
    #[allow(unused_variables)]
    let (non_blocking, guard1) = tracing_appender::non_blocking(file_appender);
    #[cfg(not(debug_assertions))]
    let t = t.with(tracing_subscriber::fmt::layer().with_writer(non_blocking));
    // stdout
    #[cfg(debug_assertions)]
    let t = t.with(
        tracing_subscriber::fmt::layer().with_writer(std::io::stdout), // .with_filter(log_filter),
    );

    // // errors
    // let file_appender = tracing_appender::rolling::daily(&log_path, "app-error.log");
    // #[allow(unused_variables)]
    // let (non_blocking, guard2) = tracing_appender::non_blocking(file_appender);
    // #[cfg(not(debug_assertions))]
    // let t = t.with(
    //     tracing_subscriber::fmt::layer()
    //         .with_writer(non_blocking)
    //         .with_filter(error_filter),
    // );

    t.init();

    guard1
}
