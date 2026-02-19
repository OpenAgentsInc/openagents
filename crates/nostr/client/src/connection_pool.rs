//! Per-relay connection pooling for efficient connection reuse
//!
//! This module provides connection pooling for individual relays to avoid
//! connection exhaustion and improve performance when multiple operations
//! target the same relay concurrently.

use crate::error::{ClientError, Result};
use crate::recovery::{CircuitBreaker, ExponentialBackoff, HealthMetrics};
use crate::relay::{RelayConfig, RelayConnection};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock, Semaphore};
use tracing::{debug, warn};

/// Configuration for per-relay connection pool
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    /// Maximum number of connections per relay
    pub max_connections_per_relay: usize,
    /// Connection idle timeout before cleanup
    pub idle_timeout: Duration,
    /// Interval for cleaning up idle connections
    pub cleanup_interval: Duration,
    /// Relay configuration template
    pub relay_config: RelayConfig,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            max_connections_per_relay: 5,
            idle_timeout: Duration::from_secs(300), // 5 minutes
            cleanup_interval: Duration::from_secs(60), // 1 minute
            relay_config: RelayConfig::default(),
        }
    }
}

/// A pooled connection wrapper with idle tracking
#[derive(Clone)]
struct PooledConnection {
    connection: Arc<RelayConnection>,
    last_used: Arc<Mutex<Instant>>,
    in_use: Arc<Mutex<bool>>,
}

impl PooledConnection {
    fn new(connection: Arc<RelayConnection>) -> Self {
        Self {
            connection,
            last_used: Arc::new(Mutex::new(Instant::now())),
            in_use: Arc::new(Mutex::new(false)),
        }
    }

    async fn checkout(&self) -> Arc<RelayConnection> {
        *self.in_use.lock().await = true;
        *self.last_used.lock().await = Instant::now();
        Arc::clone(&self.connection)
    }

    async fn checkin(&self) {
        *self.in_use.lock().await = false;
        *self.last_used.lock().await = Instant::now();
    }

    async fn is_idle(&self, timeout: Duration) -> bool {
        let in_use = *self.in_use.lock().await;
        if in_use {
            return false;
        }

        let last_used = *self.last_used.lock().await;
        last_used.elapsed() > timeout
    }
}

/// Per-relay connection pool
struct RelayConnectionPool {
    /// Relay URL
    url: String,
    /// Pool of connections
    connections: Vec<PooledConnection>,
    /// Semaphore for limiting concurrent checkouts
    semaphore: Arc<Semaphore>,
    /// Configuration
    config: ConnectionPoolConfig,
    /// Circuit breaker shared across connections
    circuit_breaker: Arc<CircuitBreaker>,
    /// Backoff strategy shared across connections
    #[allow(dead_code)]
    backoff: Arc<Mutex<ExponentialBackoff>>,
    /// Health metrics
    health_metrics: Arc<RwLock<HealthMetrics>>,
}

impl RelayConnectionPool {
    fn new(url: String, config: ConnectionPoolConfig) -> Self {
        Self {
            url: url.clone(),
            connections: Vec::new(),
            semaphore: Arc::new(Semaphore::new(config.max_connections_per_relay)),
            config,
            circuit_breaker: Arc::new(CircuitBreaker::new(5, 2, Duration::from_secs(30))),
            backoff: Arc::new(Mutex::new(ExponentialBackoff::new(
                Duration::from_millis(100),
                Duration::from_secs(30),
                10,
            ))),
            health_metrics: Arc::new(RwLock::new(HealthMetrics::new(&url))),
        }
    }

