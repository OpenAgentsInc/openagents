/**
 * SDK-compatible tool input schemas.
 *
 * These schemas match Claude Agent SDK naming conventions (snake_case)
 * to enable seamless interoperability when Claude Code is available
 * and provide consistent patterns when using the minimal subagent.
 *
 * @module
 */

import * as S from "effect/Schema";

// =============================================================================
// File Operations
// =============================================================================

/**
 * Input schema for file read operations.
 * Matches Claude SDK FileReadInput.
 */
export const FileReadInput = S.Struct({
  file_path: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "Absolute path to the file to read" }),
  ),
  offset: S.optional(
    S.Number.pipe(
      S.int(),
      S.greaterThanOrEqualTo(1),
      S.annotations({ description: "Line number to start reading from (1-indexed)" }),
    ),
  ),
  limit: S.optional(
    S.Number.pipe(
      S.int(),
      S.greaterThanOrEqualTo(1),
      S.annotations({ description: "Maximum number of lines to read" }),
    ),
  ),
});
export type FileReadInput = S.Schema.Type<typeof FileReadInput>;

/**
 * Input schema for file edit operations.
 * Matches Claude SDK FileEditInput with exact string replacement semantics.
 */
export const FileEditInput = S.Struct({
  file_path: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "Absolute path to the file to edit" }),
  ),
  old_string: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "Exact text to find and replace (must match exactly)" }),
  ),
  new_string: S.String.pipe(
    S.annotations({ description: "Text to replace the old_string with" }),
  ),
  replace_all: S.optional(
    S.Boolean.pipe(
      S.annotations({ description: "Replace all occurrences instead of requiring uniqueness (default: false)" }),
    ),
  ),
});
export type FileEditInput = S.Schema.Type<typeof FileEditInput>;

/**
 * Input schema for file write operations.
 * Matches Claude SDK FileWriteInput.
 */
export const FileWriteInput = S.Struct({
  file_path: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "Absolute path to the file to write" }),
  ),
  content: S.String.pipe(
    S.annotations({ description: "Content to write to the file" }),
  ),
});
export type FileWriteInput = S.Schema.Type<typeof FileWriteInput>;

// =============================================================================
// Shell Operations
// =============================================================================

/**
 * Input schema for bash command execution.
 * Matches Claude SDK BashInput.
 */
export const BashInput = S.Struct({
  command: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "The command to execute" }),
  ),
  timeout: S.optional(
    S.Number.pipe(
      S.greaterThan(0),
      S.lessThanOrEqualTo(600000),
      S.annotations({ description: "Timeout in milliseconds (max 600000ms / 10 minutes)" }),
    ),
  ),
  description: S.optional(
    S.String.pipe(
      S.annotations({ description: "Clear, concise description of what this command does (5-10 words)" }),
    ),
  ),
  run_in_background: S.optional(
    S.Boolean.pipe(
      S.annotations({ description: "Run command in background, allowing continued work" }),
    ),
  ),
  dangerously_disable_sandbox: S.optional(
    S.Boolean.pipe(
      S.annotations({ description: "Override sandbox mode (use with caution)" }),
    ),
  ),
});
export type BashInput = S.Schema.Type<typeof BashInput>;

// =============================================================================
// Search Operations
// =============================================================================

/**
 * Output mode for grep operations.
 */
export const GrepOutputMode = S.Literal("content", "files_with_matches", "count");
export type GrepOutputMode = S.Schema.Type<typeof GrepOutputMode>;

/**
 * Input schema for grep/search operations.
 * Matches Claude SDK GrepInput with ripgrep-style parameters.
 */
export const GrepInput = S.Struct({
  pattern: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "Regular expression pattern to search for" }),
  ),
  path: S.optional(
    S.String.pipe(
      S.annotations({ description: "File or directory to search in (defaults to cwd)" }),
    ),
  ),
  glob: S.optional(
    S.String.pipe(
      S.annotations({ description: "Glob pattern to filter files (e.g., '*.ts', '*.{ts,tsx}')" }),
    ),
  ),
  type: S.optional(
    S.String.pipe(
      S.annotations({ description: "File type filter (e.g., 'js', 'py', 'rust')" }),
    ),
  ),
  output_mode: S.optional(
    GrepOutputMode.pipe(
      S.annotations({ description: "Output format: content, files_with_matches, or count" }),
    ),
  ),
  "-i": S.optional(
    S.Boolean.pipe(
      S.annotations({ description: "Case insensitive search" }),
    ),
  ),
  "-n": S.optional(
    S.Boolean.pipe(
      S.annotations({ description: "Show line numbers (default: true for content mode)" }),
    ),
  ),
  "-B": S.optional(
    S.Number.pipe(
      S.int(),
      S.greaterThanOrEqualTo(0),
      S.annotations({ description: "Lines of context before match" }),
    ),
  ),
  "-A": S.optional(
    S.Number.pipe(
      S.int(),
      S.greaterThanOrEqualTo(0),
      S.annotations({ description: "Lines of context after match" }),
    ),
  ),
  "-C": S.optional(
    S.Number.pipe(
      S.int(),
      S.greaterThanOrEqualTo(0),
      S.annotations({ description: "Lines of context before and after match" }),
    ),
  ),
  head_limit: S.optional(
    S.Number.pipe(
      S.int(),
      S.greaterThanOrEqualTo(0),
      S.annotations({ description: "Limit output to first N entries" }),
    ),
  ),
  offset: S.optional(
    S.Number.pipe(
      S.int(),
      S.greaterThanOrEqualTo(0),
      S.annotations({ description: "Skip first N entries before applying head_limit" }),
    ),
  ),
  multiline: S.optional(
    S.Boolean.pipe(
      S.annotations({ description: "Enable multiline mode for patterns spanning lines" }),
    ),
  ),
});
export type GrepInput = S.Schema.Type<typeof GrepInput>;

