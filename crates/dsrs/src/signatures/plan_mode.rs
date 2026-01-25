//! Plan mode signatures for Autopilot desktop workflows.

use dsrs_macros::Signature;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Exploration topic from structured output.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExplorationTopic {
    /// Short topic name (2-4 words).
    pub name: String,
    /// What to explore (1-2 sentences).
    pub focus: String,
    /// Search patterns or keywords for ripgrep.
    pub patterns: Vec<String>,
}

/// Topics response schema.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TopicsResponse {
    pub topics: Vec<ExplorationTopic>,
}

/// Topic Decomposition Signature - Breaks user prompts into focused exploration areas.
#[Signature]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicDecompositionSignature {
    /// Software Planning: Analyze user request and repository structure
    /// to decompose into 2-4 focused exploration topics with search patterns.
    #[input]
    /// User request to decompose.
    pub user_prompt: String,

    #[input]
    /// Repository file tree for context.
    pub file_tree: String,

    #[output]
    /// Expected output: 2-4 exploration topics as JSON.
    pub topics: String,
}

/// Parallel Exploration Signature - Individual agent exploration.
#[Signature]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelExplorationSignature {
    /// Codebase Exploration: Use available tools to explore the assigned topic
    /// and gather relevant information about code patterns and implementation details.
    #[input]
    /// Assigned exploration topic.
    pub topic: String,

    #[input]
    /// Focus area for this agent.
    pub focus: String,

    #[input]
    /// Suggested search patterns.
    pub patterns: String,

    #[input]
    /// Repository path for tool execution.
    pub repo_path: String,

    #[input]
    /// File context snapshot for this agent.
    pub file_context: String,

    #[output]
    /// Exploration findings (output).
    pub findings: String,

    #[output]
    /// Files examined during exploration.
    pub files_examined: String,
}

/// Plan Synthesis Signature - Combine exploration results into implementation plan.
#[Signature]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSynthesisSignature {
    /// Plan Synthesis: Based on exploration findings, create a comprehensive
    /// implementation plan with specific steps, files to modify, and clear objectives.
    #[input]
    /// Original user prompt.
    pub user_prompt: String,

    #[input]
    /// Combined exploration results from all agents.
    pub exploration_results: String,

    #[input]
    /// Repository context for file references.
    pub repo_context: String,

    #[output]
    /// Final implementation plan (output).
    pub implementation_plan: String,
}

/// Tool Selection Signature - Choose appropriate tools for exploration.
#[Signature]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSelectionSignature {
    #[input]
    /// Current exploration context.
    pub context: String,
    #[input]
    /// Available tools (rg, read_file, list_files).
    pub available_tools: String,
    #[input]
    /// Exploration goal.
    pub goal: String,
    #[output]
    /// Selected tools and strategy.
    pub tool_strategy: String,
}

/// Complexity Classification Signature - Route tasks based on complexity.
#[Signature]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityClassificationSignature {
    /// Complexity Analysis: Analyze the task and codebase to classify complexity
    /// and determine the appropriate planning approach.
    #[input]
    /// Task description to analyze.
    pub task_description: String,

    #[input]
    /// Repository size indicators (file count, structure).
    pub repo_indicators: String,

    #[input]
    /// Domain complexity signals.
    pub domain_signals: String,

    #[output]
    /// Complexity level: Low, Medium, High, VeryHigh.
    pub complexity: String,

    #[output]
    /// Routing decision for pipeline.
    pub routing_decision: String,

    #[output]
    /// Reasoning for classification.
    pub reasoning: String,
}

/// Deep Planning Signature - Complex multi-step reasoning with chain-of-thought.
#[Signature(cot)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepPlanningSignature {
    /// Deep Planning: Perform complex multi-step reasoning to analyze sophisticated
    /// tasks requiring coordination across multiple systems, files, or domains.
    #[input]
    /// Complex request requiring deep analysis.
    pub complex_request: String,

    #[input]
    /// Full codebase analysis context.
    pub codebase_analysis: String,

    #[input]
    /// Constraints and requirements.
    pub constraints: String,

    #[output]
    /// Chain-of-thought reasoning process (output).
    pub reasoning: String,

    #[output]
    /// Strategic approach (output).
    pub strategy: String,

    #[output]
    /// Detailed implementation plan (output).
    pub implementation_plan: String,

    #[output]
    /// Risk assessment and mitigations (output).
    pub risk_assessment: String,
}

/// Result Validation Signature - Evaluate generated output for quality.
#[Signature]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultValidationSignature {
    /// Analysis: Evaluate the generated output against the original request
    /// and criteria to ensure high quality and completeness.
    #[input]
    pub original_request: String,
    #[input]
    pub generated_output: String,
    #[input]
    pub criteria: String,

    #[output]
    pub quality_assessment: String,
    #[output]
    pub issues: String,
    #[output]
    pub confidence: String,
}
