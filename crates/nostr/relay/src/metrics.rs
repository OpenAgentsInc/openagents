//! Relay metrics collection and monitoring
//!
//! Tracks various metrics about relay operations:
//! - Connection counts and bandwidth
//! - Event processing (stored, rejected, rate limited)
//! - Subscription statistics
//! - Database performance
//! - Rate limiting statistics

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Relay metrics collector
#[derive(Debug)]
pub struct RelayMetrics {
    /// Server start time
    start_time: Instant,

    /// Total active connections
    pub active_connections: AtomicUsize,

    /// Total events received
    pub events_received: AtomicU64,

    /// Total events stored successfully
    pub events_stored: AtomicU64,

    /// Events rejected due to validation failure
    pub events_rejected_validation: AtomicU64,

    /// Events rejected due to rate limiting
    pub events_rejected_rate_limit: AtomicU64,

    /// Events rejected due to signature failure
    pub events_rejected_signature: AtomicU64,

    /// Total active subscriptions across all connections
    pub active_subscriptions: AtomicUsize,

    /// Total subscription requests (REQ)
    pub subscription_requests: AtomicU64,

    /// Total subscription closes (CLOSE)
    pub subscription_closes: AtomicU64,

    /// Total bytes received
    pub bytes_received: AtomicU64,

    /// Total bytes sent
    pub bytes_sent: AtomicU64,

    /// Number of currently banned IPs
    pub banned_ips: AtomicUsize,

    /// Total connection attempts blocked (banned IPs)
    pub connections_blocked_banned: AtomicU64,

    /// Total connection attempts blocked (rate limit)
    pub connections_blocked_rate_limit: AtomicU64,

    /// Database queries executed
    pub db_queries: AtomicU64,

    /// Database query errors
    pub db_errors: AtomicU64,
}