/**
 * Input schema for glob/file pattern matching.
 * Matches Claude SDK GlobInput.
 */
export const GlobInput = S.Struct({
  pattern: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "Glob pattern to match files (e.g., '**/*.ts')" }),
  ),
  path: S.optional(
    S.String.pipe(
      S.annotations({ description: "Directory to search in (defaults to cwd)" }),
    ),
  ),
});
export type GlobInput = S.Schema.Type<typeof GlobInput>;

// =============================================================================
// Web Operations
// =============================================================================

/**
 * Input schema for web fetch operations.
 * Matches Claude SDK WebFetchInput.
 */
export const WebFetchInput = S.Struct({
  url: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "URL to fetch content from" }),
  ),
  prompt: S.String.pipe(
    S.annotations({ description: "Prompt describing what information to extract from the page" }),
  ),
});
export type WebFetchInput = S.Schema.Type<typeof WebFetchInput>;

/**
 * Input schema for web search operations.
 * Matches Claude SDK WebSearchInput.
 */
export const WebSearchInput = S.Struct({
  query: S.String.pipe(
    S.minLength(2),
    S.annotations({ description: "Search query" }),
  ),
  allowed_domains: S.optional(
    S.Array(S.String).pipe(
      S.annotations({ description: "Only include results from these domains" }),
    ),
  ),
  blocked_domains: S.optional(
    S.Array(S.String).pipe(
      S.annotations({ description: "Exclude results from these domains" }),
    ),
  ),
});
export type WebSearchInput = S.Schema.Type<typeof WebSearchInput>;

// =============================================================================
// Task Operations
// =============================================================================

/**
 * Todo item status.
 */
export const TodoStatus = S.Literal("pending", "in_progress", "completed");
export type TodoStatus = S.Schema.Type<typeof TodoStatus>;

/**
 * A single todo item.
 */
export const TodoItem = S.Struct({
  content: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "Task description in imperative form" }),
  ),
  status: TodoStatus,
  activeForm: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "Present continuous form shown during execution" }),
  ),
});
export type TodoItem = S.Schema.Type<typeof TodoItem>;

/**
 * Input schema for todo list operations.
 * Matches Claude SDK TodoWriteInput.
 */
export const TodoWriteInput = S.Struct({
  todos: S.Array(TodoItem).pipe(
    S.annotations({ description: "The updated todo list" }),
  ),
});
export type TodoWriteInput = S.Schema.Type<typeof TodoWriteInput>;

// =============================================================================
// Agent/Task Operations
// =============================================================================

/**
 * Subagent types available for task delegation.
 */
export const SubagentType = S.Literal(
  "general-purpose",
  "Explore",
  "Plan",
  "claude-code-guide",
  "statusline-setup",
);
export type SubagentType = S.Schema.Type<typeof SubagentType>;

/**
 * Model selection for subagents.
 */
export const ModelSelection = S.Literal("sonnet", "opus", "haiku");
export type ModelSelection = S.Schema.Type<typeof ModelSelection>;

/**
 * Input schema for task/subagent operations.
 * Matches Claude SDK TaskInput.
 */
export const TaskInput = S.Struct({
  description: S.String.pipe(
    S.annotations({ description: "Short (3-5 word) description of the task" }),
  ),
  prompt: S.String.pipe(
    S.annotations({ description: "The task for the agent to perform" }),
  ),
  subagent_type: SubagentType.pipe(
    S.annotations({ description: "Type of specialized agent to use" }),
  ),
  model: S.optional(
    ModelSelection.pipe(
      S.annotations({ description: "Model override for this agent" }),
    ),
  ),
  resume: S.optional(
    S.String.pipe(
      S.annotations({ description: "Agent ID to resume from" }),
    ),
  ),
});
export type TaskInput = S.Schema.Type<typeof TaskInput>;

// =============================================================================
// Notebook Operations
// =============================================================================

/**
 * Cell type for Jupyter notebooks.
 */
