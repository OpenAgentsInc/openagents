//! Rate limiting and spam protection for the relay
//!
//! Implements various rate limits to prevent abuse:
//! - Per-IP connection limits
//! - Per-connection event rate limits
//! - Subscription count limits
//! - Temporary IP banning for abuse

use governor::{Quota, RateLimiter as GovernorRateLimiter, clock, state};
use nonzero_ext::*;
use std::collections::HashMap;
use std::net::IpAddr;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::warn;

/// Rate limiter configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum connections per IP address
    pub max_connections_per_ip: usize,
    /// Maximum events per second per connection
    pub max_events_per_second: u32,
    /// Maximum active subscriptions per connection
    pub max_subscriptions_per_connection: usize,
    /// Duration to ban IPs that exceed limits (in seconds)
    pub ban_duration_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_connections_per_ip: 10,
            max_events_per_second: 10,
            max_subscriptions_per_connection: 20,
            ban_duration_secs: 3600, // 1 hour
        }
    }
}

/// Tracks rate limiting state
pub struct RateLimiter {
    config: RateLimitConfig,
    /// Tracks number of active connections per IP
    connections_per_ip: Arc<RwLock<HashMap<IpAddr, usize>>>,
    /// Tracks banned IPs and when the ban expires
    banned_ips: Arc<RwLock<HashMap<IpAddr, Instant>>>,
    /// Per-connection event rate limiter (quota-based)
    event_limiter: Arc<
        GovernorRateLimiter<state::direct::NotKeyed, state::InMemoryState, clock::DefaultClock>,
    >,
}

impl RateLimiter {
    /// Create a new rate limiter with the given configuration
    pub fn new(config: RateLimitConfig) -> Self {
        // Create quota for events per second
        let quota = Quota::per_second(
            NonZeroU32::new(config.max_events_per_second).unwrap_or(nonzero!(10u32)),
        );

        Self {
            config,
            connections_per_ip: Arc::new(RwLock::new(HashMap::new())),
            banned_ips: Arc::new(RwLock::new(HashMap::new())),
            event_limiter: Arc::new(GovernorRateLimiter::direct(quota)),
        }
    }

    /// Check if an IP is currently banned
    pub async fn is_banned(&self, ip: IpAddr) -> bool {
        let mut banned = self.banned_ips.write().await;

        // Check if IP is banned and if ban has expired
        if let Some(ban_expires) = banned.get(&ip) {
            if Instant::now() < *ban_expires {
                return true;
            } else {
                // Ban has expired, remove it
                banned.remove(&ip);
            }
        }

        false
    }

    /// Ban an IP address for the configured duration
    pub async fn ban_ip(&self, ip: IpAddr) {
        let ban_until = Instant::now() + Duration::from_secs(self.config.ban_duration_secs);
        self.banned_ips.write().await.insert(ip, ban_until);
        warn!(
            "Banned IP {} for {} seconds",
            ip, self.config.ban_duration_secs
        );
    }

    /// Check if a new connection from this IP is allowed
    pub async fn check_connection_allowed(&self, ip: IpAddr) -> bool {
        // Check if IP is banned
        if self.is_banned(ip).await {
            return false;
        }

        let connections = self.connections_per_ip.read().await;
        let count = connections.get(&ip).copied().unwrap_or(0);

        count < self.config.max_connections_per_ip
    }

    /// Register a new connection from an IP
    pub async fn register_connection(&self, ip: IpAddr) {
        let mut connections = self.connections_per_ip.write().await;
        *connections.entry(ip).or_insert(0) += 1;
    }

    /// Unregister a connection from an IP
    pub async fn unregister_connection(&self, ip: IpAddr) {
        let mut connections = self.connections_per_ip.write().await;
        if let Some(count) = connections.get_mut(&ip) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                connections.remove(&ip);
            }
        }
    }

    /// Check if an event is allowed (rate limiting)
    pub fn check_event_allowed(&self) -> bool {
        self.event_limiter.check().is_ok()
    }

    /// Check if adding a subscription is allowed
    pub fn check_subscription_allowed(&self, current_count: usize) -> bool {
        current_count < self.config.max_subscriptions_per_connection
    }

    /// Get the maximum subscriptions per connection limit
    pub fn max_subscriptions(&self) -> usize {
        self.config.max_subscriptions_per_connection
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_connection_limit() {
        let config = RateLimitConfig {
            max_connections_per_ip: 2,
            ..Default::default()
        };
        let limiter = RateLimiter::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        assert!(limiter.check_connection_allowed(ip).await);
        limiter.register_connection(ip).await;

        assert!(limiter.check_connection_allowed(ip).await);
        limiter.register_connection(ip).await;

        // Should hit limit
        assert!(!limiter.check_connection_allowed(ip).await);

        // Unregister one
        limiter.unregister_connection(ip).await;
        assert!(limiter.check_connection_allowed(ip).await);
    }

    #[tokio::test]
    async fn test_ip_banning() {
        let config = RateLimitConfig {
            ban_duration_secs: 1,
            ..Default::default()
        };
        let limiter = RateLimiter::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));

        assert!(!limiter.is_banned(ip).await);

        limiter.ban_ip(ip).await;
        assert!(limiter.is_banned(ip).await);

        // Connection should be blocked
        assert!(!limiter.check_connection_allowed(ip).await);

        // Wait for ban to expire
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert!(!limiter.is_banned(ip).await);
    }

    #[tokio::test]
    async fn test_event_rate_limiting() {
        let config = RateLimitConfig {
            max_events_per_second: 2,
            ..Default::default()
        };
        let limiter = RateLimiter::new(config);

        // First two should succeed
        assert!(limiter.check_event_allowed());
        assert!(limiter.check_event_allowed());

        // Third should fail (quota exhausted)
        assert!(!limiter.check_event_allowed());
    }

    #[test]
    fn test_subscription_limit() {
        let config = RateLimitConfig {
            max_subscriptions_per_connection: 5,
            ..Default::default()
        };
        let limiter = RateLimiter::new(config);

        assert!(limiter.check_subscription_allowed(0));
        assert!(limiter.check_subscription_allowed(4));
        assert!(!limiter.check_subscription_allowed(5));
        assert!(!limiter.check_subscription_allowed(10));
    }
}
