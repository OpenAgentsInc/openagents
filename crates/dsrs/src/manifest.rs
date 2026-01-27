//! Compiled module manifest for versioned, hashable module artifacts.
//!
//! Every compiled DSPy module produces a manifest that captures:
//! - The signature being optimized
//! - Which optimizer was used
//! - Training data reference
//! - Performance metrics (scorecard)
//! - Runtime compatibility requirements

use crate::evaluate::promotion::{EvalRecord, PromotionState};
use protocol::canonical_hash;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Privacy mode requirements for a compiled module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum PrivacyMode {
    /// Public data is acceptable.
    #[default]
    PublicOk,
    /// No personally identifiable information.
    NoPii,
    /// Private repo data must be redacted.
    PrivateRepoRedacted,
    /// Private repo data is allowed.
    PrivateRepoAllowed,
}


/// Compatibility requirements for running a compiled module.
///
/// These are checked at runtime to ensure the execution environment
/// can satisfy the module's needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Compatibility {
    /// Required tools (e.g., "ripgrep", "node", "pytest").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_tools: Vec<String>,

    /// Required retrieval lanes (e.g., "lsp", "semantic", "keyword").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_lanes: Vec<String>,

    /// Allowed privacy modes for input data.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub privacy_modes_allowed: Vec<PrivacyMode>,

    /// Minimum provider reputation score (0.0 to 1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_provider_reputation: Option<f32>,
}

impl Compatibility {
    /// Create compatibility requirements with specific tools.
    pub fn with_tools(tools: Vec<String>) -> Self {
        Self {
            required_tools: tools,
            ..Default::default()
        }
    }

    /// Add required retrieval lanes.
    pub fn with_lanes(mut self, lanes: Vec<String>) -> Self {
        self.required_lanes = lanes;
        self
    }

    /// Set allowed privacy modes.
    pub fn with_privacy(mut self, modes: Vec<PrivacyMode>) -> Self {
        self.privacy_modes_allowed = modes;
        self
    }

    /// Set minimum provider reputation.
    pub fn with_min_reputation(mut self, min: f32) -> Self {
        self.min_provider_reputation = Some(min);
        self
    }

    /// Check if the given environment satisfies these requirements.
    pub fn check(
        &self,
        available_tools: &[String],
        available_lanes: &[String],
    ) -> CompatibilityResult {
        let mut missing_tools = Vec::new();
        let mut missing_lanes = Vec::new();

        for tool in &self.required_tools {
            if !available_tools.contains(tool) {
                missing_tools.push(tool.clone());
            }
        }

        for lane in &self.required_lanes {
            if !available_lanes.contains(lane) {
                missing_lanes.push(lane.clone());
            }
        }

        if missing_tools.is_empty() && missing_lanes.is_empty() {
            CompatibilityResult::Compatible
        } else {
            CompatibilityResult::Incompatible {
                missing_tools,
                missing_lanes,
            }
        }
    }
}

/// Result of compatibility checking.
#[derive(Debug, Clone, PartialEq)]
pub enum CompatibilityResult {
    /// All requirements are satisfied.
    Compatible,
    /// Some requirements are not satisfied.
    Incompatible {
        missing_tools: Vec<String>,
        missing_lanes: Vec<String>,
    },
}

impl CompatibilityResult {
    /// Check if compatible.
    pub fn is_compatible(&self) -> bool {
        matches!(self, Self::Compatible)
    }
}

/// Performance metrics from optimization.
///
/// Captures both proxy metrics (computed during optimization) and
/// truth metrics (computed on held-out data).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Scorecard {
    /// Proxy metrics used during optimization (e.g., retrieval_recall@k).
    #[serde(default)]
    pub proxy_metrics: HashMap<String, f32>,

    /// Ground truth metrics on validation set (e.g., tests_pass).
    #[serde(default)]
    pub truth_metrics: HashMap<String, f32>,

    /// Number of rollouts used to compute metrics.
    #[serde(default)]
    pub rollouts: usize,

    /// Median score across rollouts.
    #[serde(default)]
    pub median_score: f32,

    /// Failure rate across rollouts (0.0 to 1.0).
    #[serde(default)]
    pub p_fail: f32,
}

