use deadpool_redis::{Pool, redis::AsyncCommands};

use crate::models::CachedStats;

use super::circuit_breaker::CircuitBreaker;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

const KEY_PREFIX: &str = "stats:";

/// GET stats:{code} — returns None on miss or error.
pub async fn get_stats(pool: &Pool, cb: &CircuitBreaker, short_code: &str) -> Option<CachedStats> {
    if !cb.allow_request() {
        return None;
    }
    let key = format!("{KEY_PREFIX}{short_code}");
    let result: Result<Option<String>, BoxErr> = async {
        let mut conn = pool.get().await?;
        let val: Option<String> = conn.get(&key).await?;
        Ok(val)
    }
    .await;

    match result {
        Ok(Some(json)) => {
            cb.record_success();
            serde_json::from_str(&json).ok()
        }
        Ok(None) => {
            cb.record_success();
            None
        }
        Err(e) => {
            tracing::warn!(error = %e, "redis GET stats error");
            cb.record_failure();
            None
        }
    }
}

/// SET stats:{code} EX {ttl} — fire-and-forget.
pub async fn set_stats(
    pool: &Pool,
    cb: &CircuitBreaker,
    short_code: &str,
    stats: &CachedStats,
    ttl_secs: u64,
) {
    if !cb.allow_request() {
        return;
    }
    let key = format!("{KEY_PREFIX}{short_code}");
    let json = match serde_json::to_string(stats) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!(error = %e, "cache stats serialize error");
            return;
        }
    };

    let result: Result<(), BoxErr> = async {
        let mut conn = pool.get().await?;
        conn.set_ex::<_, _, ()>(&key, &json, ttl_secs).await?;
        Ok(())
    }
    .await;

    match result {
        Ok(()) => cb.record_success(),
        Err(e) => {
            tracing::warn!(error = %e, "redis SET stats error");
            cb.record_failure();
        }
    }
}
