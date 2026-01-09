//! Promotion gates for staged rollout of compiled modules.
//!
//! Provides a 4-stage promotion pipeline:
//! - Candidate → Staged: Pass proxy metrics
//! - Staged → Shadow: Pass truth metrics
//! - Shadow → Promoted: Win A/B comparison
//! - Promoted → RolledBack: On regression

use super::scoring::{ScorecardResult, Scorer};
use super::task::EvalTask;
use crate::core::Module;
use crate::manifest::CompiledModuleManifest;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// State of a compiled module in the promotion pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PromotionState {
    /// Newly compiled, untested.
    #[default]
    Candidate,
    /// Passed proxy metrics, running truth metrics.
    Staged,
    /// Running in shadow mode alongside production.
    Shadow,
    /// Promoted to production.
    Promoted,
    /// Rolled back after issues.
    RolledBack,
}

impl PromotionState {
    /// Get the next state in the pipeline.
    pub fn next(&self) -> Option<Self> {
        match self {
            Self::Candidate => Some(Self::Staged),
            Self::Staged => Some(Self::Shadow),
            Self::Shadow => Some(Self::Promoted),
            Self::Promoted => None,
            Self::RolledBack => None,
        }
    }

    /// Check if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Promoted | Self::RolledBack)
    }

    /// Get display name.
    pub fn display_name(&self) -> &str {
        match self {
            Self::Candidate => "Candidate",
            Self::Staged => "Staged",
            Self::Shadow => "Shadow",
            Self::Promoted => "Promoted",
            Self::RolledBack => "Rolled Back",
        }
    }
}

/// A gate definition for promotion.
#[derive(Debug, Clone)]
pub struct PromotionGate {
    /// Name of this gate.
    pub name: String,

    /// State to transition from.
    pub from_state: PromotionState,

    /// State to transition to.
    pub to_state: PromotionState,

    /// Requirements to pass this gate.
    pub requirements: Vec<GateRequirement>,
}

impl PromotionGate {
    /// Create a new gate.
    pub fn new(
        name: impl Into<String>,
        from: PromotionState,
        to: PromotionState,
    ) -> Self {
        Self {
            name: name.into(),
            from_state: from,
            to_state: to,
            requirements: Vec::new(),
        }
    }

    /// Add a requirement.
    pub fn with_requirement(mut self, req: GateRequirement) -> Self {
        self.requirements.push(req);
        self
    }

    /// Add multiple requirements.
    pub fn with_requirements(mut self, reqs: Vec<GateRequirement>) -> Self {
        self.requirements.extend(reqs);
        self
    }
}

/// Requirement for passing a promotion gate.
#[derive(Debug, Clone)]
pub enum GateRequirement {
    /// Minimum score on a specific metric.
    MinScore {
        metric: String,
        threshold: f64,
    },

    /// Must beat baseline by a margin.
    BeatBaseline {
        metric: String,
        margin: f64,
    },

    /// Shadow mode minimum duration.
    ShadowDuration(Duration),

    /// Shadow mode minimum sample size.
    ShadowSamples(usize),

    /// Shadow mode win rate threshold.
    ShadowWinRate(f64),

    /// Manual approval required.
    ManualApproval,

    /// Custom requirement with validator function name.
    Custom {
        name: String,
        description: String,
    },
}

impl GateRequirement {
    /// Create a minimum score requirement.
    pub fn min_score(metric: impl Into<String>, threshold: f64) -> Self {
        Self::MinScore {
            metric: metric.into(),
            threshold,
        }
    }

    /// Create a beat baseline requirement.
    pub fn beat_baseline(metric: impl Into<String>, margin: f64) -> Self {
        Self::BeatBaseline {
            metric: metric.into(),
            margin,
        }
    }

    /// Create a shadow duration requirement.
    pub fn shadow_duration(duration: Duration) -> Self {
        Self::ShadowDuration(duration)
    }

    /// Create a shadow samples requirement.
    pub fn shadow_samples(count: usize) -> Self {
        Self::ShadowSamples(count)
    }

