use deadpool_redis::{Pool, redis};

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

/// Sliding window rate limiter using Redis sorted sets.
///
/// Key: `rate_limit:{ip}`
/// Pipeline: ZREMRANGEBYSCORE + ZADD + ZCARD + EXPIRE
pub struct RateLimiter {
    pool: Pool,
    max_requests: u64,
    window_secs: u64,
}

impl RateLimiter {
    pub fn new(pool: Pool, max_requests: u64, window_secs: u64) -> Self {
        Self {
            pool,
            max_requests,
            window_secs,
        }
    }

    /// Returns true if allowed, false if rate limited.
    /// Returns true on Redis errors (fail-open).
    pub async fn is_allowed(&self, ip: &str) -> bool {
        let key = format!("rate_limit:{ip}");
        let now = chrono::Utc::now().timestamp_millis() as f64;
        let window_start = now - (self.window_secs as f64 * 1000.0);

        let result: Result<((), (), u64, ()), BoxErr> = async {
            let mut conn = self.pool.get().await?;
            let res: ((), (), u64, ()) = redis::pipe()
                .zrembyscore(&key, f64::NEG_INFINITY, window_start)
                .zadd(&key, now, now.to_string())
                .zcard(&key)
                .expire(&key, self.window_secs as i64)
                .query_async(&mut *conn)
                .await?;
            Ok(res)
        }
        .await;

        match result {
            Ok(((), (), count, ())) => count <= self.max_requests,
            Err(e) => {
                tracing::warn!(error = %e, "rate limiter redis error");
                true // Fail-open
            }
        }
    }
}
