import { Schema } from "effect"

/**
 * Agent metadata information
 */
export class AgentInfo extends Schema.Class<AgentInfo>("AgentInfo")({
  type: Schema.String, // Example value: "solver"
  version: Schema.String, // Example value: "1.0.0"
  instance_id: Schema.String, // Example value: "solver-uuid-of-issue-session-id"
  state_schema_version: Schema.String // Example value: "1.1"
}) {}

/**
 * Holds relevant timestamps for agent activity
 */
export class Timestamps extends Schema.Class<Timestamps>("Timestamps")({
  created_at: Schema.String, // ISO 8601 date string
  last_saved_at: Schema.String, // ISO 8601 date string
  last_action_at: Schema.String // ISO 8601 date string
}) {}

/**
 * Cached details about the issue being processed
 */
export class IssueDetailsCache extends Schema.Class<IssueDetailsCache>("IssueDetailsCache")({
  title: Schema.String,
  description_snippet: Schema.String,
  status: Schema.String, // Example value: "open"
  labels: Schema.Array(Schema.String),
  source_url: Schema.String
}) {}

/**
 * Represents the agent's current task focus
 */
export class CurrentTask extends Schema.Class<CurrentTask>("CurrentTask")({
  repo_owner: Schema.String,
  repo_name: Schema.String,
  repo_branch: Schema.String,
  issue_number: Schema.Number,
  issue_details_cache: Schema.Union(IssueDetailsCache, Schema.Null),
  status: Schema.String, // idle | planning | researching | implementing | testing | blocked | completed | error
  current_step_index: Schema.Number // 0-based index into plan array
}) {}

/**
 * Represents a logged tool invocation
 */
export class ToolCall extends Schema.Class<ToolCall>("ToolCall")({
  timestamp: Schema.String, // ISO 8601 date string
  tool_name: Schema.String,
  parameters: Schema.Struct({}), // Arbitrary JSON object of parameters
  status: Schema.String, // Example values: "success", "error"
  result_preview: Schema.String, // A short representation of the result
  full_result_ref: Schema.Union(Schema.String, Schema.Null) // Optional reference to externally stored full result
}) {}

/**
 * Represents a step in the agent's execution plan
 */
export class PlanStep extends Schema.Class<PlanStep>("PlanStep")({
  id: Schema.String, // Unique step identifier
  step_number: Schema.Number, // 1-based index
  description: Schema.String,
  status: Schema.String, // pending | in_progress | completed | skipped | error
  start_time: Schema.Union(Schema.String, Schema.Null), // ISO 8601 date string
  end_time: Schema.Union(Schema.String, Schema.Null), // ISO 8601 date string
  result_summary: Schema.Union(Schema.String, Schema.Null),
  tool_calls: Schema.Array(ToolCall) // Tools used for this specific step
}) {}

/**
 * Represents a current focus on a specific file
 */
export class FileFocus extends Schema.Class<FileFocus>("FileFocus")({
  path: Schema.String,
  relevant_lines: Schema.Array(Schema.Number)
}) {}

/**
 * Represents a code snippet that is relevant to the current task
 */
export class CodeSnippet extends Schema.Class<CodeSnippet>("CodeSnippet")({
  file_path: Schema.String,
  snippet: Schema.String,
  reason: Schema.String
}) {}

/**
 * Represents a reference to an external resource like an issue or PR
 */
export class ExternalReference extends Schema.Class<ExternalReference>("ExternalReference")({
  type: Schema.String, // e.g., "issue", "pr"
  identifier: Schema.String, // e.g., "121", "45"
  relationship: Schema.String, // e.g., "relates_to", "blocked_by"
  source: Schema.String // e.g., "github"
}) {}

/**
 * Holds dynamic context information during execution
 */
export class ExecutionContext extends Schema.Class<ExecutionContext>("ExecutionContext")({
  current_file_focus: Schema.Union(FileFocus, Schema.Null),
  relevant_code_snippets: Schema.Array(CodeSnippet),
  external_references: Schema.Array(ExternalReference),
  files_modified_in_session: Schema.Array(Schema.String)
}) {}

/**
 * Represents a tool call within an AI conversation
 */
