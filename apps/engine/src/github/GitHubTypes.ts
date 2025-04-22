import { Schema } from "effect"

/**
 * GitHub User type definition
 */
export class GitHubUser extends Schema.Class<GitHubUser>("GitHubUser")({
  id: Schema.Number,
  login: Schema.String,
  avatar_url: Schema.String,
  html_url: Schema.String,
  type: Schema.String
}) { }

/**
 * GitHub Label type definition
 */
export class GitHubLabel extends Schema.Class<GitHubLabel>("GitHubLabel")({
  id: Schema.Number,
  name: Schema.String,
  color: Schema.String,
  description: Schema.Union(Schema.String, Schema.Null)
}) { }

/**
 * GitHub Issue type definition
 */
export class GitHubIssue extends Schema.Class<GitHubIssue>("GitHubIssue")({
  id: Schema.Number,
  number: Schema.Number,
  title: Schema.String,
  body: Schema.String,
  state: Schema.Literal("open", "closed"),
  html_url: Schema.String,
  created_at: Schema.String,
  updated_at: Schema.String,
  user: GitHubUser,
  labels: Schema.Array(GitHubLabel),
  assignees: Schema.Array(GitHubUser),
  comments: Schema.Number
}) { }

/**
 * GitHub Issue Comment type definition
 */
export class GitHubIssueComment extends Schema.Class<GitHubIssueComment>("GitHubIssueComment")({
  id: Schema.Number,
  body: Schema.String,
  created_at: Schema.String,
  updated_at: Schema.String,
  user: GitHubUser,
  html_url: Schema.String
}) { }

/**
 * GitHub Repository type definition
 */
export class GitHubRepository extends Schema.Class<GitHubRepository>("GitHubRepository")({
  id: Schema.Number,
  name: Schema.String,
  full_name: Schema.String,
  owner: GitHubUser,
  html_url: Schema.String,
  description: Schema.Union(Schema.String, Schema.Null),
  visibility: Schema.String,
  default_branch: Schema.String,
  open_issues_count: Schema.Number,
  topics: Schema.Union(Schema.Array(Schema.String), Schema.Null)
}) { }

/**
 * GitHub Issue List Response type definition
 */
export class GitHubIssueListResponse extends Schema.Class<GitHubIssueListResponse>("GitHubIssueListResponse")({
  issues: Schema.Array(GitHubIssue)
}) { }

/**
 * GitHub Error type definition
 */
export class GitHubError extends Schema.Class<GitHubError>("GitHubError")({
  message: Schema.String,
  documentation_url: Schema.Union(Schema.String, Schema.Null)
}) { }

/**
 * Agent state type definition
 */
export class AgentInfo extends Schema.Class<AgentInfo>("AgentInfo")({
  type: Schema.String,
  version: Schema.String,
  instance_id: Schema.String,
  state_schema_version: Schema.String
}) { }

export class Timestamps extends Schema.Class<Timestamps>("Timestamps")({
  created_at: Schema.String,
  last_saved_at: Schema.String,
  last_action_at: Schema.String
}) { }

export class IssueDetailsCache extends Schema.Class<IssueDetailsCache>("IssueDetailsCache")({
  title: Schema.String,
  description_snippet: Schema.String,
  status: Schema.String,
  labels: Schema.Array(Schema.String),
  source_url: Schema.String
}) { }

export class CurrentTask extends Schema.Class<CurrentTask>("CurrentTask")({
  repo_owner: Schema.String,
  repo_name: Schema.String,
  repo_branch: Schema.String,
  issue_number: Schema.Number,
  issue_details_cache: Schema.Union(IssueDetailsCache, Schema.Null),
  status: Schema.String,
  current_step_index: Schema.Number
}) { }

export class ToolCall extends Schema.Class<ToolCall>("ToolCall")({
  timestamp: Schema.String,
  tool_name: Schema.String,
  parameters: Schema.Struct({}),
  status: Schema.String,
  result_preview: Schema.String,
  full_result_ref: Schema.Union(Schema.String, Schema.Null)
}) { }

export class PlanStep extends Schema.Class<PlanStep>("PlanStep")({
  id: Schema.String,
  step_number: Schema.Number,
  description: Schema.String,
  status: Schema.String,
  start_time: Schema.Union(Schema.String, Schema.Null),
  end_time: Schema.Union(Schema.String, Schema.Null),
  result_summary: Schema.Union(Schema.String, Schema.Null),
  tool_calls: Schema.Array(ToolCall)
}) { }

export class ExecutionContext extends Schema.Class<ExecutionContext>("ExecutionContext")({
  current_file_focus: Schema.Union(Schema.Struct({}), Schema.Null),
  relevant_code_snippets: Schema.Array(Schema.Struct({})),
  external_references: Schema.Array(Schema.Struct({})),
  files_modified_in_session: Schema.Array(Schema.String)
}) { }

export class ConversationMessage extends Schema.Class<ConversationMessage>("ConversationMessage")({
  role: Schema.String,
  content: Schema.String,
  timestamp: Schema.String,
  tool_calls: Schema.Union(Schema.Array(Schema.Struct({})), Schema.Null)
}) { }

export class KeyDecision extends Schema.Class<KeyDecision>("KeyDecision")({
  timestamp: Schema.String,
  decision: Schema.String,
  reasoning: Schema.String,
  confidence: Schema.Number
}) { }

export class ImportantFinding extends Schema.Class<ImportantFinding>("ImportantFinding")({
  timestamp: Schema.String,
  finding: Schema.String,
  source: Schema.String,
  confidence: Schema.Number
}) { }

export class Memory extends Schema.Class<Memory>("Memory")({
  conversation_history: Schema.Array(ConversationMessage),
  key_decisions: Schema.Array(KeyDecision),
  important_findings: Schema.Array(ImportantFinding),
  scratchpad: Schema.String
}) { }

export class Metrics extends Schema.Class<Metrics>("Metrics")({
  steps_completed: Schema.Number,
  total_steps_in_plan: Schema.Number,
  session_start_time: Schema.String,
  total_time_spent_seconds: Schema.Number,
  llm_calls_made: Schema.Number,
  llm_tokens_used: Schema.Struct({}),
  tools_called: Schema.Number,
  commits_made: Schema.Number
}) { }

export class ErrorState extends Schema.Class<ErrorState>("ErrorState")({
  last_error: Schema.Union(Schema.Struct({}), Schema.Null),
  consecutive_error_count: Schema.Number,
  retry_count_for_current_action: Schema.Number,
  blocked_reason: Schema.Union(Schema.String, Schema.Null)
}) { }

export class LLMConfig extends Schema.Class<LLMConfig>("LLMConfig")({
  model: Schema.String,
  temperature: Schema.Number,
  max_tokens: Schema.Number
}) { }

export class Configuration extends Schema.Class<Configuration>("Configuration")({
  agent_goal: Schema.String,
  llm_config: LLMConfig,
  max_retries_per_action: Schema.Number,
  allowed_actions: Schema.Array(Schema.String),
  restricted_paths: Schema.Array(Schema.String),
  action_timeout_seconds: Schema.Number,
  session_timeout_minutes: Schema.Number,
  github_token_available: Schema.Boolean
}) { }

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
}) { }