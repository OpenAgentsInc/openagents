/// Authentication Metrics and Monitoring
/// 
/// Phase 4: Comprehensive authentication metrics collection for production monitoring
/// Provides real-time insights into authentication performance, security events, and usage patterns

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH, Instant};
use serde::{Deserialize, Serialize};

/// Authentication metrics collector
#[derive(Debug, Default)]
pub struct AuthMetrics {
    metrics: Arc<Mutex<AuthMetricsData>>,
}

/// Internal metrics data structure
#[derive(Debug, Default)]
struct AuthMetricsData {
    /// Token operations counters
    tokens_stored: u64,
    tokens_retrieved: u64,
    tokens_expired: u64,
    tokens_removed: u64,
    
    /// Authentication attempts
    auth_successes: u64,
    auth_failures: u64,
    
    /// Performance metrics
    avg_token_retrieval_time_ms: f64,
    avg_auth_header_generation_time_ms: f64,
    
    /// Security events
    expired_access_attempts: u64,
    invalid_token_attempts: u64,
    
    /// Session tracking
    active_sessions: u64,
    total_logouts: u64,
    
    /// Performance samples (last 100 operations)
    token_retrieval_samples: Vec<u64>,
    auth_header_samples: Vec<u64>,
}

/// Exportable authentication metrics
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthMetricsSnapshot {
    pub timestamp: u64,
    pub tokens_stored: u64,
    pub tokens_retrieved: u64,
    pub tokens_expired: u64,
    pub tokens_removed: u64,
    pub auth_success_rate: f64,
    pub auth_failures: u64,
    pub avg_token_retrieval_time_ms: f64,
    pub avg_auth_header_generation_time_ms: f64,
    pub expired_access_attempts: u64,
    pub invalid_token_attempts: u64,
    pub active_sessions: u64,
    pub total_logouts: u64,
}

/// Authentication event types for monitoring
#[derive(Debug, Clone)]
pub enum AuthEvent {
    TokenStored { key: String, ttl_seconds: Option<u64> },
    TokenRetrieved { key: String, success: bool, duration_ms: u64 },
    TokenExpired { key: String },
    TokenRemoved { key: String },
    AuthSuccess { method: String },
    AuthFailure { method: String, error: String },
    AuthHeaderGenerated { duration_ms: u64 },
    ExpiredAccessAttempt { key: String },
    InvalidTokenAttempt { key: String },
    SessionStarted,
    UserLogout,
}