    /// Checkout a connection from the pool
    async fn checkout(&mut self) -> Result<(Arc<RelayConnection>, usize)> {
        // Check circuit breaker
        if !self.circuit_breaker.is_allowed().await {
            return Err(ClientError::CircuitOpen(format!(
                "Circuit breaker open for relay: {}",
                self.url
            )));
        }

        // Acquire semaphore permit
        let _permit = self.semaphore.acquire().await.map_err(|e| {
            ClientError::Internal(format!("Failed to acquire connection permit: {}", e))
        })?;

        // Try to find an available connection
        for (idx, pooled) in self.connections.iter().enumerate() {
            let in_use = *pooled.in_use.lock().await;
            if !in_use {
                let conn = pooled.checkout().await;
                debug!("Reused connection {} for relay {}", idx, self.url);
                return Ok((conn, idx));
            }
        }

        // All connections in use, create new one if under limit
        if self.connections.len() < self.config.max_connections_per_relay {
            let relay = RelayConnection::with_config(&self.url, self.config.relay_config.clone())?;
            relay.connect().await?;

            let pooled = PooledConnection::new(Arc::new(relay));
            let conn = pooled.checkout().await;
            let idx = self.connections.len();
            self.connections.push(pooled);

            debug!("Created connection {} for relay {}", idx, self.url);

            // Record success
            self.circuit_breaker.record_success().await;

            Ok((conn, idx))
        } else {
            Err(ClientError::Internal(format!(
                "All connections in use for relay: {}",
                self.url
            )))
        }
    }

    /// Return a connection to the pool
    async fn checkin(&self, index: usize) -> Result<()> {
        if let Some(pooled) = self.connections.get(index) {
            pooled.checkin().await;
            debug!("Returned connection {} to pool for {}", index, self.url);
            Ok(())
        } else {
            Err(ClientError::Internal(format!(
                "Invalid connection index: {}",
                index
            )))
        }
    }

    /// Clean up idle connections
    async fn cleanup_idle(&mut self) {
        let idle_timeout = self.config.idle_timeout;
        let mut to_remove = Vec::new();

        for (idx, pooled) in self.connections.iter().enumerate() {
            if pooled.is_idle(idle_timeout).await {
                to_remove.push(idx);
            }
        }

        // Remove in reverse order to maintain indices
        for idx in to_remove.into_iter().rev() {
            if let Some(pooled) = self.connections.get(idx)
                && let Err(e) = pooled.connection.disconnect().await
            {
                warn!("Error disconnecting idle connection: {}", e);
            }
            self.connections.remove(idx);
            debug!("Removed idle connection {} from {}", idx, self.url);
        }
    }

    /// Get health metrics for this relay pool
    async fn health(&self) -> HealthMetrics {
        self.health_metrics.read().await.clone()
    }

    /// Get pool statistics
    async fn stats(&self) -> PoolStats {
        let mut active = 0;
        for pooled in &self.connections {
            if *pooled.in_use.lock().await {
                active += 1;
            }
        }

        PoolStats {
            url: self.url.clone(),
            total_connections: self.connections.len(),
            active_connections: active,
            max_connections: self.config.max_connections_per_relay,
            available_permits: self.semaphore.available_permits(),
        }
    }
}

/// Statistics for a connection pool
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Relay URL
    pub url: String,
    /// Total connections in pool
    pub total_connections: usize,
    /// Active (checked out) connections
    pub active_connections: usize,
    /// Maximum allowed connections
    pub max_connections: usize,
    /// Available semaphore permits
    pub available_permits: usize,
}

/// Connection pool manager for multiple relays
pub struct ConnectionPoolManager {
    /// Pools per relay URL
    pools: Arc<RwLock<HashMap<String, RelayConnectionPool>>>,
    /// Configuration
    config: ConnectionPoolConfig,
}

impl Default for ConnectionPoolManager {
    fn default() -> Self {
        Self::new(ConnectionPoolConfig::default())
    }
}

impl ConnectionPoolManager {
    /// Create a new connection pool manager
    pub fn new(config: ConnectionPoolConfig) -> Self {
        let manager = Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
            config,
        };

        // Start cleanup task
        manager.start_cleanup_task();

