use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Signature;

/// Route family for the first compiled-agent graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRoute {
    /// Prompt is about provider readiness or status.
    ProviderStatus,
    /// Prompt is about wallet balance or earnings state.
    WalletStatus,
    /// Prompt is out of scope and should be refused cleanly.
    Unsupported,
}

/// Tool declaration exposed to the policy and argument phases.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    /// Stable tool name.
    pub name: String,
    /// Short tool description.
    pub description: String,
}

/// Tool call emitted by the argument phase.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Stable tool name.
    pub tool_name: String,
    /// Structured arguments that the tool should receive.
    pub arguments: Value,
}

/// Tool result consumed by grounded-answer and verification phases.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    /// Tool name that produced this result.
    pub tool_name: String,
    /// Structured result payload.
    pub payload: Value,
}

/// Intent route input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IntentRouteInput {
    /// Original user request.
    pub user_request: String,
}

/// Intent route output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IntentRouteOutput {
    /// Chosen route.
    pub route: AgentRoute,
    /// Short rationale for the route decision.
    pub rationale: String,
}

/// Tool policy input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolPolicyInput {
    /// Original user request.
    pub user_request: String,
    /// Route chosen in the previous phase.
    pub route: AgentRoute,
    /// Tools available to the policy phase.
    pub available_tools: Vec<ToolSpec>,
}

/// Tool policy output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolPolicyOutput {
    /// Tools exposed for this prompt.
    pub selected_tools: Vec<ToolSpec>,
    /// Short policy rationale.
    pub rationale: String,
}

/// Tool-argument input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolArgumentsInput {
    /// Original user request.
    pub user_request: String,
    /// Route chosen in the previous phase.
    pub route: AgentRoute,
    /// Tools exposed by tool policy.
    pub selected_tools: Vec<ToolSpec>,
}

/// Tool-argument output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ToolArgumentsOutput {
    /// Tool calls to execute.
    pub calls: Vec<ToolCall>,
}

/// Grounded-answer input.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroundedAnswerInput {
    /// Original user request.
    pub user_request: String,
    /// Route chosen in the previous phase.
    pub route: AgentRoute,
    /// Tool results available for synthesis.
    pub tool_results: Vec<ToolResult>,
}

/// Grounded-answer output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GroundedAnswerOutput {
    /// User-visible grounded answer.
    pub answer: String,
    /// Tool names cited by the grounded answer.
    pub grounded_tool_names: Vec<String>,
}

/// Verification outcome class.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifyVerdict {
    /// Final answer is grounded and acceptable.
    AcceptGroundedAnswer,
    /// Prompt is unsupported and should be refused.
    UnsupportedRefusal,
    /// Primary answer is too weak and should fall back.
    NeedsFallback,
}

/// Verification input.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerifyInput {
    /// Original user request.
    pub user_request: String,
    /// Route chosen earlier in the graph.
    pub route: AgentRoute,
    /// Tool calls emitted by the argument phase.
    pub tool_calls: Vec<ToolCall>,
    /// Tool results gathered from execution.
    pub tool_results: Vec<ToolResult>,
    /// Candidate final answer.
    pub candidate_answer: String,
}

/// Verification output.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VerifyOutput {
    /// Final verdict for the current answer path.
    pub verdict: VerifyVerdict,
    /// Short rationale for the verdict.
    pub rationale: String,
}

/// Signature marker for route selection.
#[derive(Clone, Copy, Debug)]
pub struct IntentRouteSignature;

impl Signature for IntentRouteSignature {
    const NAME: &'static str = "intent_route";
    type Input = IntentRouteInput;
    type Output = IntentRouteOutput;
}

/// Signature marker for tool policy selection.
#[derive(Clone, Copy, Debug)]
pub struct ToolPolicySignature;

impl Signature for ToolPolicySignature {
    const NAME: &'static str = "tool_policy";
    type Input = ToolPolicyInput;
    type Output = ToolPolicyOutput;
}

/// Signature marker for tool argument generation.
#[derive(Clone, Copy, Debug)]
pub struct ToolArgumentsSignature;

impl Signature for ToolArgumentsSignature {
    const NAME: &'static str = "tool_arguments";
    type Input = ToolArgumentsInput;
    type Output = ToolArgumentsOutput;
}

/// Signature marker for grounded synthesis.
#[derive(Clone, Copy, Debug)]
pub struct GroundedAnswerSignature;

impl Signature for GroundedAnswerSignature {
    const NAME: &'static str = "grounded_answer";
    type Input = GroundedAnswerInput;
    type Output = GroundedAnswerOutput;
}

/// Signature marker for verification or refusal.
#[derive(Clone, Copy, Debug)]
pub struct VerifySignature;

impl Signature for VerifySignature {
    const NAME: &'static str = "verify";
    type Input = VerifyInput;
    type Output = VerifyOutput;
}
