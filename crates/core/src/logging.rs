//! Logging setup. Call `init` once at startup; respects `RUST_LOG`.

/// Initialize env_logger. Safe to call more than once; later calls are no-ops.
pub fn init() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();
}
