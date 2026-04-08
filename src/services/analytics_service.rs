use chrono::Utc;
use std::sync::Arc;

use crate::infra::db::{UrlReadRepository, UrlWriteRepository};

/// Fire-and-forget click tracking.
/// Updates Cassandra counter + last_clicked + PG increment.
pub struct AnalyticsService {
    pg: Arc<dyn UrlWriteRepository>,
    cassandra: Arc<dyn UrlReadRepository>,
}

impl AnalyticsService {
    pub fn new(pg: Arc<dyn UrlWriteRepository>, cassandra: Arc<dyn UrlReadRepository>) -> Self {
        Self { pg, cassandra }
    }

    /// Record a click event. Errors are logged but never propagated.
    pub async fn record_click(&self, short_code: &str) {
        let now = Utc::now();

        // Cassandra: increment counter + update last_clicked (parallel)
        let cassandra = self.cassandra.clone();
        let code1 = short_code.to_string();
        let code2 = short_code.to_string();

        let (click_res, ts_res) = futures_util::join!(
            async { cassandra.increment_click(&code1).await },
            async { cassandra.update_last_clicked(&code2, now).await },
        );

        if let Err(e) = click_res {
            tracing::warn!(error = %e, code = %short_code, "cassandra increment_click failed");
        }
        if let Err(e) = ts_res {
            tracing::warn!(error = %e, code = %short_code, "cassandra update_last_clicked failed");
        }

        // PostgreSQL: increment click_count
        if let Err(e) = self.pg.increment_clicks(short_code).await {
            tracing::warn!(error = %e, code = %short_code, "pg increment_clicks failed");
        }
    }
}
