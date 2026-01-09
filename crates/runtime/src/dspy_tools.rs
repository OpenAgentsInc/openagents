//! DSPy Tool Invocation Signatures (Wave 10).
//!
//! Universal tool selection and interpretation layer.
//! These signatures enable learned, optimizable tool invocation across all agents.
//!
//! # Signatures
//!
//! - [`ToolSelectionSignature`] - Choose the right tool for any task
//! - [`ToolResultInterpretationSignature`] - Understand what a tool result means
//! - [`ToolChainPlanningSignature`] - Plan multi-tool sequences for complex tasks

use anyhow::Result;
use dsrs::core::signature::MetaSignature;
use dsrs::data::example::Example;
use serde_json::{json, Value};

// ============================================================================
// ToolSuccess Enum
// ============================================================================

/// Tool execution success status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolSuccess {
    /// Tool completed successfully with expected output.
    Yes,
    /// Tool completed but with incomplete or unexpected output.
    Partial,
    /// Tool failed or produced no useful output.
    No,
}

impl std::fmt::Display for ToolSuccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolSuccess::Yes => write!(f, "YES"),
            ToolSuccess::Partial => write!(f, "PARTIAL"),
            ToolSuccess::No => write!(f, "NO"),
        }
    }
}

impl std::str::FromStr for ToolSuccess {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "YES" | "SUCCESS" | "OK" => Ok(ToolSuccess::Yes),
            "PARTIAL" | "INCOMPLETE" => Ok(ToolSuccess::Partial),
            "NO" | "FAIL" | "FAILED" | "ERROR" => Ok(ToolSuccess::No),
            _ => Err(format!("Unknown tool success status: {}", s)),
        }
    }
}

// ============================================================================
// ToolSelectionSignature
// ============================================================================

/// Tool Selection Signature.
///
/// Chooses the right tool for any task from available options.
/// This replaces hard-coded tool selection with a learnable approach.
///
/// # Inputs
/// - `task_description`: What needs to be done
/// - `available_tools`: JSON array of tool definitions
/// - `context`: Recent tool results and conversation context
///
/// # Outputs
/// - `selected_tool`: Name of the tool to use
/// - `tool_params`: JSON parameters to pass to the tool
/// - `expected_outcome`: What the tool should produce
/// - `fallback_tool`: Alternative tool if primary fails
#[derive(Debug, Clone)]
pub struct ToolSelectionSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for ToolSelectionSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are a tool selection expert. Choose the best tool to accomplish the task.

Available tools are provided as JSON with name, description, and parameters.

Selection rules:
1. Match task requirements to tool capabilities
2. Prefer simpler tools when multiple can work
3. Consider tool cost and latency
4. Always provide a fallback option
5. Validate that required parameters are available

Think about:
- What does the task actually need?
- Which tool's output format matches requirements?
- Are there any constraints (time, cost, permissions)?
- What if the primary tool fails?

Provide clear reasoning for your selection."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl ToolSelectionSignature {
    /// Create a new tool selection signature.
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

