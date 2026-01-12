//! Simulated NIP-90 swarm for testing distributed scenarios.
//!
//! This backend simulates a distributed compute network with configurable:
//! - Latency distributions (constant, normal, long-tail)
//! - Failure rates
//! - Timeout behavior
//! - Cost variance
//! - Quorum behavior (multiple redundant responses)

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use rand::SeedableRng;
use rand::distributions::{Distribution, Standard};
use rand::rngs::StdRng;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::backend::{LmBackend, LmResponse};
use crate::error::{Error, Result};
use crate::usage::LmUsage;

/// Latency distribution configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LatencyDist {
    /// Fixed latency in milliseconds.
    Constant(u64),
    /// Normal distribution with mean and standard deviation.
    Normal { mean_ms: u64, std_ms: u64 },
    /// Long-tail distribution (realistic swarm behavior).
    /// Most responses come quickly, but some take much longer.
    LongTail { median_ms: u64, p99_ms: u64 },
}

impl Default for LatencyDist {
    fn default() -> Self {
        Self::Normal {
            mean_ms: 500,
            std_ms: 200,
        }
    }
}

impl LatencyDist {
    /// Sample a latency value from this distribution.
    pub fn sample(&self, rng: &mut StdRng) -> u64 {
        match self {
            Self::Constant(ms) => *ms,
            Self::Normal { mean_ms, std_ms } => {
                // Use Box-Muller transform for normal distribution
                let u1: f64 = Standard.sample(rng);
                let u2: f64 = Standard.sample(rng);
                let z = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
                let sample = *mean_ms as f64 + z * *std_ms as f64;
                sample.max(0.0) as u64
            }
            Self::LongTail { median_ms, p99_ms } => {
                // Log-normal distribution approximation
                let u: f64 = Standard.sample(rng);
                if u < 0.5 {
                    // 50% of requests come around median
                    let variance = *median_ms as f64 * 0.3;
                    let r: f64 = Standard.sample(rng);
                    let sample = *median_ms as f64 + (r - 0.5) * variance * 2.0;
                    sample.max(0.0) as u64
                } else if u < 0.99 {
                    // 49% between median and p99
                    let range = *p99_ms - *median_ms;
                    let r: f64 = Standard.sample(rng);
                    let sample = *median_ms as f64 + r * range as f64;
                    sample as u64
                } else {
                    // 1% are outliers (2-5x p99)
                    let r: f64 = Standard.sample(rng);
                    let multiplier = 2.0 + r * 3.0;
                    (*p99_ms as f64 * multiplier) as u64
                }
            }
        }
    }
}

/// Configuration for swarm simulation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmSimConfig {
    /// Latency distribution for responses.
    pub latency: LatencyDist,
    /// Probability of a request failing (0.0-1.0).
    pub failure_rate: f64,
    /// Probability of a request timing out (0.0-1.0).
    pub timeout_rate: f64,
    /// Timeout threshold in milliseconds.
    pub timeout_ms: u64,
    /// Cost per token in satoshis.
    pub cost_per_token_sats: f64,
    /// Cost variance (0.0-1.0) to simulate different provider pricing.
    pub cost_variance: f64,
    /// Number of redundant providers for quorum testing.
    pub quorum_size: usize,
    /// Whether responses should vary slightly (for verification testing).
    pub variance_in_results: bool,
    /// Random seed for reproducibility (None = random).
    pub seed: Option<u64>,
}

impl Default for SwarmSimConfig {
    fn default() -> Self {
        Self {
            latency: LatencyDist::default(),
            failure_rate: 0.0,
            timeout_rate: 0.0,
            timeout_ms: 30000,
            cost_per_token_sats: 0.1,
            cost_variance: 0.0,
            quorum_size: 1,
            variance_in_results: false,
            seed: None,
        }
    }
}

impl SwarmSimConfig {
    /// Create a config for testing quorum scenarios.
    pub fn quorum_test(n: usize) -> Self {
        Self {
            quorum_size: n,
            variance_in_results: true,
            ..Default::default()
        }
    }

    /// Create a config for testing failure handling.
    pub fn with_failures(failure_rate: f64) -> Self {
        Self {
            failure_rate,
            ..Default::default()
        }
    }

    /// Create a config for testing timeouts.
    pub fn with_timeouts(timeout_rate: f64, timeout_ms: u64) -> Self {
        Self {
            timeout_rate,
            timeout_ms,
            ..Default::default()
        }
    }

    /// Create a config for testing long-tail latency.
    pub fn long_tail(median_ms: u64, p99_ms: u64) -> Self {
        Self {
            latency: LatencyDist::LongTail { median_ms, p99_ms },
            ..Default::default()
        }
    }
}

