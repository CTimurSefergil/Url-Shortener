use actix_web::{App, HttpServer, web};
use std::sync::Arc;

use scylla::client::session_builder::SessionBuilder;
use stockimg::config::Config;
use stockimg::infra::db::UrlWriteRepository;
use stockimg::infra::cache::CacheManager;
use stockimg::infra::cache::id_alloc::RedisIdAllocator;
use stockimg::infra::cache::pool::create_pool_with_size;
use stockimg::infra::cache::rate_limit::RateLimiter;
use stockimg::infra::db::cassandra::CassandraRepository;
use stockimg::infra::db::postgres::PgRepository;
use stockimg::infra::queue::producer::QueueProducer;
use stockimg::observability::health::HealthState;
use stockimg::services::analytics_service::AnalyticsService;
use stockimg::services::url_service::UrlService;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 1. Tracing init
    stockimg::observability::tracing_setup::init();

    // 2. Config
    let config = Config::from_env().expect("failed to load config");
    tracing::info!(host = %config.host, port = %config.port, "starting server");

    // 3. PostgreSQL
    let pg_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.database_url)
        .await
        .expect("failed to connect to PostgreSQL");
    tracing::info!("PostgreSQL connected");

    sqlx::migrate!("./migrations")
        .run(&pg_pool)
        .await
        .expect("failed to run migrations");
    tracing::info!("migrations applied");

    // 4. Redis (verify connection)
    {
        let pool = create_pool_with_size(&config.redis_url, 1).expect("failed to create Redis pool");
        let mut conn = pool.get().await.expect("failed to connect to Redis");
        let _: String = deadpool_redis::redis::cmd("PING")
            .query_async(&mut *conn)
            .await
            .expect("Redis PING failed");
    }
    tracing::info!("Redis connected (pool max={})", config.redis_pool_max);

    // 5. Cassandra
    let cassandra_session = SessionBuilder::new()
        .known_nodes(&config.cassandra_nodes)
        .build()
        .await
        .expect("failed to connect to Cassandra");
    let cassandra_session = Arc::new(cassandra_session);
    tracing::info!("Cassandra connected");

    let cassandra_repo = CassandraRepository::new(cassandra_session.clone());
    cassandra_repo
        .ensure_schema()
        .await
        .expect("failed to ensure Cassandra schema");
    tracing::info!("Cassandra schema ready");

    // 6. RabbitMQ
    let rabbitmq_conn = lapin::Connection::connect(
        &config.rabbitmq_url,
        lapin::ConnectionProperties::default(),
    )
    .await
    .expect("failed to connect to RabbitMQ");
    tracing::info!("RabbitMQ connected");

    let producer_channel = rabbitmq_conn
        .create_channel()
        .await
        .expect("failed to create producer channel");
    let producer = QueueProducer::new(producer_channel);
    producer
        .declare_queue()
        .await
        .expect("failed to declare queue");

    let consumer_channel = rabbitmq_conn
        .create_channel()
        .await
        .expect("failed to create consumer channel");

    // 7. ID generator
    let id_allocator = Arc::new(RedisIdAllocator::new(
        create_pool_with_size(&config.redis_url, 2).expect("failed to create ID alloc pool"),
    ));
    let id_gen = Arc::new(stockimg::id_generator::IdGenerator::new(
        id_allocator,
        config.id_batch_size,
        config.feistel_master_key,
    ));

    // 8. Prometheus metrics
    let prometheus_registry = prometheus::Registry::new();
    let _app_metrics = stockimg::observability::metrics::AppMetrics::new(&prometheus_registry)
        .expect("failed to create metrics");
    let prom_middleware =
        actix_web_prom::PrometheusMetricsBuilder::new("api")
            .registry(prometheus_registry)
            .endpoint("/metrics")
            .build()
            .expect("failed to build prometheus middleware");

    // 9. Service layer (DI wiring)
    let pg_repo: Arc<dyn stockimg::infra::db::UrlWriteRepository> =
        Arc::new(PgRepository::new(pg_pool.clone()));
    let cassandra_read: Arc<dyn stockimg::infra::db::UrlReadRepository> =
        Arc::new(CassandraRepository::new(cassandra_session.clone()));

    let cache_manager: Arc<dyn stockimg::infra::cache::UrlCacheOps> = Arc::new(
        CacheManager::new(
            create_pool_with_size(&config.redis_url, config.redis_pool_max)
                .expect("failed to create cache pool"),
            config.redis_cb_failure_threshold,
            config.redis_cb_open_duration_secs,
        ),
    );

    let analytics = Arc::new(AnalyticsService::new(
        pg_repo.clone(),
        cassandra_read.clone(),
    ));

    let url_service = web::Data::new(UrlService::new(
        pg_repo,
        cassandra_read.clone(),
        cache_manager,
        id_gen,
        Arc::new(producer),
        analytics,
        config.clone(),
    ));

    // Rate limiter
    let rate_limiter = Arc::new(RateLimiter::new(
        create_pool_with_size(&config.redis_url, 4).expect("failed to create rate limiter pool"),
        config.rate_limit_max_requests,
        config.rate_limit_window_secs,
    ));

    // Health check state
    let health_state = web::Data::new(HealthState {
        redis_pool: create_pool_with_size(&config.redis_url, 2)
            .expect("failed to create health check pool"),
        pg_pool: pg_pool.clone(),
        cassandra: cassandra_session.clone(),
    });

    // 10. Background: RabbitMQ consumer
    stockimg::infra::queue::consumer::spawn_consumer(consumer_channel, cassandra_read)
        .await
        .expect("failed to spawn consumer");
    tracing::info!("RabbitMQ consumer started");

    // 11. Background: expired URL cleanup (1 hour interval)
    {
        let pg_cleanup = PgRepository::new(pg_pool.clone());
        actix_web::rt::spawn(async move {
            let mut interval = actix_web::rt::time::interval(std::time::Duration::from_secs(3600));
            loop {
                interval.tick().await;
                match pg_cleanup.cleanup_expired().await {
                    Ok(count) => {
                        if count > 0 {
                            tracing::info!(count, "cleaned up expired URLs");
                        }
                    }
                    Err(e) => tracing::error!(error = %e, "expired cleanup failed"),
                }
            }
        });
    }

    // 12. Start server
    let bind_addr = format!("{}:{}", config.host, config.port);
    tracing::info!("server listening on {}", bind_addr);

    HttpServer::new(move || {
        App::new()
            .wrap(prom_middleware.clone())
            .wrap(tracing_actix_web::TracingLogger::default())
            .app_data(url_service.clone())
            .app_data(health_state.clone())
            .route("/health", web::get().to(stockimg::observability::health::health_check))
            .configure(stockimg::api::configure_routes_with_limiter(
                rate_limiter.clone(),
            ))
    })
    .bind(&bind_addr)?
    .run()
    .await
}