        manager
    }

    /// Checkout a connection for a relay
    pub async fn checkout(&self, url: &str) -> Result<(Arc<RelayConnection>, String, usize)> {
        let mut pools = self.pools.write().await;

        // Get or create pool for this relay
        let pool = pools.entry(url.to_string()).or_insert_with(|| {
            debug!("Creating new connection pool for {}", url);
            RelayConnectionPool::new(url.to_string(), self.config.clone())
        });

        let (conn, idx) = pool.checkout().await?;
        Ok((conn, url.to_string(), idx))
    }

    /// Return a connection to the pool
    pub async fn checkin(&self, url: &str, index: usize) -> Result<()> {
        let pools = self.pools.read().await;

        if let Some(pool) = pools.get(url) {
            pool.checkin(index).await
        } else {
            Err(ClientError::Internal(format!(
                "No pool found for relay: {}",
                url
            )))
        }
    }

    /// Get health metrics for a relay
    pub async fn health(&self, url: &str) -> Option<HealthMetrics> {
        let pools = self.pools.read().await;
        if let Some(pool) = pools.get(url) {
            Some(pool.health().await)
        } else {
            None
        }
    }

    /// Get statistics for all pools
    pub async fn all_stats(&self) -> Vec<PoolStats> {
        let pools = self.pools.read().await;
        let mut stats = Vec::new();

        for pool in pools.values() {
            stats.push(pool.stats().await);
        }

        stats
    }

    /// Get statistics for a specific relay
    pub async fn stats(&self, url: &str) -> Option<PoolStats> {
        let pools = self.pools.read().await;
        if let Some(pool) = pools.get(url) {
            Some(pool.stats().await)
        } else {
            None
        }
    }

    /// Start background cleanup task
    fn start_cleanup_task(&self) {
        let pools = Arc::clone(&self.pools);
        let cleanup_interval = self.config.cleanup_interval;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);

            loop {
                interval.tick().await;

                let mut pools = pools.write().await;
                for pool in pools.values_mut() {
                    pool.cleanup_idle().await;
                }

                debug!("Cleaned up idle connections across all pools");
            }
        });
    }

    /// Remove a relay pool (disconnects all connections)
    pub async fn remove_pool(&self, url: &str) -> Result<()> {
        let mut pools = self.pools.write().await;

        if let Some(pool) = pools.remove(url) {
            // Disconnect all connections
            for pooled in pool.connections.iter() {
                if let Err(e) = pooled.connection.disconnect().await {
                    warn!("Error disconnecting connection during pool removal: {}", e);
                }
            }
            debug!("Removed connection pool for {}", url);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pool_manager_creation() {
        let manager = ConnectionPoolManager::new(ConnectionPoolConfig::default());
        let stats = manager.all_stats().await;
        assert_eq!(stats.len(), 0);
    }

    #[tokio::test]
    async fn test_pool_stats() {
        let config = ConnectionPoolConfig {
            max_connections_per_relay: 3,
            ..Default::default()
        };
        let manager = ConnectionPoolManager::new(config);

        // This will fail to connect but will create the pool
        let _ = manager.checkout("wss://relay.example.com").await;

        let stats = manager.stats("wss://relay.example.com").await;
        assert!(stats.is_some());

        if let Some(s) = stats {
            assert_eq!(s.url, "wss://relay.example.com");
            assert_eq!(s.max_connections, 3);
        }
    }

    #[tokio::test]
    async fn test_remove_pool() {
        let manager = ConnectionPoolManager::new(ConnectionPoolConfig::default());

        // Create pool (will fail to connect but pool will exist)
        let _ = manager.checkout("wss://relay.example.com").await;
        assert!(manager.stats("wss://relay.example.com").await.is_some());

        // Remove pool
        manager
            .remove_pool("wss://relay.example.com")
            .await
            .unwrap();
        assert!(manager.stats("wss://relay.example.com").await.is_none());
    }
}