impl AuthMetrics {
    /// Create a new authentication metrics collector
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(AuthMetricsData::default())),
        }
    }

    /// Record an authentication event
    pub fn record_event(&self, event: AuthEvent) {
        if let Ok(mut metrics) = self.metrics.lock() {
            match event {
                AuthEvent::TokenStored { key, ttl_seconds } => {
                    metrics.tokens_stored += 1;
                    log::debug!("AUTH_METRICS: Token stored [key={}, ttl={:?}]", key, ttl_seconds);
                }
                AuthEvent::TokenRetrieved { key, success, duration_ms } => {
                    metrics.tokens_retrieved += 1;
                    self.update_token_retrieval_time(&mut metrics, duration_ms);
                    log::debug!("AUTH_METRICS: Token retrieved [key={}, success={}, duration={}ms]", 
                        key, success, duration_ms);
                }
                AuthEvent::TokenExpired { key } => {
                    metrics.tokens_expired += 1;
                    log::warn!("AUTH_METRICS: Token expired [key={}]", key);
                }
                AuthEvent::TokenRemoved { key } => {
                    metrics.tokens_removed += 1;
                    log::debug!("AUTH_METRICS: Token removed [key={}]", key);
                }
                AuthEvent::AuthSuccess { method } => {
                    metrics.auth_successes += 1;
                    log::info!("AUTH_METRICS: Authentication success [method={}]", method);
                }
                AuthEvent::AuthFailure { method, error } => {
                    metrics.auth_failures += 1;
                    log::error!("AUTH_METRICS: Authentication failure [method={}, error={}]", method, error);
                }
                AuthEvent::AuthHeaderGenerated { duration_ms } => {
                    self.update_auth_header_time(&mut metrics, duration_ms);
                    log::debug!("AUTH_METRICS: Auth header generated [duration={}ms]", duration_ms);
                }
                AuthEvent::ExpiredAccessAttempt { key } => {
                    metrics.expired_access_attempts += 1;
                    log::warn!("AUTH_METRICS: Expired token access attempt [key={}]", key);
                }
                AuthEvent::InvalidTokenAttempt { key } => {
                    metrics.invalid_token_attempts += 1;
                    log::warn!("AUTH_METRICS: Invalid token access attempt [key={}]", key);
                }
                AuthEvent::SessionStarted => {
                    metrics.active_sessions += 1;
                    log::info!("AUTH_METRICS: Session started [active_sessions={}]", metrics.active_sessions);
                }
                AuthEvent::UserLogout => {
                    metrics.active_sessions = metrics.active_sessions.saturating_sub(1);
                    metrics.total_logouts += 1;
                    log::info!("AUTH_METRICS: User logout [active_sessions={}, total_logouts={}]", 
                        metrics.active_sessions, metrics.total_logouts);
                }
            }
        }
    }

    /// Get current authentication metrics snapshot
    pub fn get_snapshot(&self) -> AuthMetricsSnapshot {
        let metrics = self.metrics.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let success_rate = if metrics.auth_successes + metrics.auth_failures > 0 {
            metrics.auth_successes as f64 / (metrics.auth_successes + metrics.auth_failures) as f64 * 100.0
        } else {
            100.0
        };

        AuthMetricsSnapshot {
            timestamp: now,
            tokens_stored: metrics.tokens_stored,
            tokens_retrieved: metrics.tokens_retrieved,
            tokens_expired: metrics.tokens_expired,
            tokens_removed: metrics.tokens_removed,
            auth_success_rate: success_rate,
            auth_failures: metrics.auth_failures,
            avg_token_retrieval_time_ms: metrics.avg_token_retrieval_time_ms,
            avg_auth_header_generation_time_ms: metrics.avg_auth_header_generation_time_ms,
            expired_access_attempts: metrics.expired_access_attempts,
            invalid_token_attempts: metrics.invalid_token_attempts,
            active_sessions: metrics.active_sessions,
            total_logouts: metrics.total_logouts,
        }
    }

    /// Log comprehensive metrics summary
    pub fn log_summary(&self) {
        let snapshot = self.get_snapshot();
        
        log::info!("AUTH_METRICS_SUMMARY: [timestamp={}]", snapshot.timestamp);
        log::info!("  Token Operations: stored={}, retrieved={}, expired={}, removed={}", 
            snapshot.tokens_stored, snapshot.tokens_retrieved, snapshot.tokens_expired, snapshot.tokens_removed);
        log::info!("  Authentication: success_rate={:.1}%, failures={}", 
            snapshot.auth_success_rate, snapshot.auth_failures);
        log::info!("  Performance: token_retrieval_avg={:.1}ms, auth_header_avg={:.1}ms",
            snapshot.avg_token_retrieval_time_ms, snapshot.avg_auth_header_generation_time_ms);
        log::info!("  Security: expired_attempts={}, invalid_attempts={}", 
            snapshot.expired_access_attempts, snapshot.invalid_token_attempts);
        log::info!("  Sessions: active={}, total_logouts={}", 
            snapshot.active_sessions, snapshot.total_logouts);
    }

    /// Check for security anomalies and log warnings
    pub fn check_security_anomalies(&self) {
        let snapshot = self.get_snapshot();
        
        // High failure rate
        if snapshot.auth_success_rate < 80.0 && snapshot.auth_failures > 5 {
            log::error!("SECURITY_ALERT: High authentication failure rate [rate={:.1}%, failures={}]", 
                snapshot.auth_success_rate, snapshot.auth_failures);
        }
        
        // Many expired access attempts
        if snapshot.expired_access_attempts > 10 {
            log::error!("SECURITY_ALERT: Excessive expired token access attempts [count={}]", 
                snapshot.expired_access_attempts);
        }
        
        // Performance degradation
        if snapshot.avg_token_retrieval_time_ms > 1000.0 {
            log::warn!("PERFORMANCE_ALERT: Slow token retrieval [avg={}ms]", 
                snapshot.avg_token_retrieval_time_ms);
        }
    }

    /// Update token retrieval time average (rolling average of last 100 samples)
    fn update_token_retrieval_time(&self, metrics: &mut AuthMetricsData, duration_ms: u64) {
        metrics.token_retrieval_samples.push(duration_ms);
        if metrics.token_retrieval_samples.len() > 100 {
            metrics.token_retrieval_samples.remove(0);
        }
        
        metrics.avg_token_retrieval_time_ms = metrics.token_retrieval_samples.iter()
            .map(|&x| x as f64)
            .sum::<f64>() / metrics.token_retrieval_samples.len() as f64;
    }

    /// Update auth header generation time average (rolling average of last 100 samples)
    fn update_auth_header_time(&self, metrics: &mut AuthMetricsData, duration_ms: u64) {
        metrics.auth_header_samples.push(duration_ms);
        if metrics.auth_header_samples.len() > 100 {
            metrics.auth_header_samples.remove(0);
        }
        
        metrics.avg_auth_header_generation_time_ms = metrics.auth_header_samples.iter()
            .map(|&x| x as f64)
            .sum::<f64>() / metrics.auth_header_samples.len() as f64;
    }
}

