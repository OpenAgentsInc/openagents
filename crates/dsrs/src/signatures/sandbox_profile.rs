//! Sandbox Profile Selection Signature.
//!
//! Selects appropriate sandbox resources for command execution.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Sandbox resource profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SandboxProfile {
    /// 1 vCPU, 1GB RAM, 5GB disk, 60s timeout.
    Small,

    /// 2 vCPUs, 4GB RAM, 8GB disk, 120s timeout.
    #[default]
    Medium,

    /// 4 vCPUs, 8GB RAM, 10GB disk, 300s timeout.
    Large,

    /// Custom profile.
    Custom {
        vcpus: u32,
        memory_mb: u32,
        disk_mb: u32,
        timeout_secs: u32,
    },
}

impl std::fmt::Display for SandboxProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SandboxProfile::Small => write!(f, "small"),
            SandboxProfile::Medium => write!(f, "medium"),
            SandboxProfile::Large => write!(f, "large"),
            SandboxProfile::Custom { .. } => write!(f, "custom"),
        }
    }
}

/// Signature for selecting sandbox profile based on task requirements.
///
/// # Inputs
/// - `commands`: Commands to execute
/// - `repo_size`: Repository size in bytes
/// - `previous_failures`: Previous sandbox failures (if any)
///
/// # Outputs
/// - `profile`: Recommended sandbox profile
/// - `timeout`: Custom timeout override (if needed)
/// - `rationale`: Explanation of the selection
/// - `estimated_cost`: Estimated cost in millisatoshis
#[derive(Debug, Clone)]
pub struct SandboxProfileSelectionSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for SandboxProfileSelectionSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are an expert at resource allocation for sandbox execution. Given the commands
to execute and context about the repository, select the appropriate sandbox profile.

Available profiles:
- Small: 1 vCPU, 1GB RAM, 5GB disk, 60s timeout. Best for: quick tests, linting, simple builds.
- Medium: 2 vCPUs, 4GB RAM, 8GB disk, 120s timeout. Best for: typical builds, test suites.
- Large: 4 vCPUs, 8GB RAM, 10GB disk, 300s timeout. Best for: large projects, parallel tests, complex builds.

Consider:
1. What commands are being run (cargo build vs cargo test --all-features)?
2. Repository size and complexity
3. Previous failures (OOM? timeout?)
4. Cost vs reliability tradeoff

Output the recommended profile with rationale. Avoid over-provisioning."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl SandboxProfileSelectionSignature {
    /// Create a new sandbox profile selection signature.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set custom instruction.
    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = instruction.into();
        self
    }

    /// Add a demonstration example.
    pub fn with_demo(mut self, demo: Example) -> Self {
        self.demos.push(demo);
        self
    }
}

impl MetaSignature for SandboxProfileSelectionSignature {
    fn demos(&self) -> Vec<Example> {
        self.demos.clone()
    }

    fn set_demos(&mut self, demos: Vec<Example>) -> Result<()> {
        self.demos = demos;
        Ok(())
    }

    fn instruction(&self) -> String {
        self.instruction.clone()
    }

    fn input_fields(&self) -> Value {
        json!({
            "commands": {
                "type": "Vec<String>",
                "desc": "Commands to execute in the sandbox",
                "__dsrs_field_type": "input"
            },
            "repo_size": {
                "type": "u64",
                "desc": "Repository size in bytes",
                "__dsrs_field_type": "input"
            },
            "previous_failures": {
                "type": "Vec<String>",
                "desc": "Previous sandbox failures (error messages)",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "profile": {
                "type": "SandboxProfile",
                "desc": "Recommended sandbox profile (Small, Medium, Large)",
                "__dsrs_field_type": "output"
            },
            "timeout": {
                "type": "u32",
                "desc": "Custom timeout override in seconds (optional)",
                "__dsrs_field_type": "output"
            },
            "rationale": {
                "type": "String",
                "desc": "Explanation of the selection",
                "__dsrs_field_type": "output"
            },
            "estimated_cost": {
                "type": "u64",
                "desc": "Estimated cost in millisatoshis",
                "__dsrs_field_type": "output"
            }
        })
    }

    fn update_instruction(&mut self, instruction: String) -> Result<()> {
        self.instruction = instruction;
        Ok(())
    }

    fn append(&mut self, _name: &str, _value: Value) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_profile_selection_signature() {
        let sig = SandboxProfileSelectionSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("commands").is_some());
        assert!(inputs.get("repo_size").is_some());
        assert!(inputs.get("previous_failures").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("profile").is_some());
        assert!(outputs.get("timeout").is_some());
        assert!(outputs.get("rationale").is_some());
        assert!(outputs.get("estimated_cost").is_some());
    }

    #[test]
    fn test_sandbox_profile_display() {
        assert_eq!(SandboxProfile::Small.to_string(), "small");
        assert_eq!(SandboxProfile::Medium.to_string(), "medium");
        assert_eq!(SandboxProfile::Large.to_string(), "large");
    }

    #[test]
    fn test_custom_profile() {
        let profile = SandboxProfile::Custom {
            vcpus: 8,
            memory_mb: 16384,
            disk_mb: 20480,
            timeout_secs: 600,
        };

        assert_eq!(profile.to_string(), "custom");
    }
}