/// A simulated provider in the swarm.
#[derive(Debug)]
struct SimulatedProvider {
    id: String,
    base_response: String,
    cost_multiplier: f64,
}

/// Simulated NIP-90 swarm backend.
pub struct SwarmSimulator {
    config: SwarmSimConfig,
    rng: Arc<Mutex<StdRng>>,
    providers: Vec<SimulatedProvider>,
    total_requests: AtomicU64,
    total_failures: AtomicU64,
    total_timeouts: AtomicU64,
}

impl SwarmSimulator {
    /// Create a new swarm simulator with default config.
    pub fn new() -> Self {
        Self::with_config(SwarmSimConfig::default())
    }

    /// Create a new swarm simulator with custom config.
    pub fn with_config(config: SwarmSimConfig) -> Self {
        let rng = match config.seed {
            Some(seed) => StdRng::seed_from_u64(seed),
            None => StdRng::from_entropy(),
        };

        // Create simulated providers
        let mut providers = Vec::with_capacity(config.quorum_size.max(1));
        for i in 0..config.quorum_size.max(1) {
            providers.push(SimulatedProvider {
                id: format!("provider-{}", i),
                base_response: format!("Response from provider {}", i),
                cost_multiplier: 1.0 + (i as f64 * 0.1), // Slight cost variance
            });
        }

        Self {
            config,
            rng: Arc::new(Mutex::new(rng)),
            providers,
            total_requests: AtomicU64::new(0),
            total_failures: AtomicU64::new(0),
            total_timeouts: AtomicU64::new(0),
        }
    }

    /// Get statistics about the simulation.
    pub fn stats(&self) -> SwarmStats {
        SwarmStats {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            total_failures: self.total_failures.load(Ordering::Relaxed),
            total_timeouts: self.total_timeouts.load(Ordering::Relaxed),
        }
    }

    /// Reset statistics.
    pub fn reset_stats(&self) {
        self.total_requests.store(0, Ordering::Relaxed);
        self.total_failures.store(0, Ordering::Relaxed);
        self.total_timeouts.store(0, Ordering::Relaxed);
    }

    /// Simulate a single provider response.
    async fn simulate_provider(
        &self,
        provider: &SimulatedProvider,
        prompt: &str,
        _max_tokens: usize,
    ) -> Result<LmResponse> {
        let mut rng = self.rng.lock().await;

        // Check for failure
        let fail_roll: f64 = Standard.sample(&mut *rng);
        if fail_roll < self.config.failure_rate {
            self.total_failures.fetch_add(1, Ordering::Relaxed);
            return Err(Error::SimulatedFailure(format!(
                "Provider {} failed",
                provider.id
            )));
        }

        // Sample latency
        let latency_ms = self.config.latency.sample(&mut *rng);

        // Check for timeout
        let timeout_roll: f64 = Standard.sample(&mut *rng);
        if timeout_roll < self.config.timeout_rate || latency_ms > self.config.timeout_ms {
            self.total_timeouts.fetch_add(1, Ordering::Relaxed);
            return Err(Error::Timeout(latency_ms));
        }

        // Simulate the latency
        drop(rng); // Release lock before sleeping
        tokio::time::sleep(Duration::from_millis(latency_ms.min(100))).await; // Cap actual sleep for tests

        // Generate response
        let response_text = if self.config.variance_in_results {
            // Add slight variance for verification testing
            let mut rng = self.rng.lock().await;
            let coin: f64 = Standard.sample(&mut *rng);
            let variance_char = if coin < 0.5 { "." } else { "!" };
            format!(
                "{} processed: {}{}",
                provider.base_response,
                &prompt[..prompt.len().min(50)],
                variance_char
            )
        } else {
            format!(
                "{} processed: {}",
                provider.base_response,
                &prompt[..prompt.len().min(50)]
            )
        };

        // Calculate tokens and cost
        let prompt_tokens = prompt.len() / 4;
        let completion_tokens = response_text.len() / 4;
        let total_tokens = prompt_tokens + completion_tokens;

        let base_cost = total_tokens as f64 * self.config.cost_per_token_sats;
        let cost_with_variance = if self.config.cost_variance > 0.0 {
            let mut rng = self.rng.lock().await;
            let variance_roll: f64 = Standard.sample(&mut *rng);
            let variance_factor = 1.0 + (variance_roll - 0.5) * 2.0 * self.config.cost_variance;
            base_cost * variance_factor * provider.cost_multiplier
        } else {
            base_cost * provider.cost_multiplier
        };

        let usage = LmUsage::new(prompt_tokens, completion_tokens)
            .with_cost_sats(cost_with_variance as u64);

        Ok(LmResponse::new(response_text, "swarm-sim", usage).with_latency(latency_ms))
    }
}

