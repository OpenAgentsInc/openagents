//! DSPy security signatures and helpers for skill execution.

use dsrs::{example, Predict, Prediction, Predictor, Signature, GLOBAL_SETTINGS};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn from_label(label: &str) -> Self {
        match label.to_lowercase().as_str() {
            "critical" => Self::Critical,
            "high" => Self::High,
            "medium" => Self::Medium,
            "low" => Self::Low,
            _ => Self::Medium,
        }
    }

    pub fn requires_approval(&self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSecurityDecision {
    pub risk_level: RiskLevel,
    pub concerns: Vec<String>,
    pub recommended_sandbox: String,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDecision {
    pub allowed: bool,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimitDecision {
    pub approved_limits: String,
    pub adjustments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafePathDecision {
    pub safe: bool,
    pub reason: String,
}

#[Signature]
pub struct SkillSecurityClassifier {
    /// Classify security risk of a skill execution.

    /// Skill manifest payload
    #[input]
    pub skill_manifest: String,

    /// Requested permissions (filesystem/network/limits)
    #[input]
    pub requested_permissions: String,

    /// Execution context (paths, runtime, caller)
    #[input]
    pub execution_context: String,

    /// Risk level: Low/Medium/High/Critical
    #[output]
    pub risk_level: String,

    /// Concerns (JSON array)
    #[output]
    pub concerns: String,

    /// Recommended sandbox profile
    #[output]
    pub recommended_sandbox: String,

    /// Whether human approval is required
    #[output]
    pub requires_approval: bool,
}

#[Signature]
pub struct FilesystemPermissionSignature {
    /// Learn safe filesystem permissions for skill types.

    /// Skill type or ID
    #[input]
    pub skill_type: String,

    /// Requested paths (JSON array)
    #[input]
    pub requested_paths: String,

    /// Operation (Read/Write/Execute)
    #[input]
    pub operation: String,

    /// Whether the request is allowed
    #[output]
    pub allowed: bool,

    /// Rationale for the decision
    #[output]
    pub reasoning: String,
}

#[Signature]
pub struct ResourceLimitSignature {
    /// Learn appropriate resource limits for skills.

    /// Skill type or ID
    #[input]
    pub skill_type: String,

    /// Requested limits (JSON)
    #[input]
    pub requested_limits: String,

    /// Approved limits (JSON)
    #[output]
    pub approved_limits: String,

    /// Adjustments or rationale
    #[output]
    pub adjustments: String,
}

#[Signature]
pub struct SafePathValidationSignature {
    /// Learn path safety patterns.

    /// Path to validate
    #[input]
    pub path: String,

    /// Whether the path is safe
    #[output]
    pub safe: bool,

    /// Reason for decision
    #[output]
    pub reason: String,
}

fn dspy_ready() -> bool {
    GLOBAL_SETTINGS.read().unwrap().is_some()
}

fn run_prediction<F>(future: F) -> Option<Prediction>
where
    F: Future<Output = std::result::Result<Prediction, anyhow::Error>>,
{
    if !dspy_ready() {
        return None;
    }

    let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
        catch_unwind(AssertUnwindSafe(|| {
            tokio::task::block_in_place(|| handle.block_on(future))
        }))
    } else if let Ok(runtime) = tokio::runtime::Runtime::new() {
        catch_unwind(AssertUnwindSafe(|| runtime.block_on(future)))
    } else {
        return None;
    };

    match result {
        Ok(Ok(prediction)) => Some(prediction),
        _ => None,
    }
}

fn get_string(prediction: &Prediction, key: &str) -> String {
    let val = prediction.get(key, None);
    if let Some(s) = val.as_str() {
        s.to_string()
    } else {
        val.to_string().trim_matches('"').to_string()
    }
}

fn get_bool(prediction: &Prediction, key: &str) -> bool {
    let val = prediction.get(key, None);
    if let Some(b) = val.as_bool() {
        b
    } else if let Some(s) = val.as_str() {
        matches!(s.to_lowercase().as_str(), "true" | "yes" | "1")
    } else {
        false
    }
}

fn parse_json_array(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw).unwrap_or_else(|_| {
        raw.lines()
            .map(|l| l.trim().trim_start_matches('-').trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    })
}

pub fn classify_skill_security(
    skill_manifest: &str,
    requested_permissions: &str,
    execution_context: &str,
) -> Option<SkillSecurityDecision> {
    let classifier = Predict::new(SkillSecurityClassifier::new());
    let example = example! {
        "skill_manifest": "input" => skill_manifest.to_string(),
        "requested_permissions": "input" => requested_permissions.to_string(),
        "execution_context": "input" => execution_context.to_string(),
    };

    let prediction = run_prediction(classifier.forward(example))?;
    let risk_level = RiskLevel::from_label(&get_string(&prediction, "risk_level"));
    let concerns_raw = get_string(&prediction, "concerns");

    Some(SkillSecurityDecision {
        risk_level,
        concerns: parse_json_array(&concerns_raw),
        recommended_sandbox: get_string(&prediction, "recommended_sandbox"),
        requires_approval: get_bool(&prediction, "requires_approval"),
    })
}

pub fn classify_filesystem_permission(
    skill_type: &str,
    requested_paths: &str,
    operation: &str,
) -> Option<PermissionDecision> {
    let classifier = Predict::new(FilesystemPermissionSignature::new());
    let example = example! {
        "skill_type": "input" => skill_type.to_string(),
        "requested_paths": "input" => requested_paths.to_string(),
        "operation": "input" => operation.to_string(),
    };

    let prediction = run_prediction(classifier.forward(example))?;
    Some(PermissionDecision {
        allowed: get_bool(&prediction, "allowed"),
        reasoning: get_string(&prediction, "reasoning"),
    })
}

pub fn classify_resource_limits(
    skill_type: &str,
    requested_limits: &str,
) -> Option<ResourceLimitDecision> {
    let classifier = Predict::new(ResourceLimitSignature::new());
    let example = example! {
        "skill_type": "input" => skill_type.to_string(),
        "requested_limits": "input" => requested_limits.to_string(),
    };

    let prediction = run_prediction(classifier.forward(example))?;
    Some(ResourceLimitDecision {
        approved_limits: get_string(&prediction, "approved_limits"),
        adjustments: get_string(&prediction, "adjustments"),
    })
}

pub fn classify_safe_path(path: &str) -> Option<SafePathDecision> {
    let classifier = Predict::new(SafePathValidationSignature::new());
    let example = example! {
        "path": "input" => path.to_string(),
    };

    let prediction = run_prediction(classifier.forward(example))?;
    Some(SafePathDecision {
        safe: get_bool(&prediction, "safe"),
        reason: get_string(&prediction, "reason"),
    })
}