    /// Create a shadow win rate requirement.
    pub fn shadow_win_rate(rate: f64) -> Self {
        Self::ShadowWinRate(rate.clamp(0.0, 1.0))
    }

    /// Get a human-readable description.
    pub fn description(&self) -> String {
        match self {
            Self::MinScore { metric, threshold } => {
                format!("{} >= {:.2}", metric, threshold)
            }
            Self::BeatBaseline { metric, margin } => {
                format!("{} beats baseline by {:.1}%", metric, margin * 100.0)
            }
            Self::ShadowDuration(d) => {
                format!("Shadow mode for {:?}", d)
            }
            Self::ShadowSamples(n) => {
                format!("At least {} shadow samples", n)
            }
            Self::ShadowWinRate(r) => {
                format!("Shadow win rate >= {:.1}%", r * 100.0)
            }
            Self::ManualApproval => "Manual approval".to_string(),
            Self::Custom { name, .. } => format!("Custom: {}", name),
        }
    }
}

/// Result of checking a gate requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementResult {
    /// The requirement that was checked.
    pub requirement: String,

    /// Whether it passed.
    pub passed: bool,

    /// Actual value (if applicable).
    pub actual_value: Option<f64>,

    /// Required value (if applicable).
    pub required_value: Option<f64>,

    /// Explanation.
    pub reason: String,
}

/// Result of a promotion attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionResult {
    /// Whether promotion succeeded.
    pub success: bool,

    /// The gate that was evaluated.
    pub gate_name: String,

    /// New state (if promoted).
    pub new_state: Option<PromotionState>,

    /// Results for each requirement.
    pub requirement_results: Vec<RequirementResult>,

    /// Overall reason.
    pub reason: String,
}

impl PromotionResult {
    /// Create a successful promotion result.
    pub fn success(gate_name: impl Into<String>, new_state: PromotionState) -> Self {
        Self {
            success: true,
            gate_name: gate_name.into(),
            new_state: Some(new_state),
            requirement_results: Vec::new(),
            reason: "All requirements met".into(),
        }
    }

    /// Create a failed promotion result.
    pub fn failure(gate_name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            success: false,
            gate_name: gate_name.into(),
            new_state: None,
            requirement_results: Vec::new(),
            reason: reason.into(),
        }
    }

    /// Add requirement results.
    pub fn with_results(mut self, results: Vec<RequirementResult>) -> Self {
        self.requirement_results = results;
        self
    }
}

/// Result of shadow mode comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowResult {
    /// Number of times candidate won.
    pub candidate_wins: usize,

    /// Number of times production won.
    pub production_wins: usize,

    /// Number of ties.
    pub ties: usize,

    /// Candidate's overall score.
    pub candidate_score: f64,

    /// Production's overall score.
    pub production_score: f64,

    /// Duration of shadow mode.
    pub duration: Duration,

    /// Per-task comparison results.
    pub per_task: Vec<ShadowTaskResult>,
}

impl ShadowResult {
    /// Calculate win rate for candidate.
    pub fn candidate_win_rate(&self) -> f64 {
        let total = self.candidate_wins + self.production_wins + self.ties;
        if total == 0 {
            0.5
        } else {
            self.candidate_wins as f64 / total as f64
        }
    }

    /// Calculate total samples.
    pub fn total_samples(&self) -> usize {
        self.candidate_wins + self.production_wins + self.ties
    }

    /// Check if candidate should be promoted.
    pub fn should_promote(&self, min_samples: usize, min_win_rate: f64) -> bool {
        self.total_samples() >= min_samples && self.candidate_win_rate() >= min_win_rate
    }
}

/// Result of shadow comparison for a single task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowTaskResult {
    /// Task ID.
    pub task_id: String,

    /// Candidate's score.
    pub candidate_score: f64,

    /// Production's score.
    pub production_score: f64,

    /// Winner (candidate, production, or tie).
    pub winner: ShadowWinner,
}