impl Scorecard {
    /// Create a new scorecard with median score.
    pub fn new(median_score: f32) -> Self {
        Self {
            median_score,
            ..Default::default()
        }
    }

    /// Add a proxy metric.
    pub fn with_proxy(mut self, name: impl Into<String>, value: f32) -> Self {
        self.proxy_metrics.insert(name.into(), value);
        self
    }

    /// Add a truth metric.
    pub fn with_truth(mut self, name: impl Into<String>, value: f32) -> Self {
        self.truth_metrics.insert(name.into(), value);
        self
    }

    /// Set rollout count.
    pub fn with_rollouts(mut self, n: usize) -> Self {
        self.rollouts = n;
        self
    }

    /// Set failure rate.
    pub fn with_p_fail(mut self, p: f32) -> Self {
        self.p_fail = p;
        self
    }
}

/// Manifest for a compiled DSPy module.
///
/// Every optimized module produces a manifest that:
/// - Identifies the module via `compiled_id` (SHA-256 hash)
/// - Records which optimizer produced it
/// - Links to training data
/// - Captures performance metrics
/// - Specifies runtime requirements
///
/// # Example
///
/// ```ignore
/// use dsrs::manifest::{CompiledModuleManifest, Scorecard, Compatibility};
///
/// let manifest = CompiledModuleManifest::new("PlanningSignature", "GEPA")
///     .with_trainset_id("abc123...")
///     .with_scorecard(Scorecard::new(0.85).with_rollouts(10))
///     .with_compatibility(Compatibility::with_tools(vec!["ripgrep".into()]));
///
/// // Compute deterministic hash
/// let compiled_id = manifest.compute_compiled_id().unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledModuleManifest {
    /// Name of the signature being optimized.
    pub signature_name: String,

    /// Deterministic hash of the optimized artifact.
    /// Computed from signature + instruction + demos.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiled_id: Option<String>,

    /// Optimizer used (e.g., "GEPA", "MIPROv2", "COPRO").
    pub optimizer: String,

    /// Hash of the training data used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trainset_id: Option<String>,

    /// Performance metrics from optimization.
    #[serde(default)]
    pub scorecard: Scorecard,

    /// Runtime compatibility requirements.
    #[serde(default)]
    pub compatibility: Compatibility,

    /// Unix timestamp when compiled.
    pub created_at: u64,

    /// Optimized instruction (the prompt).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,

    /// Number of demonstrations.
    #[serde(default)]
    pub demo_count: usize,

    /// Current promotion state.
    #[serde(default)]
    pub promotion_state: PromotionState,

    /// Evaluation history.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub eval_history: Vec<EvalRecord>,
}

impl CompiledModuleManifest {
    /// Create a new manifest for a signature and optimizer.
    pub fn new(signature_name: impl Into<String>, optimizer: impl Into<String>) -> Self {
        Self {
            signature_name: signature_name.into(),
            compiled_id: None,
            optimizer: optimizer.into(),
            trainset_id: None,
            scorecard: Scorecard::default(),
            compatibility: Compatibility::default(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            instruction: None,
            demo_count: 0,
            promotion_state: PromotionState::Candidate,
            eval_history: Vec::new(),
        }
    }

    /// Set promotion state.
    pub fn with_promotion_state(mut self, state: PromotionState) -> Self {
        self.promotion_state = state;
        self
    }

    /// Add an evaluation record.
    pub fn with_eval_record(mut self, record: EvalRecord) -> Self {
        self.eval_history.push(record);
        self
    }

    /// Set the training set ID.
    pub fn with_trainset_id(mut self, id: impl Into<String>) -> Self {
        self.trainset_id = Some(id.into());
        self
    }

    /// Set the scorecard.
    pub fn with_scorecard(mut self, scorecard: Scorecard) -> Self {
        self.scorecard = scorecard;
        self
    }

    /// Set compatibility requirements.
    pub fn with_compatibility(mut self, compat: Compatibility) -> Self {
        self.compatibility = compat;
        self
    }

    /// Set the optimized instruction.
    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = Some(instruction.into());
        self
    }

    /// Set demonstration count.
    pub fn with_demo_count(mut self, count: usize) -> Self {
        self.demo_count = count;
        self
    }