export class ConversationToolCall extends Schema.Class<ConversationToolCall>("ConversationToolCall")({
  id: Schema.String,
  name: Schema.String,
  input: Schema.Struct({}) // Parameters passed to the tool
}) {}

/**
 * Represents a message in the conversation history
 */
export class ConversationMessage extends Schema.Class<ConversationMessage>("ConversationMessage")({
  role: Schema.String, // Example values: "user", "assistant", "tool"
  content: Schema.String,
  timestamp: Schema.String, // ISO 8601 date string
  tool_calls: Schema.Union(Schema.Array(ConversationToolCall), Schema.Null) // Present for tool usage messages
}) {}

/**
 * Represents a key decision made by the agent
 */
export class KeyDecision extends Schema.Class<KeyDecision>("KeyDecision")({
  timestamp: Schema.String, // ISO 8601 date string
  decision: Schema.String,
  reasoning: Schema.String,
  confidence: Schema.Number // Value between 0 and 1
}) {}

/**
 * Represents an important finding discovered by the agent
 */
export class ImportantFinding extends Schema.Class<ImportantFinding>("ImportantFinding")({
  timestamp: Schema.String, // ISO 8601 date string
  finding: Schema.String,
  source: Schema.String, // Example value: "code_analysis"
  confidence: Schema.Number // Value between 0 and 1
}) {}

/**
 * Holds the agent's memory components
 */
export class Memory extends Schema.Class<Memory>("Memory")({
  conversation_history: Schema.Array(ConversationMessage),
  key_decisions: Schema.Array(KeyDecision),
  important_findings: Schema.Array(ImportantFinding),
  scratchpad: Schema.String
}) {}

/**
 * Tracks LLM token usage
 */
export class LLMTokensUsed extends Schema.Class<LLMTokensUsed>("LLMTokensUsed")({
  prompt: Schema.Number,
  completion: Schema.Number
}) {}

/**
 * Holds performance and usage metrics
 */
export class Metrics extends Schema.Class<Metrics>("Metrics")({
  steps_completed: Schema.Number,
  total_steps_in_plan: Schema.Number,
  session_start_time: Schema.String, // ISO 8601 date string
  total_time_spent_seconds: Schema.Number,
  llm_calls_made: Schema.Number,
  llm_tokens_used: LLMTokensUsed,
  tools_called: Schema.Number,
  commits_made: Schema.Number
}) {}

/**
 * Represents the last error encountered by the agent
 */
export class LastError extends Schema.Class<LastError>("LastError")({
  timestamp: Schema.String, // ISO 8601 date string
  message: Schema.String,
  type: Schema.Literal("api_error", "tool_error", "internal"),
  details: Schema.String // Can contain stack trace or more info
}) {}

/**
 * Holds information about errors encountered
 */
export class ErrorState extends Schema.Class<ErrorState>("ErrorState")({
  last_error: Schema.Union(LastError, Schema.Null),
  consecutive_error_count: Schema.Number,
  retry_count_for_current_action: Schema.Number,
  blocked_reason: Schema.Union(Schema.String, Schema.Null)
}) {}

/**
 * LLM configuration settings
 */
export class LLMConfig extends Schema.Class<LLMConfig>("LLMConfig")({
  model: Schema.String,
  temperature: Schema.Number,
  max_tokens: Schema.Number
}) {}

/**
 * Holds agent configuration settings
 */
export class Configuration extends Schema.Class<Configuration>("Configuration")({
  agent_goal: Schema.String,
  llm_config: LLMConfig,
  max_retries_per_action: Schema.Number,
  allowed_actions: Schema.Array(Schema.String),
  restricted_paths: Schema.Array(Schema.String),
  action_timeout_seconds: Schema.Number,
  session_timeout_minutes: Schema.Number,
  github_token_available: Schema.Boolean
}) {}

/**
 * Main, top-level state object for the agent
 */
export class AgentState extends Schema.Class<AgentState>("AgentState")({
  agent_info: AgentInfo,
  timestamps: Timestamps,
  current_task: CurrentTask,
  plan: Schema.Array(PlanStep),
  execution_context: ExecutionContext,
  tool_invocation_log: Schema.Array(ToolCall),
  memory: Memory,
  metrics: Metrics,
  error_state: ErrorState,
  configuration: Configuration
}) {}
