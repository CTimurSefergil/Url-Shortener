use async_trait::async_trait;
use deadpool_redis::{Pool, redis::AsyncCommands};

use crate::id_generator::{IdAllocator, IdGenError};

const KEY: &str = "url_shortener:next_id";

/// Redis-backed ID allocator using INCRBY for batch allocation.
pub struct RedisIdAllocator {
    pool: Pool,
}

impl RedisIdAllocator {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl IdAllocator for RedisIdAllocator {
    async fn allocate_batch(&self, batch_size: u64) -> Result<(u64, u64), IdGenError> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| IdGenError::Alloc(e.to_string()))?;

        let end: u64 = conn
            .incr(KEY, batch_size)
            .await
            .map_err(|e| IdGenError::Alloc(e.to_string()))?;

        let start = end - batch_size;
        Ok((start, end))
    }
}
