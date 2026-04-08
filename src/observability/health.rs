use actix_web::{HttpResponse, web};
use deadpool_redis::Pool;
use scylla::client::session::Session;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Instant;

use crate::models::{ComponentHealth, HealthChecks, HealthResponse};

/// Shared health check state.
pub struct HealthState {
    pub redis_pool: Pool,
    pub pg_pool: PgPool,
    pub cassandra: Arc<Session>,
}

/// GET /health — composite check for Redis, Cassandra, PostgreSQL.
pub async fn health_check(state: web::Data<HealthState>) -> HttpResponse {
    let (redis, cassandra, postgres) = futures_util::join!(
        check_redis(&state.redis_pool),
        check_cassandra(&state.cassandra),
        check_postgres(&state.pg_pool),
    );

    let all_up = redis.status == "up" && cassandra.status == "up" && postgres.status == "up";
    let status = if all_up { "healthy" } else { "degraded" };

    let code = if all_up {
        actix_web::http::StatusCode::OK
    } else {
        actix_web::http::StatusCode::SERVICE_UNAVAILABLE
    };

    let response = HealthResponse {
        status: status.to_string(),
        checks: HealthChecks {
            redis,
            cassandra,
            postgres,
        },
    };

    HttpResponse::build(code).json(response)
}

async fn check_redis(pool: &Pool) -> ComponentHealth {
    let start = Instant::now();
    let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
        let mut conn = pool.get().await?;
        let _: String = deadpool_redis::redis::cmd("PING")
            .query_async(&mut *conn)
            .await?;
        Ok(())
    }
    .await;
    let latency_ms = start.elapsed().as_millis() as u64;
    match result {
        Ok(()) => ComponentHealth {
            status: "up".to_string(),
            latency_ms,
        },
        Err(e) => {
            tracing::warn!(error = %e, "health: redis down");
            ComponentHealth {
                status: "down".to_string(),
                latency_ms,
            }
        }
    }
}

async fn check_cassandra(session: &Session) -> ComponentHealth {
    let start = Instant::now();
    let result = session
        .query_unpaged("SELECT now() FROM system.local", &())
        .await;
    let latency_ms = start.elapsed().as_millis() as u64;
    match result {
        Ok(_) => ComponentHealth {
            status: "up".to_string(),
            latency_ms,
        },
        Err(e) => {
            tracing::warn!(error = %e, "health: cassandra down");
            ComponentHealth {
                status: "down".to_string(),
                latency_ms,
            }
        }
    }
}

async fn check_postgres(pool: &PgPool) -> ComponentHealth {
    let start = Instant::now();
    let result: Result<(i32,), _> = sqlx::query_as("SELECT 1").fetch_one(pool).await;
    let latency_ms = start.elapsed().as_millis() as u64;
    match result {
        Ok(_) => ComponentHealth {
            status: "up".to_string(),
            latency_ms,
        },
        Err(e) => {
            tracing::warn!(error = %e, "health: postgres down");
            ComponentHealth {
                status: "down".to_string(),
                latency_ms,
            }
        }
    }
}
