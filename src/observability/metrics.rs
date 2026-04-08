use prometheus::{
    CounterVec, GaugeVec, HistogramOpts, HistogramVec, IntGaugeVec, Opts, Registry,
};

/// Application-level Prometheus metrics.
pub struct AppMetrics {
    pub cache_requests: CounterVec,
    pub cache_latency: HistogramVec,
    pub db_latency: HistogramVec,
    pub circuit_breaker_state: GaugeVec,
    pub redirect_total: CounterVec,
    pub active_stampede_locks: IntGaugeVec,
}

impl AppMetrics {
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let cache_requests = CounterVec::new(
            Opts::new("cache_requests_total", "Total cache requests"),
            &["operation", "result"],
        )?;
        registry.register(Box::new(cache_requests.clone()))?;

        let cache_latency = HistogramVec::new(
            HistogramOpts::new("cache_latency_seconds", "Redis operation latency")
                .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1]),
            &["operation"],
        )?;
        registry.register(Box::new(cache_latency.clone()))?;

        let db_latency = HistogramVec::new(
            HistogramOpts::new("db_latency_seconds", "Database query latency")
                .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]),
            &["operation"],
        )?;
        registry.register(Box::new(db_latency.clone()))?;

        let circuit_breaker_state = GaugeVec::new(
            Opts::new(
                "circuit_breaker_state",
                "Circuit breaker state (0=closed, 1=half_open, 2=open)",
            ),
            &["name"],
        )?;
        registry.register(Box::new(circuit_breaker_state.clone()))?;

        let redirect_total = CounterVec::new(
            Opts::new("redirect_total", "Total redirect responses by status"),
            &["status"],
        )?;
        registry.register(Box::new(redirect_total.clone()))?;

        let active_stampede_locks = IntGaugeVec::new(
            Opts::new("active_stampede_locks", "Active stampede protection locks"),
            &[],
        )?;
        registry.register(Box::new(active_stampede_locks.clone()))?;

        Ok(Self {
            cache_requests,
            cache_latency,
            db_latency,
            circuit_breaker_state,
            redirect_total,
            active_stampede_locks,
        })
    }
}