/// Winner of a shadow comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShadowWinner {
    Candidate,
    Production,
    Tie,
}

/// Promotion manager for managing the promotion pipeline.
pub struct PromotionManager {
    /// Gates in the pipeline.
    gates: Vec<PromotionGate>,

    /// Scorer for evaluation.
    scorer: Scorer,

    /// Margin for tie detection in shadow mode.
    tie_margin: f64,
}

impl PromotionManager {
    /// Create a new promotion manager with gates.
    pub fn with_gates(gates: Vec<PromotionGate>, scorer: Scorer) -> Self {
        Self {
            gates,
            scorer,
            tie_margin: 0.01,
        }
    }

    /// Set the tie margin for shadow comparisons.
    pub fn with_tie_margin(mut self, margin: f64) -> Self {
        self.tie_margin = margin;
        self
    }

    /// Attempt to promote a candidate.
    pub async fn try_promote<M: Module>(
        &self,
        candidate: &M,
        manifest: &CompiledModuleManifest,
        current_state: PromotionState,
        tasks: &[EvalTask],
    ) -> Result<PromotionResult> {
        // Find the gate for this transition
        let gate = match self.gates.iter().find(|g| g.from_state == current_state) {
            Some(g) => g,
            None => {
                return Ok(PromotionResult::failure(
                    "none",
                    format!("No gate defined for state {:?}", current_state),
                ))
            }
        };

        // Evaluate the candidate
        let scorecard = self.scorer.score(candidate, tasks).await?;

        // Check each requirement
        let mut requirement_results = Vec::new();
        let mut all_passed = true;

        for req in &gate.requirements {
            let result = self.check_requirement(req, &scorecard, manifest).await?;
            if !result.passed {
                all_passed = false;
            }
            requirement_results.push(result);
        }

        if all_passed {
            Ok(PromotionResult::success(&gate.name, gate.to_state).with_results(requirement_results))
        } else {
            let failed: Vec<_> = requirement_results
                .iter()
                .filter(|r| !r.passed)
                .map(|r| r.requirement.clone())
                .collect();
            Ok(PromotionResult::failure(
                &gate.name,
                format!("Failed requirements: {}", failed.join(", ")),
            )
            .with_results(requirement_results))
        }
    }

