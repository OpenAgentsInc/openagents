//! Signature router for A/B testing and shadow mode.
//!
//! Routes requests to appropriate compiled modules based on routing strategy.
//! Supports:
//! - Promoted: Always use the promoted module
//! - Shadow: Run candidate alongside promoted, compare results
//! - ABTest: Route percentage of traffic to candidate
//!
//! # Usage
//!
//! ```ignore
//! use autopilot_core::dspy_router::SignatureRouter;
//! use autopilot_core::dspy_hub::{DspyHub, RoutingStrategy};
//!
//! let hub = DspyHub::new();
//! let mut router = SignatureRouter::new(hub);
//!
//! // Set up A/B test
//! router.set_routing(
//!     "PlanningSignature",
//!     RoutingStrategy::ABTest {
//!         candidate_pct: 0.1,
//!         candidate_id: "abc123".to_string(),
//!     },
//! );
//!
//! // Get module for a request
//! let manifest = router.get_module("PlanningSignature")?;
//! ```

use crate::dspy_hub::{DspyHub, RoutingStrategy};
use anyhow::Result;
use dsrs::evaluate::promotion::{ShadowResult, ShadowTaskResult, ShadowWinner};
use dsrs::manifest::CompiledModuleManifest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Router for selecting compiled modules based on routing strategy.
pub struct SignatureRouter {
    /// Hub for loading modules.
    hub: DspyHub,

    /// Active routing strategies per signature.
    active_routes: HashMap<String, RoutingStrategy>,

    /// Shadow mode statistics per signature.
    shadow_stats: HashMap<String, ShadowStats>,
}

/// Statistics for shadow mode comparison.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShadowStats {
    /// Candidate module ID.
    pub candidate_id: String,

    /// Number of times candidate won.
    pub candidate_wins: usize,

    /// Number of times production won.
    pub production_wins: usize,

    /// Number of ties.
    pub ties: usize,

    /// Total candidate score across all tasks.
    pub candidate_score_sum: f64,

    /// Total production score across all tasks.
    pub production_score_sum: f64,

    /// Task-level results.
    pub task_results: Vec<ShadowTaskResult>,

    /// When shadow mode started.
    pub started_at: u64,
}

impl ShadowStats {
    /// Create new shadow stats for a candidate.
    pub fn new(candidate_id: String) -> Self {
        Self {
            candidate_id,
            started_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            ..Default::default()
        }
    }

    /// Get total samples.
    pub fn total_samples(&self) -> usize {
        self.candidate_wins + self.production_wins + self.ties
    }

    /// Get candidate win rate.
    pub fn candidate_win_rate(&self) -> f64 {
        let total = self.total_samples();
        if total == 0 {
            0.5
        } else {
            self.candidate_wins as f64 / total as f64
        }
    }

    /// Get average candidate score.
    pub fn avg_candidate_score(&self) -> f64 {
        let total = self.total_samples();
        if total == 0 {
            0.0
        } else {
            self.candidate_score_sum / total as f64
        }
    }

    /// Get average production score.
    pub fn avg_production_score(&self) -> f64 {
        let total = self.total_samples();
        if total == 0 {
            0.0
        } else {
            self.production_score_sum / total as f64
        }
    }

    /// Get duration in shadow mode.
    pub fn duration(&self) -> Duration {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Duration::from_secs(now.saturating_sub(self.started_at))
    }

    /// Convert to ShadowResult for compatibility.
    pub fn to_shadow_result(&self) -> ShadowResult {
        ShadowResult {
            candidate_wins: self.candidate_wins,
            production_wins: self.production_wins,
            ties: self.ties,
            candidate_score: self.avg_candidate_score(),
            production_score: self.avg_production_score(),
            duration: self.duration(),
            per_task: self.task_results.clone(),
        }
    }

    /// Check if candidate should be promoted.
    pub fn should_promote(&self, min_samples: usize, min_win_rate: f64) -> bool {
        self.total_samples() >= min_samples && self.candidate_win_rate() >= min_win_rate
    }
}

/// Result of a routing decision.
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    /// The module to use.
    pub manifest: CompiledModuleManifest,

    /// Whether this is the candidate (for shadow/A/B).
    pub is_candidate: bool,

    /// The routing strategy that was applied.
    pub strategy: RoutingStrategy,
}

impl SignatureRouter {
    /// Create a new router with the given hub.
    pub fn new(hub: DspyHub) -> Self {
        Self {
            hub,
            active_routes: HashMap::new(),
            shadow_stats: HashMap::new(),
        }
    }

    /// Set the routing strategy for a signature.
    pub fn set_routing(&mut self, signature_name: &str, strategy: RoutingStrategy) {
        // Initialize shadow stats if entering shadow mode
        if let RoutingStrategy::Shadow { candidate_id } = &strategy {
            if !self.shadow_stats.contains_key(signature_name) {
                self.shadow_stats
                    .insert(signature_name.to_string(), ShadowStats::new(candidate_id.clone()));
            }
        }

        self.active_routes.insert(signature_name.to_string(), strategy);
    }