    /// Compute the deterministic compiled_id hash.
    ///
    /// The hash is computed from:
    /// - signature_name
    /// - optimizer
    /// - instruction (if present)
    /// - demo_count
    pub fn compute_compiled_id(&self) -> Result<String, protocol::HashError> {
        #[derive(Serialize)]
        struct HashInput<'a> {
            signature_name: &'a str,
            optimizer: &'a str,
            instruction: Option<&'a str>,
            demo_count: usize,
        }

        let input = HashInput {
            signature_name: &self.signature_name,
            optimizer: &self.optimizer,
            instruction: self.instruction.as_deref(),
            demo_count: self.demo_count,
        };

        canonical_hash(&input)
    }

    /// Finalize the manifest by computing the compiled_id.
    pub fn finalize(mut self) -> Result<Self, protocol::HashError> {
        self.compiled_id = Some(self.compute_compiled_id()?);
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_creation() {
        let manifest = CompiledModuleManifest::new("TestSignature", "GEPA")
            .with_trainset_id("train123")
            .with_scorecard(Scorecard::new(0.9).with_rollouts(5))
            .with_instruction("You are a helpful assistant.");

        assert_eq!(manifest.signature_name, "TestSignature");
        assert_eq!(manifest.optimizer, "GEPA");
        assert!(manifest.trainset_id.is_some());
    }

    #[test]
    fn test_compiled_id_determinism() {
        let manifest1 = CompiledModuleManifest::new("Sig", "GEPA")
            .with_instruction("Test instruction")
            .with_demo_count(3);

        let manifest2 = CompiledModuleManifest::new("Sig", "GEPA")
            .with_instruction("Test instruction")
            .with_demo_count(3);

        let id1 = manifest1.compute_compiled_id().unwrap();
        let id2 = manifest2.compute_compiled_id().unwrap();

        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn test_compiled_id_differs_with_changes() {
        let manifest1 =
            CompiledModuleManifest::new("Sig", "GEPA").with_instruction("Instruction A");

        let manifest2 =
            CompiledModuleManifest::new("Sig", "GEPA").with_instruction("Instruction B");

        let id1 = manifest1.compute_compiled_id().unwrap();
        let id2 = manifest2.compute_compiled_id().unwrap();

        assert_ne!(id1, id2);
    }

    #[test]
    fn test_compatibility_check() {
        let compat = Compatibility::with_tools(vec!["ripgrep".into(), "node".into()])
            .with_lanes(vec!["semantic".into()]);

        // All requirements met
        let result = compat.check(
            &["ripgrep".into(), "node".into(), "git".into()],
            &["semantic".into(), "keyword".into()],
        );
        assert!(result.is_compatible());

        // Missing tool
        let result = compat.check(&["ripgrep".into()], &["semantic".into()]);
        match result {
            CompatibilityResult::Incompatible { missing_tools, .. } => {
                assert!(missing_tools.contains(&"node".to_string()));
            }
            _ => panic!("Expected incompatible"),
        }
    }

    #[test]
    fn test_scorecard_builder() {
        let scorecard = Scorecard::new(0.85)
            .with_proxy("retrieval_recall@10", 0.9)
            .with_truth("tests_pass", 1.0)
            .with_rollouts(10)
            .with_p_fail(0.1);

        assert_eq!(scorecard.median_score, 0.85);
        assert_eq!(
            scorecard.proxy_metrics.get("retrieval_recall@10"),
            Some(&0.9)
        );
        assert_eq!(scorecard.truth_metrics.get("tests_pass"), Some(&1.0));
        assert_eq!(scorecard.rollouts, 10);
        assert_eq!(scorecard.p_fail, 0.1);
    }

    #[test]
    fn test_manifest_serde() {
        let manifest = CompiledModuleManifest::new("TestSig", "MIPROv2")
            .with_scorecard(Scorecard::new(0.8))
            .finalize()
            .unwrap();

        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: CompiledModuleManifest = serde_json::from_str(&json).unwrap();

        assert_eq!(manifest.signature_name, parsed.signature_name);
        assert_eq!(manifest.compiled_id, parsed.compiled_id);
    }

    #[test]
    fn test_privacy_mode_serde() {
        let mode = PrivacyMode::NoPii;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"no_pii\"");

        let parsed: PrivacyMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, parsed);
    }
}
