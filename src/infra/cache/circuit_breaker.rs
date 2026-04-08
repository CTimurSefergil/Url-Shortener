use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Lock-free circuit breaker for Redis.
///
/// States: Closed → Open → HalfOpen → Closed
///
/// Closed: normal operation, errors increment failure counter.
/// Open: all calls bypassed (return None), entered after threshold failures.
/// HalfOpen: after open_duration, exactly one probe call is allowed.
pub struct CircuitBreaker {
    failure_count: AtomicU32,
    last_failure_epoch: AtomicI64,
    failure_threshold: u32,
    open_duration_secs: u64,
    half_open_permit: AtomicBool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CbState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, open_duration_secs: u64) -> Self {
        Self {
            failure_count: AtomicU32::new(0),
            last_failure_epoch: AtomicI64::new(0),
            failure_threshold,
            open_duration_secs,
            half_open_permit: AtomicBool::new(true),
        }
    }

    /// Check if a call should be allowed.
    pub fn allow_request(&self) -> bool {
        match self.state() {
            CbState::Closed => true,
            CbState::HalfOpen => {
                // CAS: only one probe gets through
                self.half_open_permit
                    .compare_exchange(true, false, Ordering::AcqRel, Ordering::Relaxed)
                    .is_ok()
            }
            CbState::Open => false,
        }
    }

    /// Report a successful call — reset to Closed.
    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
        self.half_open_permit.store(true, Ordering::Release);
    }

    /// Report a failed call — increment counter, potentially transition to Open.
    pub fn record_failure(&self) {
        self.failure_count.fetch_add(1, Ordering::Relaxed);
        self.last_failure_epoch
            .store(now_epoch(), Ordering::Relaxed);
        self.half_open_permit.store(true, Ordering::Release);
    }

    /// Current state of the circuit breaker.
    pub fn state(&self) -> CbState {
        let failures = self.failure_count.load(Ordering::Relaxed);
        if failures < self.failure_threshold {
            return CbState::Closed;
        }
        let last_failure = self.last_failure_epoch.load(Ordering::Relaxed);
        let elapsed = now_epoch() - last_failure;
        if elapsed >= self.open_duration_secs as i64 {
            CbState::HalfOpen
        } else {
            CbState::Open
        }
    }

    /// Numeric state for Prometheus gauge: 0=closed, 1=half_open, 2=open.
    pub fn state_as_gauge(&self) -> f64 {
        match self.state() {
            CbState::Closed => 0.0,
            CbState::HalfOpen => 1.0,
            CbState::Open => 2.0,
        }
    }
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_closed() {
        let cb = CircuitBreaker::new(3, 30);
        assert_eq!(cb.state(), CbState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn opens_after_threshold() {
        let cb = CircuitBreaker::new(3, 30);
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), CbState::Closed);
        cb.record_failure();
        assert_eq!(cb.state(), CbState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn success_resets() {
        let cb = CircuitBreaker::new(3, 30);
        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        assert_eq!(cb.state(), CbState::Closed);
    }

    #[test]
    fn half_open_allows_only_one_probe() {
        let cb = CircuitBreaker::new(1, 0); // 0 second open duration
        cb.record_failure();
        // With 0s duration, it should immediately go to HalfOpen
        assert_eq!(cb.state(), CbState::HalfOpen);
        assert!(cb.allow_request()); // first caller wins the probe
        assert!(!cb.allow_request()); // second caller blocked
    }

    #[test]
    fn half_open_permit_resets_on_success() {
        let cb = CircuitBreaker::new(1, 0);
        cb.record_failure();
        assert!(cb.allow_request()); // probe taken
        cb.record_success(); // back to Closed
        assert_eq!(cb.state(), CbState::Closed);
        assert!(cb.allow_request()); // normal request works
    }

    #[test]
    fn gauge_values() {
        let cb = CircuitBreaker::new(1, 30);
        assert_eq!(cb.state_as_gauge(), 0.0);
        cb.record_failure();
        assert_eq!(cb.state_as_gauge(), 2.0);
    }
}