impl RelayMetrics {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            active_connections: AtomicUsize::new(0),
            events_received: AtomicU64::new(0),
            events_stored: AtomicU64::new(0),
            events_rejected_validation: AtomicU64::new(0),
            events_rejected_rate_limit: AtomicU64::new(0),
            events_rejected_signature: AtomicU64::new(0),
            active_subscriptions: AtomicUsize::new(0),
            subscription_requests: AtomicU64::new(0),
            subscription_closes: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            banned_ips: AtomicUsize::new(0),
            connections_blocked_banned: AtomicU64::new(0),
            connections_blocked_rate_limit: AtomicU64::new(0),
            db_queries: AtomicU64::new(0),
            db_errors: AtomicU64::new(0),
        }
    }

    /// Get uptime in seconds
    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Get uptime duration
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Increment active connections
    pub fn connection_opened(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active connections
    pub fn connection_closed(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    /// Record an event received
    pub fn event_received(&self) {
        self.events_received.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an event stored
    pub fn event_stored(&self) {
        self.events_stored.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an event rejected (validation)
    pub fn event_rejected_validation(&self) {
        self.events_rejected_validation
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Record an event rejected (rate limit)
    pub fn event_rejected_rate_limit(&self) {
        self.events_rejected_rate_limit
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Record an event rejected (signature)
    pub fn event_rejected_signature(&self) {
        self.events_rejected_signature
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Increment active subscriptions
    pub fn subscription_opened(&self) {
        self.active_subscriptions.fetch_add(1, Ordering::Relaxed);
        self.subscription_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement active subscriptions
    pub fn subscription_closed(&self) {
        self.active_subscriptions.fetch_sub(1, Ordering::Relaxed);
        self.subscription_closes.fetch_add(1, Ordering::Relaxed);
    }

    /// Record bytes received
    pub fn bytes_in(&self, count: u64) {
        self.bytes_received.fetch_add(count, Ordering::Relaxed);
    }

    /// Record bytes sent
    pub fn bytes_out(&self, count: u64) {
        self.bytes_sent.fetch_add(count, Ordering::Relaxed);
    }

    /// Record IP ban
    pub fn ip_banned(&self) {
        self.banned_ips.fetch_add(1, Ordering::Relaxed);
    }

    /// Record IP unban
    pub fn ip_unbanned(&self) {
        self.banned_ips.fetch_sub(1, Ordering::Relaxed);
    }

    /// Record connection blocked (banned IP)
    pub fn connection_blocked_banned(&self) {
        self.connections_blocked_banned
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Record connection blocked (rate limit)
    pub fn connection_blocked_rate_limit(&self) {
        self.connections_blocked_rate_limit
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Record database query
    pub fn db_query(&self) {
        self.db_queries.fetch_add(1, Ordering::Relaxed);
    }

    /// Record database error
    pub fn db_error(&self) {
        self.db_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Get a snapshot of current metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            uptime_secs: self.uptime_secs(),
            active_connections: self.active_connections.load(Ordering::Relaxed),
            events_received: self.events_received.load(Ordering::Relaxed),
            events_stored: self.events_stored.load(Ordering::Relaxed),
            events_rejected_validation: self.events_rejected_validation.load(Ordering::Relaxed),
            events_rejected_rate_limit: self.events_rejected_rate_limit.load(Ordering::Relaxed),
            events_rejected_signature: self.events_rejected_signature.load(Ordering::Relaxed),
            active_subscriptions: self.active_subscriptions.load(Ordering::Relaxed),
            subscription_requests: self.subscription_requests.load(Ordering::Relaxed),
            subscription_closes: self.subscription_closes.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            banned_ips: self.banned_ips.load(Ordering::Relaxed),
            connections_blocked_banned: self.connections_blocked_banned.load(Ordering::Relaxed),
            connections_blocked_rate_limit: self
                .connections_blocked_rate_limit
                .load(Ordering::Relaxed),
            db_queries: self.db_queries.load(Ordering::Relaxed),
            db_errors: self.db_errors.load(Ordering::Relaxed),
        }
    }

    /// Export metrics in Prometheus text format
    pub fn prometheus_export(&self) -> String {
        let snapshot = self.snapshot();
        format!(
            "# HELP nostr_relay_uptime_seconds Relay uptime in seconds\n\
             # TYPE nostr_relay_uptime_seconds gauge\n\
             nostr_relay_uptime_seconds {}\n\
             # HELP nostr_relay_connections_active Number of active connections\n\
             # TYPE nostr_relay_connections_active gauge\n\
             nostr_relay_connections_active {}\n\
             # HELP nostr_relay_events_received_total Total events received\n\
             # TYPE nostr_relay_events_received_total counter\n\
             nostr_relay_events_received_total {}\n\
             # HELP nostr_relay_events_stored_total Total events stored\n\
             # TYPE nostr_relay_events_stored_total counter\n\
             nostr_relay_events_stored_total {}\n\
             # HELP nostr_relay_events_rejected_total Total events rejected\n\
             # TYPE nostr_relay_events_rejected_total counter\n\
             nostr_relay_events_rejected_total{{reason=\"validation\"}} {}\n\
             nostr_relay_events_rejected_total{{reason=\"rate_limit\"}} {}\n\
             nostr_relay_events_rejected_total{{reason=\"signature\"}} {}\n\
             # HELP nostr_relay_subscriptions_active Number of active subscriptions\n\
             # TYPE nostr_relay_subscriptions_active gauge\n\
             nostr_relay_subscriptions_active {}\n\
             # HELP nostr_relay_subscriptions_total Total subscription requests\n\
             # TYPE nostr_relay_subscriptions_total counter\n\
             nostr_relay_subscriptions_total {}\n\
             # HELP nostr_relay_bytes_received_total Total bytes received\n\
             # TYPE nostr_relay_bytes_received_total counter\n\
             nostr_relay_bytes_received_total {}\n\
             # HELP nostr_relay_bytes_sent_total Total bytes sent\n\
             # TYPE nostr_relay_bytes_sent_total counter\n\
             nostr_relay_bytes_sent_total {}\n\
             # HELP nostr_relay_banned_ips Number of banned IPs\n\
             # TYPE nostr_relay_banned_ips gauge\n\
             nostr_relay_banned_ips {}\n\
             # HELP nostr_relay_connections_blocked_total Total connections blocked\n\
             # TYPE nostr_relay_connections_blocked_total counter\n\
             nostr_relay_connections_blocked_total{{reason=\"banned\"}} {}\n\
             nostr_relay_connections_blocked_total{{reason=\"rate_limit\"}} {}\n\
             # HELP nostr_relay_db_queries_total Total database queries\n\
             # TYPE nostr_relay_db_queries_total counter\n\
             nostr_relay_db_queries_total {}\n\
             # HELP nostr_relay_db_errors_total Total database errors\n\
             # TYPE nostr_relay_db_errors_total counter\n\
             nostr_relay_db_errors_total {}\n",
            snapshot.uptime_secs,
            snapshot.active_connections,
            snapshot.events_received,
            snapshot.events_stored,
            snapshot.events_rejected_validation,
            snapshot.events_rejected_rate_limit,
            snapshot.events_rejected_signature,
            snapshot.active_subscriptions,
            snapshot.subscription_requests,
            snapshot.bytes_received,
            snapshot.bytes_sent,
            snapshot.banned_ips,
            snapshot.connections_blocked_banned,
            snapshot.connections_blocked_rate_limit,
            snapshot.db_queries,
            snapshot.db_errors,
        )
    }
}

impl Default for RelayMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of metrics at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Unix timestamp when snapshot was taken
    pub timestamp: u64,
    /// Uptime in seconds
    pub uptime_secs: u64,
    /// Active connections
    pub active_connections: usize,
    /// Events received
    pub events_received: u64,
    /// Events stored
    pub events_stored: u64,
    /// Events rejected (validation)
    pub events_rejected_validation: u64,
    /// Events rejected (rate limit)
    pub events_rejected_rate_limit: u64,
    /// Events rejected (signature)
    pub events_rejected_signature: u64,
    /// Active subscriptions
    pub active_subscriptions: usize,
    /// Total subscription requests
    pub subscription_requests: u64,
    /// Total subscription closes
    pub subscription_closes: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Banned IPs
    pub banned_ips: usize,
    /// Connections blocked (banned)
    pub connections_blocked_banned: u64,
    /// Connections blocked (rate limit)
    pub connections_blocked_rate_limit: u64,
    /// Database queries
    pub db_queries: u64,
    /// Database errors
    pub db_errors: u64,
}

impl MetricsSnapshot {
    /// Calculate events per second rate
    pub fn events_per_second(&self) -> f64 {
        if self.uptime_secs == 0 {
            0.0
        } else {
            self.events_received as f64 / self.uptime_secs as f64
        }
    }

    /// Calculate storage success rate (%)
    pub fn storage_success_rate(&self) -> f64 {
        if self.events_received == 0 {
            100.0
        } else {
            (self.events_stored as f64 / self.events_received as f64) * 100.0
        }
    }

    /// Calculate average subscriptions per connection
    pub fn avg_subscriptions_per_connection(&self) -> f64 {
        if self.active_connections == 0 {
            0.0
        } else {
            self.active_subscriptions as f64 / self.active_connections as f64
        }
    }

    /// Calculate database error rate (%)
    pub fn db_error_rate(&self) -> f64 {
        if self.db_queries == 0 {
            0.0
        } else {
            (self.db_errors as f64 / self.db_queries as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = RelayMetrics::new();
        assert_eq!(metrics.active_connections.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.events_received.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_connection_tracking() {
        let metrics = RelayMetrics::new();

        metrics.connection_opened();
        assert_eq!(metrics.active_connections.load(Ordering::Relaxed), 1);

        metrics.connection_opened();
        assert_eq!(metrics.active_connections.load(Ordering::Relaxed), 2);

        metrics.connection_closed();
        assert_eq!(metrics.active_connections.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_event_tracking() {
        let metrics = RelayMetrics::new();

        metrics.event_received();
        metrics.event_received();
        assert_eq!(metrics.events_received.load(Ordering::Relaxed), 2);

        metrics.event_stored();
        assert_eq!(metrics.events_stored.load(Ordering::Relaxed), 1);

        metrics.event_rejected_rate_limit();
        assert_eq!(
            metrics.events_rejected_rate_limit.load(Ordering::Relaxed),
            1
        );
    }

    #[test]
    fn test_subscription_tracking() {
        let metrics = RelayMetrics::new();

        metrics.subscription_opened();
        assert_eq!(metrics.active_subscriptions.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.subscription_requests.load(Ordering::Relaxed), 1);

        metrics.subscription_closed();
        assert_eq!(metrics.active_subscriptions.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.subscription_closes.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_snapshot() {
        let metrics = RelayMetrics::new();

        metrics.connection_opened();
        metrics.event_received();
        metrics.event_stored();

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.active_connections, 1);
        assert_eq!(snapshot.events_received, 1);
        assert_eq!(snapshot.events_stored, 1);
    }

    #[test]
    fn test_prometheus_export() {
        let metrics = RelayMetrics::new();
        metrics.connection_opened();
        metrics.event_received();

        let prometheus = metrics.prometheus_export();
        assert!(prometheus.contains("nostr_relay_uptime_seconds"));
        assert!(prometheus.contains("nostr_relay_connections_active 1"));
        assert!(prometheus.contains("nostr_relay_events_received_total 1"));
    }

    #[test]
    fn test_metrics_calculations() {
        let snapshot = MetricsSnapshot {
            timestamp: 0,
            uptime_secs: 100,
            active_connections: 10,
            events_received: 1000,
            events_stored: 950,
            events_rejected_validation: 30,
            events_rejected_rate_limit: 10,
            events_rejected_signature: 10,
            active_subscriptions: 50,
            subscription_requests: 100,
            subscription_closes: 50,
            bytes_received: 1024000,
            bytes_sent: 2048000,
            banned_ips: 5,
            connections_blocked_banned: 20,
            connections_blocked_rate_limit: 10,
            db_queries: 5000,
            db_errors: 50,
        };

        assert_eq!(snapshot.events_per_second(), 10.0);
        assert_eq!(snapshot.storage_success_rate(), 95.0);
        assert_eq!(snapshot.avg_subscriptions_per_connection(), 5.0);
        assert_eq!(snapshot.db_error_rate(), 1.0);
    }
}
