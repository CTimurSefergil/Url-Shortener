use deadpool_redis::{Pool, redis::AsyncCommands};

use crate::models::CachedUrl;

use super::circuit_breaker::CircuitBreaker;

type BoxErr = Box<dyn std::error::Error + Send + Sync>;

const KEY_PREFIX: &str = "url:";

/// GET url:{code} — returns None on miss or error.
pub async fn get_url(pool: &Pool, cb: &CircuitBreaker, short_code: &str) -> Option<CachedUrl> {
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
            tracing::warn!(error = %e, "redis GET error");
            cb.record_failure();
            None
        }
    }
}

/// SET url:{code} — fire-and-forget, errors logged but swallowed.
pub async fn set_url(pool: &Pool, cb: &CircuitBreaker, short_code: &str, entry: &CachedUrl) {
    if !cb.allow_request() {
        return;
    }
    let key = format!("{KEY_PREFIX}{short_code}");
    let json = match serde_json::to_string(entry) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!(error = %e, "cache serialize error");
            return;
        }
    };

    let result: Result<(), BoxErr> = async {
        let mut conn = pool.get().await?;
        let now = chrono::Utc::now().timestamp();
        if entry.expires_at > 0 {
            let ttl = entry.expires_at - now;
            if ttl > 0 {
                conn.set_ex::<_, _, ()>(&key, &json, ttl as u64).await?;
            }
        } else {
            conn.set::<_, _, ()>(&key, &json).await?;
        }
        Ok(())
    }
    .await;

    match result {
        Ok(()) => cb.record_success(),
        Err(e) => {
            tracing::warn!(error = %e, "redis SET error");
            cb.record_failure();
        }
    }
}

/// DEL url:{code} — fire-and-forget.
pub async fn del_url(pool: &Pool, cb: &CircuitBreaker, short_code: &str) {
    if !cb.allow_request() {
        return;
    }
    let key = format!("{KEY_PREFIX}{short_code}");
    let result: Result<(), BoxErr> = async {
        let mut conn = pool.get().await?;
        conn.del::<_, ()>(&key).await?;
        Ok(())
    }
    .await;

    match result {
        Ok(()) => cb.record_success(),
        Err(e) => {
            tracing::warn!(error = %e, "redis DEL error");
            cb.record_failure();
        }
    }
}
