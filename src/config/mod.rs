use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    // Server
    pub host: String,
    pub port: u16,

    // PostgreSQL
    pub database_url: String,

    // Cassandra
    pub cassandra_nodes: Vec<String>,
    pub cassandra_keyspace: String,
    pub cassandra_query_timeout_ms: u64,

    // Redis
    pub redis_url: String,
    pub redis_pool_min: usize,
    pub redis_pool_max: usize,
    pub redis_pool_timeout_ms: u64,
    pub redis_connect_timeout_ms: u64,
    pub redis_command_timeout_ms: u64,

    // Circuit Breaker
    pub redis_cb_failure_threshold: u32,
    pub redis_cb_open_duration_secs: u64,

    // Cache
    pub stats_cache_ttl_secs: u64,

    // RabbitMQ
    pub rabbitmq_url: String,

    // Rate Limiting
    pub rate_limit_max_requests: u64,
    pub rate_limit_window_secs: u64,

    // ID Generation
    pub id_batch_size: u64,
    pub feistel_master_key: u64,

    // Base URL for short links
    pub base_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();

        Ok(Self {
            host: env_or("HOST", "0.0.0.0"),
            port: env_parse("PORT", 8080)?,

            database_url: env_required("DATABASE_URL")?,

            cassandra_nodes: env_or("CASSANDRA_NODES", "127.0.0.1:9042")
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
            cassandra_keyspace: env_or("CASSANDRA_KEYSPACE", "urlshortener"),
            cassandra_query_timeout_ms: env_parse("CASSANDRA_QUERY_TIMEOUT_MS", 1000)?,

            redis_url: env_or("REDIS_URL", "redis://127.0.0.1:6379"),
            redis_pool_min: env_parse("REDIS_POOL_MIN", num_cpus() * 2)?,
            redis_pool_max: env_parse("REDIS_POOL_MAX", num_cpus() * 8)?,
            redis_pool_timeout_ms: env_parse("REDIS_POOL_TIMEOUT_MS", 100)?,
            redis_connect_timeout_ms: env_parse("REDIS_CONNECT_TIMEOUT_MS", 500)?,
            redis_command_timeout_ms: env_parse("REDIS_COMMAND_TIMEOUT_MS", 200)?,

            redis_cb_failure_threshold: env_parse("REDIS_CB_FAILURE_THRESHOLD", 5)?,
            redis_cb_open_duration_secs: env_parse("REDIS_CB_OPEN_DURATION_SECS", 30)?,

            stats_cache_ttl_secs: env_parse("STATS_CACHE_TTL_SECS", 5)?,

            rabbitmq_url: env_or("RABBITMQ_URL", "amqp://guest:guest@127.0.0.1:5672"),

            rate_limit_max_requests: env_parse("RATE_LIMIT_MAX_REQUESTS", 10)?,
            rate_limit_window_secs: env_parse("RATE_LIMIT_WINDOW_SECS", 60)?,

            id_batch_size: env_parse("ID_BATCH_SIZE", 1000)?,
            feistel_master_key: env_parse("FEISTEL_MASTER_KEY", 0xDEAD_BEEF_CAFE_BABEu64)?,

            base_url: env_or("BASE_URL", "http://localhost:8080"),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required env var: {0}")]
    Missing(String),
    #[error("invalid value for {key}: {source}")]
    Parse {
        key: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

fn env_required(key: &str) -> Result<String, ConfigError> {
    env::var(key).map_err(|_| ConfigError::Missing(key.to_string()))
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_parse<T>(key: &str, default: T) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    match env::var(key) {
        Ok(val) => val.parse().map_err(|e: T::Err| ConfigError::Parse {
            key: key.to_string(),
            source: Box::new(e),
        }),
        Err(_) => Ok(default),
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
