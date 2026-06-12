//! Logging setup. Call [`init_logging`] once at startup; respects `RUST_LOG`.

/// Initializes env_logger with a default filter of `info`.
///
/// The `RUST_LOG` environment variable overrides the default as usual.
/// Safe to call more than once; later calls are no-ops.
pub fn init_logging() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();
}
