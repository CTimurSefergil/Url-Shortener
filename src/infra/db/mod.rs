pub mod cassandra;
pub mod postgres;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::models::ShortenedUrl;
use crate::errors::AppError;

/// Write-path repository (PostgreSQL — source of truth).
#[async_trait]
pub trait UrlWriteRepository: Send + Sync {
    async fn insert(&self, url: &ShortenedUrl) -> Result<(), AppError>;
    async fn increment_clicks(&self, short_code: &str) -> Result<(), AppError>;
    async fn cleanup_expired(&self) -> Result<u64, AppError>;
}

/// Read-path repository (Cassandra — fast reads).
#[async_trait]
pub trait UrlReadRepository: Send + Sync {
    async fn get_url(&self, short_code: &str) -> Result<Option<CassandraUrl>, AppError>;
    async fn get_stats(&self, short_code: &str) -> Result<Option<CassandraStats>, AppError>;
    async fn insert_url(&self, url: &CassandraUrl, ttl_secs: Option<i32>) -> Result<(), AppError>;
    async fn increment_click(&self, short_code: &str) -> Result<(), AppError>;
    async fn update_last_clicked(
        &self,
        short_code: &str,
        at: DateTime<Utc>,
    ) -> Result<(), AppError>;
}

/// Row from Cassandra `urls` table.
#[derive(Debug, Clone)]
pub struct CassandraUrl {
    pub short_code: String,
    pub original_url: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Aggregated stats from Cassandra (3 tables joined in application).
#[derive(Debug, Clone)]
pub struct CassandraStats {
    pub short_code: String,
    pub original_url: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub click_count: i64,
    pub last_clicked_at: Option<DateTime<Utc>>,
}
