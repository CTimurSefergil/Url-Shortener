pub mod circuit_breaker;
pub mod id_alloc;
pub mod pool;
pub mod rate_limit;
pub mod stats_cache;
pub mod url_cache;

use async_trait::async_trait;
use deadpool_redis::Pool;
use std::sync::Arc;

use crate::models::{CachedStats, CachedUrl};

use self::circuit_breaker::CircuitBreaker;

/// Cache operations for URL lookups.
#[async_trait]
pub trait UrlCacheOps: Send + Sync {
    async fn get_url(&self, short_code: &str) -> Option<CachedUrl>;
    async fn set_url(&self, short_code: &str, entry: &CachedUrl);
    async fn del_url(&self, short_code: &str);
    async fn get_stats(&self, short_code: &str) -> Option<CachedStats>;
    async fn set_stats(&self, short_code: &str, stats: &CachedStats, ttl_secs: u64);
}

/// Manages Redis pool + circuit breaker. Implements UrlCacheOps.
pub struct CacheManager {
    pool: Pool,
    cb: Arc<CircuitBreaker>,
}

impl CacheManager {
    pub fn new(pool: Pool, failure_threshold: u32, open_duration_secs: u64) -> Self {
        Self {
            pool,
            cb: Arc::new(CircuitBreaker::new(failure_threshold, open_duration_secs)),
        }
    }

    pub fn pool(&self) -> &Pool {
        &self.pool
    }

    pub fn circuit_breaker(&self) -> &CircuitBreaker {
        &self.cb
    }
}

#[async_trait]
impl UrlCacheOps for CacheManager {
    async fn get_url(&self, short_code: &str) -> Option<CachedUrl> {
        url_cache::get_url(&self.pool, &self.cb, short_code).await
    }

    async fn set_url(&self, short_code: &str, entry: &CachedUrl) {
        url_cache::set_url(&self.pool, &self.cb, short_code, entry).await;
    }

    async fn del_url(&self, short_code: &str) {
        url_cache::del_url(&self.pool, &self.cb, short_code).await;
    }

    async fn get_stats(&self, short_code: &str) -> Option<CachedStats> {
        stats_cache::get_stats(&self.pool, &self.cb, short_code).await
    }

    async fn set_stats(&self, short_code: &str, stats: &CachedStats, ttl_secs: u64) {
        stats_cache::set_stats(&self.pool, &self.cb, short_code, stats, ttl_secs).await;
    }
}