impl MetaSignature for ToolSelectionSignature {
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
            "task_description": {
                "type": "String",
                "desc": "What needs to be done - the user's request or task",
                "__dsrs_field_type": "input"
            },
            "available_tools": {
                "type": "String",
                "desc": "JSON array of tool definitions with name, description, and parameters",
                "__dsrs_field_type": "input"
            },
            "context": {
                "type": "String",
                "desc": "Recent tool results and conversation context",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "selected_tool": {
                "type": "String",
                "desc": "Name of the tool to use",
                "__dsrs_field_type": "output"
            },
            "tool_params": {
                "type": "String",
                "desc": "JSON object with parameters to pass to the tool",
                "__dsrs_field_type": "output"
            },
            "expected_outcome": {
                "type": "String",
                "desc": "What the tool should produce",
                "__dsrs_field_type": "output"
            },
            "fallback_tool": {
                "type": "String",
                "desc": "Alternative tool name if primary fails",
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

// ============================================================================
// ToolResultInterpretationSignature
// ============================================================================

/// Tool Result Interpretation Signature.
///
/// Understands what a tool result means and determines next steps.
/// Critical for error handling and multi-step workflows.
///
/// # Inputs
/// - `tool_name`: Which tool was called
/// - `tool_output`: Raw output from the tool (stdout, stderr, result)
/// - `original_intent`: What we were trying to accomplish
///
/// # Outputs
/// - `success`: YES, PARTIAL, or NO
/// - `extracted_info`: Key information extracted from the output
/// - `next_steps`: What to do next based on the result
/// - `error_analysis`: If failed, explanation of why and how to fix
#[derive(Debug, Clone)]
pub struct ToolResultInterpretationSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for ToolResultInterpretationSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are a tool result interpreter. Analyze tool output and determine next actions.

Your job:
1. Determine if the tool succeeded, partially succeeded, or failed
2. Extract the key information from the output
3. Decide what should happen next
4. If there was an error, explain why and how to recover

Success levels:
- YES: Tool completed successfully, output matches expectations
- PARTIAL: Tool completed but output is incomplete or unexpected
- NO: Tool failed, errored, or produced no useful output

For error analysis:
- Identify the root cause
- Check for common issues (permissions, missing files, network)
- Suggest specific fixes or alternative approaches

Be concise but thorough in your analysis."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl ToolResultInterpretationSignature {
    /// Create a new tool result interpretation signature.
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

impl MetaSignature for ToolResultInterpretationSignature {
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
            "tool_name": {
                "type": "String",
                "desc": "Name of the tool that was called",
                "__dsrs_field_type": "input"
            },
            "tool_output": {
                "type": "String",
                "desc": "Raw output from the tool (stdout, stderr, result JSON)",
                "__dsrs_field_type": "input"
            },
            "original_intent": {
                "type": "String",
                "desc": "What we were trying to accomplish with this tool call",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "success": {
                "type": "String",
                "desc": "Success status: YES, PARTIAL, or NO",
                "__dsrs_field_type": "output"
            },
            "extracted_info": {
                "type": "String",
                "desc": "Key information extracted from the tool output",
                "__dsrs_field_type": "output"
            },
            "next_steps": {
                "type": "String",
                "desc": "What to do next based on the result",
                "__dsrs_field_type": "output"
            },
            "error_analysis": {
                "type": "String",
                "desc": "If failed, explanation of why and suggested fixes",
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

// ============================================================================
// ToolChainPlanningSignature
// ============================================================================

/// Tool Chain Planning Signature.
///
/// Plans multi-tool sequences for complex tasks.
/// Identifies dependencies and opportunities for parallelization.
///
/// # Inputs
/// - `goal`: What we're trying to accomplish
/// - `available_tools`: JSON array of tool definitions
/// - `constraints`: Time, cost, resource limits
///
/// # Outputs
/// - `tool_sequence`: JSON array of tool calls in execution order
/// - `dependencies`: Which calls depend on others (DAG edges)
/// - `parallelizable`: Groups of calls that can run in parallel
#[derive(Debug, Clone)]
pub struct ToolChainPlanningSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for ToolChainPlanningSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are a tool chain planner. Design multi-tool workflows for complex tasks.

Your job:
1. Break down the goal into tool-sized steps
2. Order the steps by dependencies
3. Identify which steps can run in parallel
4. Consider constraints (time, cost, resources)

Output format:
- tool_sequence: JSON array of {step_id, tool_name, params, depends_on}
- dependencies: Which steps must complete before others
- parallelizable: Groups of step_ids that can run concurrently

Planning rules:
1. Minimize total execution time by parallelizing where possible
2. Respect data dependencies (step B needs output from step A)
3. Stay within budget/resource constraints
4. Prefer simpler chains when possible
5. Include error handling steps for critical paths

Example sequence item:
{"step_id": "s1", "tool_name": "read_file", "params": {"path": "..."}, "depends_on": []}"#
                .to_string(),
            demos: vec![],
        }
    }
}

impl ToolChainPlanningSignature {
    /// Create a new tool chain planning signature.
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

impl MetaSignature for ToolChainPlanningSignature {
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
            "goal": {
                "type": "String",
                "desc": "What we're trying to accomplish",
                "__dsrs_field_type": "input"
            },
            "available_tools": {
                "type": "String",
                "desc": "JSON array of tool definitions with name, description, and parameters",
                "__dsrs_field_type": "input"
            },
            "constraints": {
                "type": "String",
                "desc": "Time, cost, and resource constraints as JSON",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "tool_sequence": {
                "type": "String",
                "desc": "JSON array of tool calls in execution order",
                "__dsrs_field_type": "output"
            },
            "dependencies": {
                "type": "String",
                "desc": "JSON object mapping step_id to list of required prior step_ids",
                "__dsrs_field_type": "output"
            },
            "parallelizable": {
                "type": "String",
                "desc": "JSON array of groups, where each group is step_ids that can run in parallel",
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_success_parse() {
        assert_eq!("YES".parse::<ToolSuccess>().unwrap(), ToolSuccess::Yes);
        assert_eq!("yes".parse::<ToolSuccess>().unwrap(), ToolSuccess::Yes);
        assert_eq!("SUCCESS".parse::<ToolSuccess>().unwrap(), ToolSuccess::Yes);
        assert_eq!("PARTIAL".parse::<ToolSuccess>().unwrap(), ToolSuccess::Partial);
        assert_eq!("incomplete".parse::<ToolSuccess>().unwrap(), ToolSuccess::Partial);
        assert_eq!("NO".parse::<ToolSuccess>().unwrap(), ToolSuccess::No);
        assert_eq!("fail".parse::<ToolSuccess>().unwrap(), ToolSuccess::No);
        assert_eq!("ERROR".parse::<ToolSuccess>().unwrap(), ToolSuccess::No);
    }

    #[test]
    fn test_tool_success_display() {
        assert_eq!(ToolSuccess::Yes.to_string(), "YES");
        assert_eq!(ToolSuccess::Partial.to_string(), "PARTIAL");
        assert_eq!(ToolSuccess::No.to_string(), "NO");
    }

    #[test]
    fn test_tool_selection_signature_fields() {
        let sig = ToolSelectionSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("task_description").is_some());
        assert!(inputs.get("available_tools").is_some());
        assert!(inputs.get("context").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("selected_tool").is_some());
        assert!(outputs.get("tool_params").is_some());
        assert!(outputs.get("expected_outcome").is_some());
        assert!(outputs.get("fallback_tool").is_some());
    }

    #[test]
    fn test_tool_selection_signature_instruction() {
        let sig = ToolSelectionSignature::new();
        let instruction = sig.instruction();
        assert!(instruction.contains("tool selection"));
        assert!(instruction.contains("fallback"));
    }

    #[test]
    fn test_tool_result_interpretation_signature_fields() {
        let sig = ToolResultInterpretationSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("tool_name").is_some());
        assert!(inputs.get("tool_output").is_some());
        assert!(inputs.get("original_intent").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("success").is_some());
        assert!(outputs.get("extracted_info").is_some());
        assert!(outputs.get("next_steps").is_some());
        assert!(outputs.get("error_analysis").is_some());
    }

    #[test]
    fn test_tool_result_interpretation_signature_instruction() {
        let sig = ToolResultInterpretationSignature::new();
        let instruction = sig.instruction();
        assert!(instruction.contains("interpreter"));
        assert!(instruction.contains("YES"));
        assert!(instruction.contains("PARTIAL"));
        assert!(instruction.contains("NO"));
    }

    #[test]
    fn test_tool_chain_planning_signature_fields() {
        let sig = ToolChainPlanningSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("goal").is_some());
        assert!(inputs.get("available_tools").is_some());
        assert!(inputs.get("constraints").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("tool_sequence").is_some());
        assert!(outputs.get("dependencies").is_some());
        assert!(outputs.get("parallelizable").is_some());
    }

    #[test]
    fn test_tool_chain_planning_signature_instruction() {
        let sig = ToolChainPlanningSignature::new();
        let instruction = sig.instruction();
        assert!(instruction.contains("planner"));
        assert!(instruction.contains("parallel"));
        assert!(instruction.contains("dependencies"));
    }

    #[test]
    fn test_signature_with_custom_instruction() {
        let sig = ToolSelectionSignature::new()
            .with_instruction("Custom instruction");
        assert_eq!(sig.instruction(), "Custom instruction");
    }

    #[test]
    fn test_signature_with_demo() {
        let demo = Example::default();
        let sig = ToolSelectionSignature::new().with_demo(demo);
        assert_eq!(sig.demos().len(), 1);
    }
}