    /// Get the current routing strategy for a signature.
    pub fn get_routing(&self, signature_name: &str) -> RoutingStrategy {
        self.active_routes
            .get(signature_name)
            .cloned()
            .unwrap_or_default()
    }

    /// Clear routing for a signature (fall back to Promoted).
    pub fn clear_routing(&mut self, signature_name: &str) {
        self.active_routes.remove(signature_name);
        self.shadow_stats.remove(signature_name);
    }

    /// Get a module based on current routing strategy.
    pub fn get_module(&self, signature_name: &str) -> Result<CompiledModuleManifest> {
        let strategy = self.get_routing(signature_name);
        self.hub.get_module_for_routing(signature_name, &strategy)
    }

    /// Get a routing decision with metadata.
    pub fn get_routing_decision(&self, signature_name: &str) -> Result<RoutingDecision> {
        let strategy = self.get_routing(signature_name);
        let manifest = self.hub.get_module_for_routing(signature_name, &strategy)?;

        let is_candidate = match &strategy {
            RoutingStrategy::Promoted => false,
            RoutingStrategy::Shadow { candidate_id } => {
                manifest.compiled_id.as_deref() == Some(candidate_id.as_str())
            }
            RoutingStrategy::ABTest { candidate_id, .. } => {
                manifest.compiled_id.as_deref() == Some(candidate_id.as_str())
            }
        };

        Ok(RoutingDecision {
            manifest,
            is_candidate,
            strategy,
        })
    }

    /// Record a shadow mode result.
    pub fn record_shadow_result(&mut self, signature_name: &str, result: ShadowTaskResult) {
        if let Some(stats) = self.shadow_stats.get_mut(signature_name) {
            // Update scores
            stats.candidate_score_sum += result.candidate_score;
            stats.production_score_sum += result.production_score;

            // Update wins
            match result.winner {
                ShadowWinner::Candidate => stats.candidate_wins += 1,
                ShadowWinner::Production => stats.production_wins += 1,
                ShadowWinner::Tie => stats.ties += 1,
            }

            // Store task result
            stats.task_results.push(result);
        }
    }

    /// Record a shadow comparison (convenience method).
    pub fn record_shadow_comparison(
        &mut self,
        signature_name: &str,
        task_id: &str,
        candidate_score: f64,
        production_score: f64,
        tie_margin: f64,
    ) {
        let diff = candidate_score - production_score;
        let winner = if diff > tie_margin {
            ShadowWinner::Candidate
        } else if diff < -tie_margin {
            ShadowWinner::Production
        } else {
            ShadowWinner::Tie
        };

        let result = ShadowTaskResult {
            task_id: task_id.to_string(),
            candidate_score,
            production_score,
            winner,
        };

        self.record_shadow_result(signature_name, result);
    }

    /// Get shadow statistics for a signature.
    pub fn get_shadow_stats(&self, signature_name: &str) -> Option<&ShadowStats> {
        self.shadow_stats.get(signature_name)
    }

    /// Get shadow result for a signature (compatible with PromotionManager).
    pub fn get_shadow_result(&self, signature_name: &str) -> Option<ShadowResult> {
        self.shadow_stats.get(signature_name).map(|s| s.to_shadow_result())
    }

    /// Check if candidate should be promoted based on shadow results.
    pub fn should_promote_candidate(
        &self,
        signature_name: &str,
        min_samples: usize,
        min_win_rate: f64,
    ) -> bool {
        self.shadow_stats
            .get(signature_name)
            .map(|s| s.should_promote(min_samples, min_win_rate))
            .unwrap_or(false)
    }

    /// Get all signatures with active routing.
    pub fn active_signatures(&self) -> Vec<&String> {
        self.active_routes.keys().collect()
    }

    /// Get summary of all routing configurations.
    pub fn routing_summary(&self) -> HashMap<String, RoutingSummary> {
        self.active_routes
            .iter()
            .map(|(sig, strategy)| {
                let shadow_stats = self.shadow_stats.get(sig);
                let summary = RoutingSummary {
                    strategy: strategy.clone(),
                    shadow_samples: shadow_stats.map(|s| s.total_samples()),
                    candidate_win_rate: shadow_stats.map(|s| s.candidate_win_rate()),
                };
                (sig.clone(), summary)
            })
            .collect()
    }
}