    /// Check a single requirement.
    async fn check_requirement(
        &self,
        req: &GateRequirement,
        scorecard: &ScorecardResult,
        _manifest: &CompiledModuleManifest,
    ) -> Result<RequirementResult> {
        match req {
            GateRequirement::MinScore { metric, threshold } => {
                let actual = scorecard.per_metric.get(metric).copied().unwrap_or(0.0);
                let passed = actual >= *threshold;
                Ok(RequirementResult {
                    requirement: req.description(),
                    passed,
                    actual_value: Some(actual),
                    required_value: Some(*threshold),
                    reason: if passed {
                        format!("{}: {:.2} >= {:.2}", metric, actual, threshold)
                    } else {
                        format!("{}: {:.2} < {:.2}", metric, actual, threshold)
                    },
                })
            }

            GateRequirement::BeatBaseline { metric, margin } => {
                let candidate_score = scorecard.per_metric.get(metric).copied().unwrap_or(0.0);
                // TODO: Get baseline score from stored baseline results
                let baseline_score = 0.5; // Placeholder
                let passed = candidate_score >= baseline_score + margin;

                Ok(RequirementResult {
                    requirement: req.description(),
                    passed,
                    actual_value: Some(candidate_score - baseline_score),
                    required_value: Some(*margin),
                    reason: if passed {
                        format!(
                            "Candidate {:.2} beats baseline {:.2} by {:.2}",
                            candidate_score,
                            baseline_score,
                            candidate_score - baseline_score
                        )
                    } else {
                        format!(
                            "Candidate {:.2} does not beat baseline {:.2} by required {:.2}",
                            candidate_score, baseline_score, margin
                        )
                    },
                })
            }

            GateRequirement::ShadowSamples(required) => {
                // TODO: Get actual shadow sample count from manifest/storage
                let actual = 0usize;
                let passed = actual >= *required;

                Ok(RequirementResult {
                    requirement: req.description(),
                    passed,
                    actual_value: Some(actual as f64),
                    required_value: Some(*required as f64),
                    reason: format!("Shadow samples: {} / {}", actual, required),
                })
            }

            GateRequirement::ShadowWinRate(required) => {
                // TODO: Get actual win rate from shadow results
                let actual = 0.0;
                let passed = actual >= *required;

                Ok(RequirementResult {
                    requirement: req.description(),
                    passed,
                    actual_value: Some(actual),
                    required_value: Some(*required),
                    reason: format!("Win rate: {:.1}% / {:.1}%", actual * 100.0, required * 100.0),
                })
            }

            GateRequirement::ShadowDuration(required) => {
                // TODO: Get actual duration from manifest
                let actual = Duration::from_secs(0);
                let passed = actual >= *required;

                Ok(RequirementResult {
                    requirement: req.description(),
                    passed,
                    actual_value: Some(actual.as_secs() as f64),
                    required_value: Some(required.as_secs() as f64),
                    reason: format!("Shadow duration: {:?} / {:?}", actual, required),
                })
            }

            GateRequirement::ManualApproval => Ok(RequirementResult {
                requirement: req.description(),
                passed: false, // Always requires explicit approval
                actual_value: None,
                required_value: None,
                reason: "Awaiting manual approval".into(),
            }),

            GateRequirement::Custom { name, description } => Ok(RequirementResult {
                requirement: req.description(),
                passed: false,
                actual_value: None,
                required_value: None,
                reason: format!("Custom requirement '{}' not implemented: {}", name, description),
            }),
        }
    }

    /// Run shadow mode comparison between candidate and production.
    pub async fn run_shadow<C: Module, P: Module>(
        &self,
        candidate: &C,
        production: &P,
        tasks: &[EvalTask],
    ) -> Result<ShadowResult> {
        let start = std::time::Instant::now();

        let candidate_scorecard = self.scorer.score(candidate, tasks).await?;
        let production_scorecard = self.scorer.score(production, tasks).await?;

        let mut result = ShadowResult {
            candidate_wins: 0,
            production_wins: 0,
            ties: 0,
            candidate_score: candidate_scorecard.overall_score,
            production_score: production_scorecard.overall_score,
            duration: start.elapsed(),
            per_task: Vec::new(),
        };

        // Compare per-task scores
        for task in tasks {
            let candidate_task = candidate_scorecard.per_task.get(&task.id);
            let production_task = production_scorecard.per_task.get(&task.id);

            if let (Some(c), Some(p)) = (candidate_task, production_task) {
                let diff = c.score - p.score;
                let winner = if diff > self.tie_margin {
                    result.candidate_wins += 1;
                    ShadowWinner::Candidate
                } else if diff < -self.tie_margin {
                    result.production_wins += 1;
                    ShadowWinner::Production
                } else {
                    result.ties += 1;
                    ShadowWinner::Tie
                };

                result.per_task.push(ShadowTaskResult {
                    task_id: task.id.clone(),
                    candidate_score: c.score,
                    production_score: p.score,
                    winner,
                });
            }
        }

        Ok(result)
    }

    /// Get default gates for the standard promotion pipeline.
    pub fn default_gates() -> Vec<PromotionGate> {
        vec![
            // Candidate → Staged: Pass proxy metrics
            PromotionGate::new("proxy_gate", PromotionState::Candidate, PromotionState::Staged)
                .with_requirements(vec![
                    GateRequirement::min_score("format", 0.95),
                    GateRequirement::min_score("syntax", 0.90),
                ]),
            // Staged → Shadow: Pass truth metrics
            PromotionGate::new("truth_gate", PromotionState::Staged, PromotionState::Shadow)
                .with_requirements(vec![
                    GateRequirement::min_score("llm_judge", 0.80),
                    GateRequirement::beat_baseline("overall", 0.02),
                ]),
            // Shadow → Promoted: Win A/B comparison
            PromotionGate::new("shadow_gate", PromotionState::Shadow, PromotionState::Promoted)
                .with_requirements(vec![
                    GateRequirement::shadow_samples(100),
                    GateRequirement::shadow_win_rate(0.52),
                ]),
        ]
    }
}

