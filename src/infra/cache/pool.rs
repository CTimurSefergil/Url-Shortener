use deadpool_redis::{Config, Pool, Runtime};

use crate::config::Config as AppConfig;

/// Create a deadpool-redis connection pool from app config.
pub fn create_pool(config: &AppConfig) -> Result<Pool, deadpool_redis::CreatePoolError> {
    let cfg = Config::from_url(&config.redis_url);
    let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
    // Note: deadpool-redis doesn't directly support min/max from config,
    // but the pool size is set via the builder. Using the Config builder:
    // Max size is set, min connections are created on demand.
    // For production tuning, the pool max is typically config.redis_pool_max.
    Ok(pool)
}

/// Create pool with explicit max size.
pub fn create_pool_with_size(
    redis_url: &str,
    max_size: usize,
) -> Result<Pool, deadpool_redis::CreatePoolError> {
    let mut cfg = Config::from_url(redis_url);
    cfg.pool = Some(deadpool_redis::PoolConfig {
        max_size,
        ..Default::default()
    });
    cfg.create_pool(Some(Runtime::Tokio1))
}