/// Summary of routing configuration for a signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingSummary {
    /// The routing strategy.
    pub strategy: RoutingStrategy,

    /// Number of shadow samples (if in shadow mode).
    pub shadow_samples: Option<usize>,

    /// Candidate win rate (if in shadow mode).
    pub candidate_win_rate: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsrs::manifest::Scorecard;
    use dsrs::evaluate::promotion::PromotionState;
    use tempfile::TempDir;

    fn test_router() -> (SignatureRouter, TempDir) {
        let temp = TempDir::new().unwrap();
        let hub = DspyHub::with_base_path(temp.path().to_path_buf());
        let router = SignatureRouter::new(hub);
        (router, temp)
    }

    #[test]
    fn test_default_routing() {
        let (router, _temp) = test_router();
        let strategy = router.get_routing("AnySignature");
        assert_eq!(strategy, RoutingStrategy::Promoted);
    }

    #[test]
    fn test_set_routing() {
        let (mut router, _temp) = test_router();

        router.set_routing(
            "TestSig",
            RoutingStrategy::ABTest {
                candidate_pct: 0.2,
                candidate_id: "abc123".to_string(),
            },
        );

        let strategy = router.get_routing("TestSig");
        match strategy {
            RoutingStrategy::ABTest { candidate_pct, candidate_id } => {
                assert_eq!(candidate_pct, 0.2);
                assert_eq!(candidate_id, "abc123");
            }
            _ => panic!("Expected ABTest strategy"),
        }
    }

    #[test]
    fn test_shadow_stats() {
        let (mut router, _temp) = test_router();

        router.set_routing(
            "ShadowTest",
            RoutingStrategy::Shadow {
                candidate_id: "candidate123".to_string(),
            },
        );

        // Record some results
        router.record_shadow_comparison("ShadowTest", "task1", 0.9, 0.8, 0.01);
        router.record_shadow_comparison("ShadowTest", "task2", 0.7, 0.8, 0.01);
        router.record_shadow_comparison("ShadowTest", "task3", 0.85, 0.85, 0.01);

        let stats = router.get_shadow_stats("ShadowTest").unwrap();
        assert_eq!(stats.candidate_wins, 1);
        assert_eq!(stats.production_wins, 1);
        assert_eq!(stats.ties, 1);
        assert_eq!(stats.total_samples(), 3);
    }

    #[test]
    fn test_shadow_win_rate() {
        let (mut router, _temp) = test_router();

        router.set_routing(
            "WinRateTest",
            RoutingStrategy::Shadow {
                candidate_id: "test".to_string(),
            },
        );

        // Candidate wins 6/10
        for i in 0..6 {
            router.record_shadow_comparison("WinRateTest", &format!("task{}", i), 0.9, 0.8, 0.01);
        }
        for i in 6..10 {
            router.record_shadow_comparison("WinRateTest", &format!("task{}", i), 0.7, 0.8, 0.01);
        }

        let stats = router.get_shadow_stats("WinRateTest").unwrap();
        assert_eq!(stats.candidate_win_rate(), 0.6);
        assert!(stats.should_promote(10, 0.5));
        assert!(!stats.should_promote(10, 0.7));
    }

    #[test]
    fn test_clear_routing() {
        let (mut router, _temp) = test_router();

        router.set_routing(
            "ClearTest",
            RoutingStrategy::Shadow {
                candidate_id: "test".to_string(),
            },
        );

        router.record_shadow_comparison("ClearTest", "task1", 0.9, 0.8, 0.01);

        assert!(router.get_shadow_stats("ClearTest").is_some());

        router.clear_routing("ClearTest");

        assert_eq!(router.get_routing("ClearTest"), RoutingStrategy::Promoted);
        assert!(router.get_shadow_stats("ClearTest").is_none());
    }

    #[test]
    fn test_routing_summary() {
        let (mut router, _temp) = test_router();

        router.set_routing("Sig1", RoutingStrategy::Promoted);
        router.set_routing(
            "Sig2",
            RoutingStrategy::Shadow {
                candidate_id: "test".to_string(),
            },
        );

        let summary = router.routing_summary();
        assert_eq!(summary.len(), 2);
        assert!(summary.contains_key("Sig1"));
        assert!(summary.contains_key("Sig2"));
    }

    #[test]
    fn test_get_module_with_promoted() {
        let (router, temp) = test_router();

        // Create a promoted module
        let hub = DspyHub::with_base_path(temp.path().to_path_buf());
        let manifest = CompiledModuleManifest::new("GetModuleTest", "GEPA")
            .with_instruction("Test")
            .with_promotion_state(PromotionState::Promoted)
            .with_scorecard(Scorecard::new(0.9))
            .finalize()
            .unwrap();

        hub.save_module(&manifest, &[]).unwrap();

        let result = router.get_module("GetModuleTest");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().signature_name, "GetModuleTest");
    }

    #[test]
    fn test_active_signatures() {
        let (mut router, _temp) = test_router();

        router.set_routing("Sig1", RoutingStrategy::Promoted);
        router.set_routing("Sig2", RoutingStrategy::Promoted);

        let active = router.active_signatures();
        assert_eq!(active.len(), 2);
    }
}