/// Record of an evaluation in the promotion history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalRecord {
    /// Timestamp of evaluation.
    pub timestamp: u64,

    /// State at time of evaluation.
    pub state: PromotionState,

    /// Scorecard from evaluation.
    pub scorecard: ScorecardResult,

    /// Whether promotion was attempted.
    pub promotion_attempted: bool,

    /// Result of promotion attempt (if any).
    pub promotion_success: Option<bool>,

    /// Reason for promotion result.
    pub promotion_reason: Option<String>,
}

impl EvalRecord {
    /// Create a new eval record.
    pub fn new(state: PromotionState, scorecard: ScorecardResult) -> Self {
        Self {
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            state,
            scorecard,
            promotion_attempted: false,
            promotion_success: None,
            promotion_reason: None,
        }
    }

    /// Record a promotion attempt.
    pub fn with_promotion_result(mut self, success: bool, reason: impl Into<String>) -> Self {
        self.promotion_attempted = true;
        self.promotion_success = Some(success);
        self.promotion_reason = Some(reason.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_promotion_state_transitions() {
        assert_eq!(PromotionState::Candidate.next(), Some(PromotionState::Staged));
        assert_eq!(PromotionState::Staged.next(), Some(PromotionState::Shadow));
        assert_eq!(PromotionState::Shadow.next(), Some(PromotionState::Promoted));
        assert_eq!(PromotionState::Promoted.next(), None);
        assert_eq!(PromotionState::RolledBack.next(), None);
    }

    #[test]
    fn test_promotion_state_terminal() {
        assert!(!PromotionState::Candidate.is_terminal());
        assert!(!PromotionState::Staged.is_terminal());
        assert!(!PromotionState::Shadow.is_terminal());
        assert!(PromotionState::Promoted.is_terminal());
        assert!(PromotionState::RolledBack.is_terminal());
    }

    #[test]
    fn test_gate_requirement_descriptions() {
        let req = GateRequirement::min_score("format", 0.95);
        assert!(req.description().contains("format"));
        assert!(req.description().contains("0.95"));

        let req = GateRequirement::shadow_win_rate(0.52);
        assert!(req.description().contains("52"));
    }

    #[test]
    fn test_shadow_result() {
        let result = ShadowResult {
            candidate_wins: 60,
            production_wins: 30,
            ties: 10,
            candidate_score: 0.85,
            production_score: 0.80,
            duration: Duration::from_secs(3600),
            per_task: vec![],
        };

        assert_eq!(result.total_samples(), 100);
        assert_eq!(result.candidate_win_rate(), 0.6);
        assert!(result.should_promote(100, 0.52));
        assert!(!result.should_promote(100, 0.65));
    }

    #[test]
    fn test_default_gates() {
        let gates = PromotionManager::default_gates();
        assert_eq!(gates.len(), 3);
        assert_eq!(gates[0].from_state, PromotionState::Candidate);
        assert_eq!(gates[0].to_state, PromotionState::Staged);
        assert_eq!(gates[1].from_state, PromotionState::Staged);
        assert_eq!(gates[2].from_state, PromotionState::Shadow);
    }

    #[test]
    fn test_promotion_result() {
        let success = PromotionResult::success("proxy_gate", PromotionState::Staged);
        assert!(success.success);
        assert_eq!(success.new_state, Some(PromotionState::Staged));

        let failure = PromotionResult::failure("proxy_gate", "format score too low");
        assert!(!failure.success);
        assert_eq!(failure.new_state, None);
    }
}
