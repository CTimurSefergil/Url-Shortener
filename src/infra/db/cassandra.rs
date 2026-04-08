use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use scylla::client::session::Session;
use scylla::value::{Counter, CqlTimestamp};
use std::sync::Arc;

use crate::errors::AppError;

use super::{CassandraStats, CassandraUrl, UrlReadRepository};

pub struct CassandraRepository {
    session: Arc<Session>,
}

impl CassandraRepository {
    pub fn new(session: Arc<Session>) -> Self {
        Self { session }
    }

    /// Ensure keyspace and tables exist.
    pub async fn ensure_schema(&self) -> Result<(), AppError> {
        let queries = [
            "CREATE KEYSPACE IF NOT EXISTS urlshortener
             WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 3}",
            "CREATE TABLE IF NOT EXISTS urlshortener.urls (
                 short_code TEXT PRIMARY KEY,
                 original_url TEXT,
                 created_at TIMESTAMP,
                 expires_at TIMESTAMP
             )",
            "CREATE TABLE IF NOT EXISTS urlshortener.url_clicks (
                 short_code TEXT PRIMARY KEY,
                 click_count COUNTER
             )",
            "CREATE TABLE IF NOT EXISTS urlshortener.url_last_clicked (
                 short_code TEXT PRIMARY KEY,
                 last_clicked_at TIMESTAMP
             )",
        ];

        for q in queries {
            self.session.query_unpaged(q, &()).await.map_err(|e| {
                tracing::error!(error = %e, "cassandra schema error");
                AppError::Internal("cassandra schema setup failed".to_string())
            })?;
        }
        Ok(())
    }
}

#[async_trait]
impl UrlReadRepository for CassandraRepository {
    async fn get_url(&self, short_code: &str) -> Result<Option<CassandraUrl>, AppError> {
        let result = self
            .session
            .query_unpaged(
                "SELECT short_code, original_url, created_at, expires_at
                 FROM urlshortener.urls WHERE short_code = ?",
                &(short_code.to_string(),),
            )
            .await
            .map_err(map_scylla_err)?;

        let rows = result.into_rows_result().map_err(map_scylla_err)?;

        let Some(first_row) =
            rows.maybe_first_row::<(String, String, CqlTimestamp, CqlTimestamp)>()
                .map_err(map_scylla_err)?
        else {
            return Ok(None);
        };

        let (sc, original_url, created_at, expires_at) = first_row;

        Ok(Some(CassandraUrl {
            short_code: sc,
            original_url,
            created_at: cql_ts_to_chrono(created_at),
            expires_at: cql_ts_to_chrono(expires_at),
        }))
    }

    async fn get_stats(&self, short_code: &str) -> Result<Option<CassandraStats>, AppError> {
        let url = self.get_url(short_code).await?;
        let Some(url) = url else {
            return Ok(None);
        };

        // Run click_count and last_clicked queries concurrently
        let click_params = (short_code.to_string(),);
        let last_clicked_params = (short_code.to_string(),);
        let click_fut = self.session.query_unpaged(
            "SELECT click_count FROM urlshortener.url_clicks WHERE short_code = ?",
            &click_params,
        );
        let last_clicked_fut = self.session.query_unpaged(
            "SELECT last_clicked_at FROM urlshortener.url_last_clicked WHERE short_code = ?",
            &last_clicked_params,
        );

        let (click_result, last_clicked_result) =
            futures_util::join!(click_fut, last_clicked_fut);

        let click_count = click_result
            .map_err(map_scylla_err)?
            .into_rows_result()
            .ok()
            .and_then(|rows| rows.maybe_first_row::<(Counter,)>().ok().flatten())
            .map(|(c,)| c.0)
            .unwrap_or(0);

        let last_clicked_at = last_clicked_result
            .map_err(map_scylla_err)?
            .into_rows_result()
            .ok()
            .and_then(|rows| rows.maybe_first_row::<(CqlTimestamp,)>().ok().flatten())
            .map(|(ts,)| cql_ts_to_chrono(ts));

        Ok(Some(CassandraStats {
            short_code: url.short_code,
            original_url: url.original_url,
            created_at: url.created_at,
            expires_at: url.expires_at,
            click_count,
            last_clicked_at,
        }))
    }

    async fn insert_url(&self, url: &CassandraUrl, ttl_secs: Option<i32>) -> Result<(), AppError> {
        let values = (
            url.short_code.clone(),
            url.original_url.clone(),
            chrono_to_cql_ts(url.created_at),
            chrono_to_cql_ts(url.expires_at),
        );

        if let Some(ttl) = ttl_secs {
            let query = format!(
                "INSERT INTO urlshortener.urls (short_code, original_url, created_at, expires_at)
                 VALUES (?, ?, ?, ?) USING TTL {}",
                ttl
            );
            self.session
                .query_unpaged(query.as_str(), &values)
                .await
                .map_err(map_scylla_err)?;
        } else {
            self.session
                .query_unpaged(
                    "INSERT INTO urlshortener.urls (short_code, original_url, created_at, expires_at)
                     VALUES (?, ?, ?, ?)",
                    &values,
                )
                .await
                .map_err(map_scylla_err)?;
        }
        Ok(())
    }

    async fn increment_click(&self, short_code: &str) -> Result<(), AppError> {
        self.session
            .query_unpaged(
                "UPDATE urlshortener.url_clicks SET click_count = click_count + 1
                 WHERE short_code = ?",
                &(short_code.to_string(),),
            )
            .await
            .map_err(map_scylla_err)?;
        Ok(())
    }

    async fn update_last_clicked(
        &self,
        short_code: &str,
        at: DateTime<Utc>,
    ) -> Result<(), AppError> {
        self.session
            .query_unpaged(
                "INSERT INTO urlshortener.url_last_clicked (short_code, last_clicked_at)
                 VALUES (?, ?)",
                &(short_code.to_string(), chrono_to_cql_ts(at)),
            )
            .await
            .map_err(map_scylla_err)?;
        Ok(())
    }
}

fn cql_ts_to_chrono(ts: CqlTimestamp) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ts.0).single().unwrap_or_default()
}

fn chrono_to_cql_ts(dt: DateTime<Utc>) -> CqlTimestamp {
    CqlTimestamp(dt.timestamp_millis())
}

fn map_scylla_err(e: impl std::fmt::Display) -> AppError {
    tracing::error!(error = %e, "cassandra error");
    AppError::Internal(format!("cassandra error: {e}"))
}
