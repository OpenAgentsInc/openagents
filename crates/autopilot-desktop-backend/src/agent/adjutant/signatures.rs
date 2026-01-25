//! DSPy Signatures for Adjutant Agent
//!
//! Defines the core DSPy signatures used by Adjutant for planning, topic decomposition,
//! parallel exploration, and plan synthesis based on the planmode system.

use dsrs_macros::Signature;
use serde::{Deserialize, Serialize};

/// Topic Decomposition Signature - Breaks user prompts into focused exploration areas
///
/// Based on planmode's topic decomposition with JSON schema enforcement
#[Signature]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicDecompositionSignature {
    /// Software Planning: Analyze user request and repository structure
    /// to decompose into 2-4 focused exploration topics with search patterns.

    #[input]
    /// User request to decompose
    pub user_prompt: String,

    #[input]
    /// Repository file tree for context
    pub file_tree: String,

    #[output]
    /// Expected output: 2-4 exploration topics as JSON
    pub topics: String,
}

/// Exploration Topic from structured output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationTopic {
    /// Short topic name (2-4 words)
    pub name: String,
    /// What to explore (1-2 sentences)
    pub focus: String,
    /// Search patterns or keywords for ripgrep
    pub patterns: Vec<String>,
}

/// Topics Response for JSON schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicsResponse {
    pub topics: Vec<ExplorationTopic>,
}

/// Parallel Exploration Signature - Individual agent exploration
///
/// Each explore agent gets its own context and tool calls (max 8 per agent)
#[Signature]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelExplorationSignature {
    /// Codebase Exploration: Use available tools to explore the assigned topic
    /// and gather relevant information about code patterns and implementation details.

    #[input]
    /// Assigned exploration topic
    pub topic: String,

    #[input]
    /// Focus area for this agent
    pub focus: String,

    #[input]
    /// Suggested search patterns
    pub patterns: String, // JSON array of patterns

    #[input]
    /// Repository path for tool execution
    pub repo_path: String,

    #[output]
    /// Exploration findings (output)
    pub findings: String,

    #[output]
    /// Files examined during exploration
    pub files_examined: String, // JSON array of file paths
}

/// Plan Synthesis Signature - Combine exploration results into implementation plan
///
/// Takes findings from all parallel agents and synthesizes into actionable plan
#[Signature]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSynthesisSignature {
    /// Plan Synthesis: Based on exploration findings, create a comprehensive
    /// implementation plan with specific steps, files to modify, and clear objectives.

    #[input]
    /// Original user prompt
    pub user_prompt: String,

    #[input]
    /// Combined exploration results from all agents
    pub exploration_results: String,

    #[input]
    /// Repository context for file references
    pub repo_context: String,

    #[output]
    /// Final implementation plan (output)
    pub implementation_plan: String,
}

/// Tool Selection Signature - Choose appropriate tools for exploration
///
/// Used by exploration agents to select optimal tools for their topic
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSelectionSignature {
    /// Current exploration context
    pub context: String,
    /// Available tools (rg, read_file, list_files)
    pub available_tools: String,
    /// Exploration goal
    pub goal: String,
    /// Selected tools and strategy (output)
    pub tool_strategy: String,
}

/// Complexity Classification Signature - Route tasks based on complexity
///
/// Determines whether to use simple or deep planning based on task analysis
#[Signature]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityClassificationSignature {
    /// Complexity Analysis: Analyze the task and codebase to classify complexity
    /// and determine the appropriate planning approach.

    #[input]
    /// Task description to analyze
    pub task_description: String,

    #[input]
    /// Repository size indicators (file count, structure)
    pub repo_indicators: String,

    #[input]
    /// Domain complexity signals
    pub domain_signals: String,

    #[output]
    /// Complexity level: Low, Medium, High, VeryHigh
    pub complexity: String,

    #[output]
    /// Routing decision for pipeline
    pub routing_decision: String,

    #[output]
    /// Reasoning for classification
    pub reasoning: String,
}

/// Deep Planning Signature - Complex multi-step reasoning with chain-of-thought
///
/// Used for sophisticated tasks requiring analysis across multiple domains
#[Signature(cot)] // Enable chain-of-thought reasoning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepPlanningSignature {
    /// Deep Planning: Perform complex multi-step reasoning to analyze sophisticated
    /// tasks requiring coordination across multiple systems, files, or domains.

    #[input]
    /// Complex request requiring deep analysis
    pub complex_request: String,

    #[input]
    /// Full codebase analysis context
    pub codebase_analysis: String,

    #[input]
    /// Constraints and requirements
    pub constraints: String,

    #[output]
    /// Chain-of-thought reasoning process (output)
    pub reasoning: String,

    #[output]
    /// Strategic approach (output)
    pub strategy: String,

    #[output]
    /// Detailed implementation plan (output)
    pub implementation_plan: String,

    #[output]
    /// Risk assessment and mitigations (output)
    pub risk_assessment: String,
}

// ResultValidationSignature moved to planning.rs to avoid visibility issues with the macro

// Note: The #[Signature] macro generates the `new()` constructors automatically

/// Plan Mode Pipeline Configuration
#[derive(Debug, Clone)]
pub struct PlanModeConfig {
    /// Maximum number of exploration topics (2-4)
    pub max_topics: usize,
    /// Maximum tool calls per exploration agent
    pub max_tool_calls_per_agent: usize,
    /// Enable deep planning for complex tasks
    pub enable_deep_planning: bool,
    /// Complexity threshold for deep planning (0.0-1.0)
    pub deep_planning_threshold: f32,
    /// Enable result validation
    pub enable_validation: bool,
}

impl Default for PlanModeConfig {
    fn default() -> Self {
        Self {
            max_topics: 4,
            max_tool_calls_per_agent: 8,
            enable_deep_planning: true,
            deep_planning_threshold: 0.7,
            enable_validation: true,
        }
    }
}
