use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// --- Request / Response ---

#[derive(Debug, Deserialize)]
pub struct CreateUrlRequest {
    pub url: String,
    pub expires_in_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct CreateUrlResponse {
    pub short_url: String,
    pub short_code: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

// --- Domain entities ---

#[derive(Debug, Clone)]
pub struct ShortenedUrl {
    pub id: i64,
    pub short_code: String,
    pub original_url: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub click_count: i64,
    pub last_clicked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UrlStats {
    pub short_code: String,
    pub original_url: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub click_count: i64,
    pub last_clicked_at: Option<DateTime<Utc>>,
}

// --- Cache entries ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedUrl {
    pub original_url: String,
    pub expires_at: i64, // Unix timestamp
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedStats {
    pub short_code: String,
    pub original_url: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub click_count: i64,
    pub last_clicked_at: Option<i64>,
}

// --- Health ---

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub checks: HealthChecks,
}

#[derive(Debug, Serialize)]
pub struct HealthChecks {
    pub redis: ComponentHealth,
    pub cassandra: ComponentHealth,
    pub postgres: ComponentHealth,
}

#[derive(Debug, Serialize)]
pub struct ComponentHealth {
    pub status: String,
    pub latency_ms: u64,
}
