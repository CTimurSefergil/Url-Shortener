use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize tracing with env filter.
/// Set RUST_LOG=info (default) or RUST_LOG=debug for more detail.
pub fn init() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(fmt::layer())
        .init();
}
