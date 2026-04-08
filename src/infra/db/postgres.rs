use async_trait::async_trait;
use sqlx::PgPool;

use crate::models::ShortenedUrl;
use crate::errors::AppError;

use super::UrlWriteRepository;

pub struct PgRepository {
    pool: PgPool,
}

impl PgRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UrlWriteRepository for PgRepository {
    async fn insert(&self, url: &ShortenedUrl) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO shortened_urls (id, short_code, original_url, created_at, expires_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(url.id)
        .bind(&url.short_code)
        .bind(&url.original_url)
        .bind(url.created_at)
        .bind(url.expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn increment_clicks(&self, short_code: &str) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE shortened_urls
             SET click_count = click_count + 1, last_clicked_at = NOW()
             WHERE short_code = $1",
        )
        .bind(short_code)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn cleanup_expired(&self) -> Result<u64, AppError> {
        let result = sqlx::query("DELETE FROM shortened_urls WHERE expires_at < NOW()")
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}
