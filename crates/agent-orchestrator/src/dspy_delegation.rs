//! DSPy Delegation Signature for Sisyphus.
//!
//! Replaces static prompt-based delegation with a learnable DSPy signature.

use anyhow::Result;
use dsrs::core::signature::MetaSignature;
use dsrs::data::example::Example;
use serde_json::{json, Value};

/// Target agent for delegation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetAgent {
    /// Oracle - Architecture decisions, complex debugging.
    Oracle,
    /// Librarian - External docs, GitHub search, OSS reference.
    Librarian,
    /// Explore - Codebase navigation, pattern search.
    Explore,
    /// Frontend - UI/UX development, visual changes.
    Frontend,
    /// DocWriter - README, API docs, guides.
    DocWriter,
    /// Multimodal - PDF/image analysis.
    Multimodal,
    /// Self - Handle directly without delegation.
    Direct,
}

impl std::fmt::Display for TargetAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TargetAgent::Oracle => write!(f, "oracle"),
            TargetAgent::Librarian => write!(f, "librarian"),
            TargetAgent::Explore => write!(f, "explore"),
            TargetAgent::Frontend => write!(f, "frontend"),
            TargetAgent::DocWriter => write!(f, "docwriter"),
            TargetAgent::Multimodal => write!(f, "multimodal"),
            TargetAgent::Direct => write!(f, "direct"),
        }
    }
}

impl std::str::FromStr for TargetAgent {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "oracle" => Ok(TargetAgent::Oracle),
            "librarian" => Ok(TargetAgent::Librarian),
            "explore" => Ok(TargetAgent::Explore),
            "frontend" => Ok(TargetAgent::Frontend),
            "docwriter" | "doc_writer" => Ok(TargetAgent::DocWriter),
            "multimodal" => Ok(TargetAgent::Multimodal),
            "direct" | "self" | "sisyphus" => Ok(TargetAgent::Direct),
            _ => Err(format!("Unknown agent: {}", s)),
        }
    }
}

/// Delegation Signature for Sisyphus.
///
/// Decides which specialized subagent should handle a task.
/// This replaces the rule-based delegation logic with a learnable approach.
///
/// # Inputs
/// - `task_description`: What needs to be done
/// - `available_agents`: JSON with agent capabilities and current status
/// - `current_workload`: What each agent is currently doing
///
/// # Outputs
/// - `assigned_agent`: Which agent to delegate to (oracle, librarian, explore, frontend, docwriter, multimodal, direct)
/// - `task_refinement`: Refined instructions for the assigned agent
/// - `expected_deliverables`: Concrete list of what the agent should produce
/// - `fallback_agent`: Alternative agent if primary fails
#[derive(Debug, Clone)]
pub struct DelegationSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for DelegationSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are Sisyphus, the master orchestrator. Decide which specialized agent should handle this task.

Available agents:
- oracle: Architecture and design decisions, complex debugging, security analysis (GPT-5.2, read-only)
- librarian: External library docs, API usage examples, OSS reference lookup (Claude Sonnet, read-only)
- explore: Find code in codebase, locate definitions, trace references, pattern search (Grok-3, read-only)
- frontend: UI/UX design, CSS styling, visual components (Gemini Pro, can edit)
- docwriter: README, API docs, guides, code comments (Gemini Pro, can edit)
- multimodal: Analyze images, PDFs, diagrams, screenshots (Gemini Flash, read-only)
- direct: Handle yourself without delegation (for trivial or explicit tasks)

Delegation rules:
1. NEVER work alone when a specialist is available for the domain
2. Delegate the smallest atomic task possible
3. Provide clear, specific expected deliverables
4. Always include a fallback agent
5. Frontend visual/styling work MUST go to frontend
6. External docs lookup MUST go to librarian
7. Codebase exploration MUST go to explore

Task classification:
- Trivial (single file, known location) → direct
- Explicit (specific file/line, clear command) → direct
- Exploratory ("How does X work?") → explore
- Visual/UI changes → frontend
- Documentation → docwriter
- Architecture/design → oracle
- External library questions → librarian
- Image/PDF analysis → multimodal"#
                .to_string(),
            demos: vec![],
        }
    }
}

impl DelegationSignature {
    /// Create a new delegation signature.
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

impl MetaSignature for DelegationSignature {
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
            "available_agents": {
                "type": "String",
                "desc": "JSON object with agent capabilities, models, and permissions",
                "__dsrs_field_type": "input"
            },
            "current_workload": {
                "type": "String",
                "desc": "JSON object showing what each agent is currently doing",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "assigned_agent": {
                "type": "String",
                "desc": "Target agent: oracle, librarian, explore, frontend, docwriter, multimodal, or direct",
                "__dsrs_field_type": "output"
            },
            "task_refinement": {
                "type": "String",
                "desc": "Refined, specific instructions for the assigned agent including TASK, EXPECTED OUTCOME, MUST DO, MUST NOT DO",
                "__dsrs_field_type": "output"
            },
            "expected_deliverables": {
                "type": "String",
                "desc": "Concrete list of what the agent should produce",
                "__dsrs_field_type": "output"
            },
            "fallback_agent": {
                "type": "String",
                "desc": "Alternative agent if primary fails or is unavailable",
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
    fn test_target_agent_parse() {
        assert_eq!("oracle".parse::<TargetAgent>().unwrap(), TargetAgent::Oracle);
        assert_eq!("LIBRARIAN".parse::<TargetAgent>().unwrap(), TargetAgent::Librarian);
        assert_eq!("Explore".parse::<TargetAgent>().unwrap(), TargetAgent::Explore);
        assert_eq!("frontend".parse::<TargetAgent>().unwrap(), TargetAgent::Frontend);
        assert_eq!("docwriter".parse::<TargetAgent>().unwrap(), TargetAgent::DocWriter);
        assert_eq!("doc_writer".parse::<TargetAgent>().unwrap(), TargetAgent::DocWriter);
        assert_eq!("multimodal".parse::<TargetAgent>().unwrap(), TargetAgent::Multimodal);
        assert_eq!("direct".parse::<TargetAgent>().unwrap(), TargetAgent::Direct);
        assert_eq!("self".parse::<TargetAgent>().unwrap(), TargetAgent::Direct);
    }

    #[test]
    fn test_target_agent_display() {
        assert_eq!(TargetAgent::Oracle.to_string(), "oracle");
        assert_eq!(TargetAgent::Librarian.to_string(), "librarian");
        assert_eq!(TargetAgent::Explore.to_string(), "explore");
        assert_eq!(TargetAgent::Frontend.to_string(), "frontend");
        assert_eq!(TargetAgent::DocWriter.to_string(), "docwriter");
        assert_eq!(TargetAgent::Multimodal.to_string(), "multimodal");
        assert_eq!(TargetAgent::Direct.to_string(), "direct");
    }

    #[test]
    fn test_delegation_signature_fields() {
        let sig = DelegationSignature::new();

        let inputs = sig.input_fields();
        assert!(inputs.get("task_description").is_some());
        assert!(inputs.get("available_agents").is_some());
        assert!(inputs.get("current_workload").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("assigned_agent").is_some());
        assert!(outputs.get("task_refinement").is_some());
        assert!(outputs.get("expected_deliverables").is_some());
        assert!(outputs.get("fallback_agent").is_some());
    }

    #[test]
    fn test_delegation_signature_instruction() {
        let sig = DelegationSignature::new();
        let instruction = sig.instruction();
        assert!(instruction.contains("Sisyphus"));
        assert!(instruction.contains("oracle"));
        assert!(instruction.contains("librarian"));
        assert!(instruction.contains("explore"));
    }
}
