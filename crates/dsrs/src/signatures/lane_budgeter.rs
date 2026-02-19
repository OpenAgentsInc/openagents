//! Lane Budgeter Signature.
//!
//! Allocates budget across different execution lanes.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;

/// Execution lane types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecutionLane {
    /// Local execution (free, potentially slow).
    Local,

    /// Swarm execution (cheap, distributed).
    Swarm,

    /// Premium cloud execution (expensive, fast).
    Datacenter,

    /// Cached/precomputed (free, instant).
    Cache,
}

impl std::fmt::Display for ExecutionLane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionLane::Local => write!(f, "local"),
            ExecutionLane::Swarm => write!(f, "swarm"),
            ExecutionLane::Datacenter => write!(f, "datacenter"),
            ExecutionLane::Cache => write!(f, "cache"),
        }
    }
}

/// Budget allocation for a lane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaneAllocation {
    /// The execution lane.
    pub lane: ExecutionLane,

    /// Allocated budget in millisatoshis.
    pub budget_msats: u64,

    /// Maximum number of requests.
    pub max_requests: Option<u32>,

    /// Priority (lower = higher priority).
    pub priority: u32,
}

/// Signature for allocating budget across execution lanes.
///
/// # Inputs
/// - `task_complexity`: Estimated complexity (low, medium, high)
/// - `budget_remaining`: Total remaining budget in millisatoshis
/// - `available_lanes`: List of available execution lanes
/// - `task_type`: Type of task (inference, retrieval, sandbox)
///
/// # Outputs
/// - `allocations`: Budget allocation per lane
/// - `rationale`: Explanation of allocation strategy
/// - `fallback_order`: Order of lanes for fallback
#[derive(Debug, Clone)]
pub struct LaneBudgeterSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for LaneBudgeterSignature {
    fn default() -> Self {
        Self {
            instruction:
                r#"You are an expert at optimizing AI compute costs. Given a task and budget,
allocate resources across available execution lanes.

Available lanes and characteristics:
- Local: Free, uses user's machine. Good for: small tasks, privacy-sensitive data.
- Swarm: ~1-10 msats/call. Distributed inference via OpenAgents network.
- Datacenter: ~100-1000 msats/call. Premium providers (Codex, GPT-4).
- Cache: Free, instant. Use when results are precomputed.

Consider:
1. Task complexity and required quality
2. Budget constraints
3. Latency requirements
4. Privacy/security requirements
5. Fallback strategies if primary lane fails

Output budget allocations with rationale and fallback order."#
                    .to_string(),
            demos: vec![],
        }
    }
}

impl LaneBudgeterSignature {
    /// Create a new lane budgeter signature.
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

impl MetaSignature for LaneBudgeterSignature {
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
            "task_complexity": {
                "type": "String",
                "desc": "Estimated complexity: low, medium, or high",
                "__dsrs_field_type": "input"
            },
            "budget_remaining": {
                "type": "u64",
                "desc": "Total remaining budget in millisatoshis",
                "__dsrs_field_type": "input"
            },
            "available_lanes": {
                "type": "Vec<String>",
                "desc": "List of available execution lanes",
                "__dsrs_field_type": "input"
            },
            "task_type": {
                "type": "String",
                "desc": "Type of task: inference, retrieval, or sandbox",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "allocations": {
                "type": "Vec<LaneAllocation>",
                "desc": "Budget allocation per lane",
                "__dsrs_field_type": "output"
            },
            "rationale": {
                "type": "String",
                "desc": "Explanation of allocation strategy",
                "__dsrs_field_type": "output"
            },
            "fallback_order": {
                "type": "Vec<String>",
                "desc": "Order of lanes for fallback (most to least preferred)",
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

/// Helper to create default allocations.
pub fn default_allocations(budget_msats: u64) -> HashMap<ExecutionLane, u64> {
    let mut allocations = HashMap::new();

    // Default split: 60% swarm, 30% datacenter, 10% reserve
    allocations.insert(ExecutionLane::Swarm, (budget_msats * 60) / 100);
    allocations.insert(ExecutionLane::Datacenter, (budget_msats * 30) / 100);
    allocations.insert(ExecutionLane::Local, 0); // Free
    allocations.insert(ExecutionLane::Cache, 0); // Free

    allocations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lane_budgeter_signature() {
        let sig = LaneBudgeterSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("task_complexity").is_some());
        assert!(inputs.get("budget_remaining").is_some());
        assert!(inputs.get("available_lanes").is_some());
        assert!(inputs.get("task_type").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("allocations").is_some());
        assert!(outputs.get("rationale").is_some());
        assert!(outputs.get("fallback_order").is_some());
    }

    #[test]
    fn test_execution_lane_display() {
        assert_eq!(ExecutionLane::Local.to_string(), "local");
        assert_eq!(ExecutionLane::Swarm.to_string(), "swarm");
        assert_eq!(ExecutionLane::Datacenter.to_string(), "datacenter");
        assert_eq!(ExecutionLane::Cache.to_string(), "cache");
    }

    #[test]
    fn test_default_allocations() {
        let allocations = default_allocations(10000);

        assert_eq!(allocations.get(&ExecutionLane::Swarm), Some(&6000));
        assert_eq!(allocations.get(&ExecutionLane::Datacenter), Some(&3000));
        assert_eq!(allocations.get(&ExecutionLane::Local), Some(&0));
    }
}