export const NotebookCellType = S.Literal("code", "markdown");
export type NotebookCellType = S.Schema.Type<typeof NotebookCellType>;

/**
 * Edit mode for notebook cells.
 */
export const NotebookEditMode = S.Literal("replace", "insert", "delete");
export type NotebookEditMode = S.Schema.Type<typeof NotebookEditMode>;

/**
 * Input schema for Jupyter notebook cell editing.
 * Matches Claude SDK NotebookEditInput.
 */
export const NotebookEditInput = S.Struct({
  notebook_path: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "Absolute path to the Jupyter notebook" }),
  ),
  new_source: S.String.pipe(
    S.annotations({ description: "New source content for the cell" }),
  ),
  cell_id: S.optional(
    S.String.pipe(
      S.annotations({ description: "ID of the cell to edit" }),
    ),
  ),
  cell_type: S.optional(
    NotebookCellType.pipe(
      S.annotations({ description: "Cell type (code or markdown)" }),
    ),
  ),
  edit_mode: S.optional(
    NotebookEditMode.pipe(
      S.annotations({ description: "Edit mode: replace, insert, or delete" }),
    ),
  ),
});
export type NotebookEditInput = S.Schema.Type<typeof NotebookEditInput>;

// =============================================================================
// Background Shell Operations
// =============================================================================

/**
 * Input schema for reading background shell output.
 */
export const BashOutputInput = S.Struct({
  bash_id: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "ID of the background shell" }),
  ),
  filter: S.optional(
    S.String.pipe(
      S.annotations({ description: "Regex to filter output lines" }),
    ),
  ),
});
export type BashOutputInput = S.Schema.Type<typeof BashOutputInput>;

/**
 * Input schema for killing a background shell.
 */
export const KillShellInput = S.Struct({
  shell_id: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "ID of the background shell to kill" }),
  ),
});
export type KillShellInput = S.Schema.Type<typeof KillShellInput>;

// =============================================================================
// User Interaction
// =============================================================================

/**
 * Option for a user question.
 */
export const QuestionOption = S.Struct({
  label: S.String.pipe(
    S.annotations({ description: "Display text for the option (1-5 words)" }),
  ),
  description: S.String.pipe(
    S.annotations({ description: "Explanation of what this option means" }),
  ),
});
export type QuestionOption = S.Schema.Type<typeof QuestionOption>;

/**
 * A single question to ask the user.
 */
export const Question = S.Struct({
  question: S.String.pipe(
    S.annotations({ description: "The question to ask" }),
  ),
  header: S.String.pipe(
    S.annotations({ description: "Short label (max 12 chars)" }),
  ),
  options: S.Array(QuestionOption).pipe(
    S.minItems(2),
    S.maxItems(4),
    S.annotations({ description: "Available choices (2-4 options)" }),
  ),
  multiSelect: S.Boolean.pipe(
    S.annotations({ description: "Allow multiple selections" }),
  ),
});
export type Question = S.Schema.Type<typeof Question>;

/**
 * Input schema for asking user questions.
 */
export const AskUserQuestionInput = S.Struct({
  questions: S.Array(Question).pipe(
    S.minItems(1),
    S.maxItems(4),
    S.annotations({ description: "Questions to ask (1-4)" }),
  ),
  answers: S.optional(
    S.Record({ key: S.String, value: S.String }).pipe(
      S.annotations({ description: "Collected answers" }),
    ),
  ),
});
export type AskUserQuestionInput = S.Schema.Type<typeof AskUserQuestionInput>;

// =============================================================================
// Skill & Command Operations
// =============================================================================

/**
 * Input schema for skill invocation.
 */
export const SkillInput = S.Struct({
  skill: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "Skill name to invoke" }),
  ),
});
export type SkillInput = S.Schema.Type<typeof SkillInput>;

/**
 * Input schema for slash command execution.
 */
export const SlashCommandInput = S.Struct({
  command: S.String.pipe(
    S.minLength(1),
    S.annotations({ description: "Slash command to execute with arguments" }),
  ),
});
export type SlashCommandInput = S.Schema.Type<typeof SlashCommandInput>;

// =============================================================================
// Exports
// =============================================================================

/**
 * All tool input schemas keyed by tool name.
 */
export const ToolInputSchemas = {
  Read: FileReadInput,
  Edit: FileEditInput,
  Write: FileWriteInput,
  Bash: BashInput,
  Grep: GrepInput,
  Glob: GlobInput,
  WebFetch: WebFetchInput,
  WebSearch: WebSearchInput,
  TodoWrite: TodoWriteInput,
  Task: TaskInput,
  NotebookEdit: NotebookEditInput,
  BashOutput: BashOutputInput,
  KillShell: KillShellInput,
  AskUserQuestion: AskUserQuestionInput,
  Skill: SkillInput,
  SlashCommand: SlashCommandInput,
} as const;

export type ToolInputSchemas = typeof ToolInputSchemas;