impl Default for SwarmSimulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics from swarm simulation.
#[derive(Debug, Clone)]
pub struct SwarmStats {
    pub total_requests: u64,
    pub total_failures: u64,
    pub total_timeouts: u64,
}

impl SwarmStats {
    /// Get failure rate as a percentage.
    pub fn failure_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.total_failures as f64 / self.total_requests as f64 * 100.0
        }
    }

    /// Get timeout rate as a percentage.
    pub fn timeout_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.total_timeouts as f64 / self.total_requests as f64 * 100.0
        }
    }
}

#[async_trait]
impl LmBackend for SwarmSimulator {
    fn name(&self) -> &str {
        "swarm-sim"
    }

    fn supported_models(&self) -> Vec<String> {
        vec!["swarm-sim".to_string()]
    }

    async fn complete(&self, model: &str, prompt: &str, max_tokens: usize) -> Result<LmResponse> {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        debug!(
            model = model,
            prompt_len = prompt.len(),
            quorum = self.config.quorum_size,
            "Swarm simulation request"
        );

        if self.config.quorum_size <= 1 {
            // Single provider mode
            return self
                .simulate_provider(&self.providers[0], prompt, max_tokens)
                .await;
        }

        // Quorum mode: query multiple providers
        let mut futs = Vec::with_capacity(self.providers.len());
        for provider in &self.providers {
            futs.push(self.simulate_provider(provider, prompt, max_tokens));
        }

        // Wait for all and collect results
        let results: Vec<_> = futures::future::join_all(futs).await;

        // Find first successful result
        // (In a real implementation, you'd implement proper quorum logic)
        let successful: Vec<_> = results.into_iter().filter_map(|r| r.ok()).collect();

        if successful.is_empty() {
            return Err(Error::SimulatedFailure(
                "All providers in quorum failed".to_string(),
            ));
        }

        // Return the first successful response
        // (Could implement majority voting here for variance_in_results mode)
        info!(
            successful_providers = successful.len(),
            total_providers = self.providers.len(),
            "Quorum response"
        );

        Ok(successful.into_iter().next().unwrap())
    }

    async fn health_check(&self) -> bool {
        // Always healthy (it's a simulation)
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_swarm_basic() {
        let sim = SwarmSimulator::new();
        let response = sim.complete("swarm-sim", "Hello world", 100).await.unwrap();

        assert!(!response.text.is_empty());
        assert!(response.usage.total_tokens > 0);
    }

    #[tokio::test]
    async fn test_swarm_with_failures() {
        let config = SwarmSimConfig {
            failure_rate: 1.0, // Always fail
            seed: Some(42),
            ..Default::default()
        };
        let sim = SwarmSimulator::with_config(config);

        let result = sim.complete("swarm-sim", "Hello", 100).await;
        assert!(result.is_err());

        let stats = sim.stats();
        assert_eq!(stats.total_failures, 1);
    }

    #[tokio::test]
    async fn test_swarm_quorum() {
        let config = SwarmSimConfig {
            quorum_size: 3,
            seed: Some(42),
            ..Default::default()
        };
        let sim = SwarmSimulator::with_config(config);

        let response = sim.complete("swarm-sim", "Hello", 100).await.unwrap();
        assert!(!response.text.is_empty());
    }

    #[test]
    fn test_latency_distribution() {
        let mut rng = StdRng::seed_from_u64(42);

        // Constant
        let constant = LatencyDist::Constant(100);
        assert_eq!(constant.sample(&mut rng), 100);

        // Normal - should be around mean
        let normal = LatencyDist::Normal {
            mean_ms: 500,
            std_ms: 100,
        };
        let samples: Vec<u64> = (0..1000).map(|_| normal.sample(&mut rng)).collect();
        let avg = samples.iter().sum::<u64>() as f64 / samples.len() as f64;
        assert!((avg - 500.0).abs() < 50.0); // Within reasonable range

        // Long tail - median should be most common
        let longtail = LatencyDist::LongTail {
            median_ms: 200,
            p99_ms: 2000,
        };
        let samples: Vec<u64> = (0..1000).map(|_| longtail.sample(&mut rng)).collect();
        let median = {
            let mut sorted = samples.clone();
            sorted.sort();
            sorted[500]
        };
        assert!(median < 500); // Median should be relatively low
    }
}