/// Global authentication metrics instance
lazy_static::lazy_static! {
    pub static ref AUTH_METRICS: AuthMetrics = AuthMetrics::new();
}

/// Helper function to record authentication events
pub fn record_auth_event(event: AuthEvent) {
    AUTH_METRICS.record_event(event);
}

/// Helper function to get metrics snapshot
pub fn get_auth_metrics() -> AuthMetricsSnapshot {
    AUTH_METRICS.get_snapshot()
}

/// Helper function to log metrics summary
pub fn log_auth_metrics_summary() {
    AUTH_METRICS.log_summary();
}

/// Helper function to check for security anomalies
pub fn check_auth_security_anomalies() {
    AUTH_METRICS.check_security_anomalies();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_metrics_creation() {
        let metrics = AuthMetrics::new();
        let snapshot = metrics.get_snapshot();
        
        assert_eq!(snapshot.tokens_stored, 0);
        assert_eq!(snapshot.auth_success_rate, 100.0);
        assert_eq!(snapshot.active_sessions, 0);
    }

    #[test]
    fn test_auth_event_recording() {
        let metrics = AuthMetrics::new();
        
        metrics.record_event(AuthEvent::TokenStored {
            key: "test".to_string(),
            ttl_seconds: Some(3600),
        });
        
        metrics.record_event(AuthEvent::AuthSuccess {
            method: "JWT".to_string(),
        });
        
        let snapshot = metrics.get_snapshot();
        assert_eq!(snapshot.tokens_stored, 1);
        assert_eq!(snapshot.auth_success_rate, 100.0);
    }

    #[test]
    fn test_success_rate_calculation() {
        let metrics = AuthMetrics::new();
        
        // Record some successes and failures
        for _ in 0..8 {
            metrics.record_event(AuthEvent::AuthSuccess {
                method: "JWT".to_string(),
            });
        }
        
        for _ in 0..2 {
            metrics.record_event(AuthEvent::AuthFailure {
                method: "JWT".to_string(),
                error: "Invalid token".to_string(),
            });
        }
        
        let snapshot = metrics.get_snapshot();
        assert_eq!(snapshot.auth_success_rate, 80.0);
        assert_eq!(snapshot.auth_failures, 2);
    }
}