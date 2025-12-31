//! Admin interface and monitoring endpoints
//!
//! Provides HTTP endpoints for relay administration and monitoring:
//! - /admin/health - Health check
//! - /admin/stats - Statistics snapshot
//! - /admin/metrics - Prometheus metrics export

use crate::metrics::RelayMetrics;
use crate::rate_limit::RateLimiter;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use warp::Filter;

/// Admin server configuration
#[derive(Debug, Clone)]
pub struct AdminConfig {
    /// Bind address for admin server
    pub bind_addr: SocketAddr,
}

impl Default for AdminConfig {
    fn default() -> Self {
        // Read port from environment or use default
        let port = std::env::var("NOSTR_ADMIN_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(7001);

        let bind_addr = format!("127.0.0.1:{}", port)
            .parse()
            .expect("Failed to parse admin bind address");

        Self { bind_addr }
    }
}

/// Health check response
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_secs: u64,
    pub timestamp: u64,
}

/// Admin statistics response
#[derive(Debug, Serialize, Deserialize)]
pub struct StatsResponse {
    pub health: HealthResponse,
    pub connections: ConnectionStats,
    pub events: EventStats,
    pub subscriptions: SubscriptionStats,
    pub bandwidth: BandwidthStats,
    pub database: DatabaseStats,
    pub rate_limiting: RateLimitStats,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionStats {
    pub active: usize,
    pub blocked_banned: u64,
    pub blocked_rate_limit: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventStats {
    pub received: u64,
    pub stored: u64,
    pub rejected_validation: u64,
    pub rejected_rate_limit: u64,
    pub rejected_signature: u64,
    pub storage_success_rate: f64,
    pub events_per_second: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubscriptionStats {
    pub active: usize,
    pub total_requests: u64,
    pub total_closes: u64,
    pub avg_per_connection: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BandwidthStats {
    pub bytes_received: u64,
    pub bytes_sent: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DatabaseStats {
    pub queries: u64,
    pub errors: u64,
    pub error_rate: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimitStats {
    pub banned_ips: usize,
}

/// Create admin server routes
pub fn create_admin_routes(
    metrics: Arc<RelayMetrics>,
    rate_limiter: Arc<RateLimiter>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let health = warp::path!("admin" / "health")
        .and(warp::get())
        .and(with_metrics(metrics.clone()))
        .map(|metrics: Arc<RelayMetrics>| {
            let snapshot = metrics.snapshot();
            let response = HealthResponse {
                status: "ok".to_string(),
                uptime_secs: snapshot.uptime_secs,
                timestamp: snapshot.timestamp,
            };
            warp::reply::json(&response)
        });

    let stats = warp::path!("admin" / "stats")
        .and(warp::get())
        .and(with_metrics(metrics.clone()))
        .and(with_rate_limiter(rate_limiter.clone()))
        .map(
            |metrics: Arc<RelayMetrics>, _rate_limiter: Arc<RateLimiter>| {
                let snapshot = metrics.snapshot();
                let response = StatsResponse {
                    health: HealthResponse {
                        status: "ok".to_string(),
                        uptime_secs: snapshot.uptime_secs,
                        timestamp: snapshot.timestamp,
                    },
                    connections: ConnectionStats {
                        active: snapshot.active_connections,
                        blocked_banned: snapshot.connections_blocked_banned,
                        blocked_rate_limit: snapshot.connections_blocked_rate_limit,
                    },
                    events: EventStats {
                        received: snapshot.events_received,
                        stored: snapshot.events_stored,
                        rejected_validation: snapshot.events_rejected_validation,
                        rejected_rate_limit: snapshot.events_rejected_rate_limit,
                        rejected_signature: snapshot.events_rejected_signature,
                        storage_success_rate: snapshot.storage_success_rate(),
                        events_per_second: snapshot.events_per_second(),
                    },
                    subscriptions: SubscriptionStats {
                        active: snapshot.active_subscriptions,
                        total_requests: snapshot.subscription_requests,
                        total_closes: snapshot.subscription_closes,
                        avg_per_connection: snapshot.avg_subscriptions_per_connection(),
                    },
                    bandwidth: BandwidthStats {
                        bytes_received: snapshot.bytes_received,
                        bytes_sent: snapshot.bytes_sent,
                    },
                    database: DatabaseStats {
                        queries: snapshot.db_queries,
                        errors: snapshot.db_errors,
                        error_rate: snapshot.db_error_rate(),
                    },
                    rate_limiting: RateLimitStats {
                        banned_ips: snapshot.banned_ips,
                    },
                };
                warp::reply::json(&response)
            },
        );

    let metrics_endpoint = warp::path!("admin" / "metrics")
        .and(warp::get())
        .and(with_metrics(metrics.clone()))
        .map(|metrics: Arc<RelayMetrics>| {
            let prometheus = metrics.prometheus_export();
            warp::reply::with_header(prometheus, "Content-Type", "text/plain; version=0.0.4")
        });

    let dashboard = warp::path!("admin" / "dashboard").and(warp::get()).map(|| {
        let html = include_str!("dashboard.html");
        warp::reply::html(html)
    });

    let admin_root = warp::path!("admin")
        .and(warp::get())
        .map(|| warp::redirect::redirect(warp::http::Uri::from_static("/admin/dashboard")));

    admin_root
        .or(dashboard)
        .or(health)
        .or(stats)
        .or(metrics_endpoint)
        .with(warp::cors().allow_any_origin())
}

/// Helper to inject metrics into routes
fn with_metrics(
    metrics: Arc<RelayMetrics>,
) -> impl Filter<Extract = (Arc<RelayMetrics>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || metrics.clone())
}

/// Helper to inject rate limiter into routes
fn with_rate_limiter(
    rate_limiter: Arc<RateLimiter>,
) -> impl Filter<Extract = (Arc<RateLimiter>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || rate_limiter.clone())
}

/// Start admin server
pub async fn start_admin_server(
    config: AdminConfig,
    metrics: Arc<RelayMetrics>,
    rate_limiter: Arc<RateLimiter>,
) {
    let routes = create_admin_routes(metrics, rate_limiter);

    tracing::info!("Admin server listening on http://{}", config.bind_addr);
    tracing::info!("  - Dashboard: http://{}/admin/dashboard", config.bind_addr);
    tracing::info!("  - Health: http://{}/admin/health", config.bind_addr);
    tracing::info!("  - Stats: http://{}/admin/stats", config.bind_addr);
    tracing::info!("  - Metrics: http://{}/admin/metrics", config.bind_addr);

    warp::serve(routes).run(config.bind_addr).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rate_limit::RateLimitConfig;

    #[tokio::test]
    async fn test_health_endpoint() {
        let metrics = Arc::new(RelayMetrics::new());
        let rate_limiter = Arc::new(RateLimiter::new(RateLimitConfig::default()));
        let routes = create_admin_routes(metrics, rate_limiter);

        let response = warp::test::request()
            .method("GET")
            .path("/admin/health")
            .reply(&routes)
            .await;

        assert_eq!(response.status(), 200);
        let body: HealthResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body.status, "ok");
    }

    #[tokio::test]
    async fn test_stats_endpoint() {
        let metrics = Arc::new(RelayMetrics::new());
        let rate_limiter = Arc::new(RateLimiter::new(RateLimitConfig::default()));

        // Add some test metrics
        metrics.connection_opened();
        metrics.event_received();
        metrics.event_stored();

        let routes = create_admin_routes(metrics, rate_limiter);

        let response = warp::test::request()
            .method("GET")
            .path("/admin/stats")
            .reply(&routes)
            .await;

        assert_eq!(response.status(), 200);
        let body: StatsResponse = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(body.connections.active, 1);
        assert_eq!(body.events.received, 1);
        assert_eq!(body.events.stored, 1);
    }

    #[tokio::test]
    async fn test_metrics_endpoint() {
        let metrics = Arc::new(RelayMetrics::new());
        let rate_limiter = Arc::new(RateLimiter::new(RateLimitConfig::default()));

        metrics.connection_opened();
        metrics.event_received();

        let routes = create_admin_routes(metrics, rate_limiter);

        let response = warp::test::request()
            .method("GET")
            .path("/admin/metrics")
            .reply(&routes)
            .await;

        assert_eq!(response.status(), 200);
        let body = String::from_utf8(response.body().to_vec())
            .expect("Response body contains invalid UTF-8");
        assert!(body.contains("nostr_relay_uptime_seconds"));
        assert!(body.contains("nostr_relay_connections_active"));
        assert!(body.contains("nostr_relay_events_received_total"));
    }

    #[test]
    fn test_stats_response_serialization() {
        let response = StatsResponse {
            health: HealthResponse {
                status: "ok".to_string(),
                uptime_secs: 100,
                timestamp: 1234567890,
            },
            connections: ConnectionStats {
                active: 10,
                blocked_banned: 5,
                blocked_rate_limit: 3,
            },
            events: EventStats {
                received: 1000,
                stored: 950,
                rejected_validation: 30,
                rejected_rate_limit: 10,
                rejected_signature: 10,
                storage_success_rate: 95.0,
                events_per_second: 10.0,
            },
            subscriptions: SubscriptionStats {
                active: 50,
                total_requests: 100,
                total_closes: 50,
                avg_per_connection: 5.0,
            },
            bandwidth: BandwidthStats {
                bytes_received: 1024000,
                bytes_sent: 2048000,
            },
            database: DatabaseStats {
                queries: 5000,
                errors: 50,
                error_rate: 1.0,
            },
            rate_limiting: RateLimitStats { banned_ips: 5 },
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"status\":\"ok\""));
        assert!(json.contains("\"active\":10"));
    }
}
