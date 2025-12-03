/**
 * SDK-compatible tool output schemas.
 *
 * These schemas define the output types for all tools, matching
 * Claude Agent SDK conventions for content blocks and result types.
 *
 * @module
 */

import * as S from "effect/Schema";

// =============================================================================
// Content Block Types
// =============================================================================

/**
 * Text content in tool output.
 */
export const TextContent = S.Struct({
  type: S.Literal("text"),
  text: S.String,
});
export type TextContent = S.Schema.Type<typeof TextContent>;

/**
 * Image content in tool output (base64 encoded).
 */
export const ImageContent = S.Struct({
  type: S.Literal("image"),
  data: S.String.pipe(
    S.annotations({ description: "Base64-encoded image data" }),
  ),
  mimeType: S.String.pipe(
    S.annotations({ description: "MIME type (e.g., 'image/png', 'image/jpeg')" }),
  ),
});
export type ImageContent = S.Schema.Type<typeof ImageContent>;

/**
 * Union of all content types that can appear in tool output.
 */
export const ToolContent = S.Union(TextContent, ImageContent);
export type ToolContent = S.Schema.Type<typeof ToolContent>;

// =============================================================================
// Type Guards
// =============================================================================

/**
 * Type guard for TextContent.
 */
export const isTextContent = (content: ToolContent): content is TextContent =>
  content.type === "text";

/**
 * Type guard for ImageContent.
 */
export const isImageContent = (content: ToolContent): content is ImageContent =>
  content.type === "image";

// =============================================================================
// File Operation Outputs
// =============================================================================

/**
 * Output from file read operations.
 */
export const ReadOutput = S.Struct({
  content: S.Array(ToolContent),
  total_lines: S.optional(
    S.Number.pipe(
      S.annotations({ description: "Total lines in the file" }),
    ),
  ),
  lines_returned: S.optional(
    S.Number.pipe(
      S.annotations({ description: "Number of lines actually returned" }),
    ),
  ),
  truncated: S.optional(
    S.Boolean.pipe(
      S.annotations({ description: "Whether output was truncated" }),
    ),
  ),
});
export type ReadOutput = S.Schema.Type<typeof ReadOutput>;

/**
 * Output from file edit operations.
 */
export const EditOutput = S.Struct({
  message: S.String.pipe(
    S.annotations({ description: "Success message describing the edit" }),
  ),
  file_path: S.String.pipe(
    S.annotations({ description: "Path to the edited file" }),
  ),
  replacements: S.Number.pipe(
    S.annotations({ description: "Number of replacements made" }),
  ),
  diff: S.optional(
    S.String.pipe(
      S.annotations({ description: "Unified diff of the changes" }),
    ),
  ),
});
export type EditOutput = S.Schema.Type<typeof EditOutput>;

/**
 * Output from file write operations.
 */
export const WriteOutput = S.Struct({
  message: S.String.pipe(
    S.annotations({ description: "Success message" }),
  ),
  file_path: S.String.pipe(
    S.annotations({ description: "Path to the written file" }),
  ),
  bytes_written: S.optional(
    S.Number.pipe(
      S.annotations({ description: "Number of bytes written" }),
    ),
  ),
});
export type WriteOutput = S.Schema.Type<typeof WriteOutput>;

// =============================================================================
// Shell Operation Outputs
// =============================================================================

/**
 * Output from bash command execution.
 */
export const BashOutput = S.Struct({
  output: S.String.pipe(
    S.annotations({ description: "Combined stdout/stderr output" }),
  ),
  exit_code: S.Number.pipe(
    S.annotations({ description: "Exit code from the command" }),
  ),
  killed: S.optional(
    S.Boolean.pipe(
      S.annotations({ description: "Whether the process was killed (timeout/signal)" }),
    ),
  ),
  shell_id: S.optional(
    S.String.pipe(
      S.annotations({ description: "ID for background shell processes" }),
    ),
  ),
  truncated: S.optional(
    S.Boolean.pipe(
      S.annotations({ description: "Whether output was truncated" }),
    ),
  ),
});
export type BashOutput = S.Schema.Type<typeof BashOutput>;

/**
 * Output from reading background shell.
 */
export const BashOutputResult = S.Struct({
  output: S.String.pipe(
    S.annotations({ description: "New output since last read" }),
  ),
  status: S.Literal("running", "completed", "failed").pipe(
    S.annotations({ description: "Current shell status" }),
  ),
  exit_code: S.optional(
    S.Number.pipe(
      S.annotations({ description: "Exit code if completed" }),
    ),
  ),
});
export type BashOutputResult = S.Schema.Type<typeof BashOutputResult>;

/**
 * Output from killing a shell.
 */
export const KillShellOutput = S.Struct({
  success: S.Boolean,
  message: S.String,
});
export type KillShellOutput = S.Schema.Type<typeof KillShellOutput>;

// =============================================================================
// Search Operation Outputs
// =============================================================================

/**
 * A single match from grep operations.
 */
export const GrepMatch = S.Struct({
  file: S.String,
  line_number: S.optional(S.Number),
  line: S.optional(S.String),
  context_before: S.optional(S.Array(S.String)),
  context_after: S.optional(S.Array(S.String)),
});
export type GrepMatch = S.Schema.Type<typeof GrepMatch>;

/**
 * Output from grep operations in content mode.
 */
export const GrepContentOutput = S.Struct({
  mode: S.Literal("content"),
  matches: S.Array(GrepMatch),
  total_matches: S.Number,
  truncated: S.optional(S.Boolean),
});
export type GrepContentOutput = S.Schema.Type<typeof GrepContentOutput>;

/**
 * Output from grep operations in files_with_matches mode.
 */
export const GrepFilesOutput = S.Struct({
  mode: S.Literal("files_with_matches"),
  files: S.Array(S.String),
  total_files: S.Number,
  truncated: S.optional(S.Boolean),
});
export type GrepFilesOutput = S.Schema.Type<typeof GrepFilesOutput>;

/**
 * Output from grep operations in count mode.
 */
export const GrepCountOutput = S.Struct({
  mode: S.Literal("count"),
  counts: S.Array(S.Struct({
    file: S.String,
    count: S.Number,
  })),
  total: S.Number,
});
export type GrepCountOutput = S.Schema.Type<typeof GrepCountOutput>;

/**
 * Union of all grep output types.
 */
export const GrepOutput = S.Union(GrepContentOutput, GrepFilesOutput, GrepCountOutput);
export type GrepOutput = S.Schema.Type<typeof GrepOutput>;

/**
 * Output from glob operations.
 */
export const GlobOutput = S.Struct({
  files: S.Array(S.String).pipe(
    S.annotations({ description: "Matching file paths" }),
  ),
  total: S.Number.pipe(
    S.annotations({ description: "Total number of matches" }),
  ),
  truncated: S.optional(
    S.Boolean.pipe(
      S.annotations({ description: "Whether results were truncated" }),
    ),
  ),
});
export type GlobOutput = S.Schema.Type<typeof GlobOutput>;

// =============================================================================
// Web Operation Outputs
// =============================================================================

/**
 * Output from web fetch operations.
 */
export const WebFetchOutput = S.Struct({
  content: S.String.pipe(
    S.annotations({ description: "Extracted/processed content from the page" }),
  ),
  url: S.String.pipe(
    S.annotations({ description: "Final URL after redirects" }),
  ),
  title: S.optional(
    S.String.pipe(
      S.annotations({ description: "Page title if available" }),
    ),
  ),
  redirected: S.optional(
    S.Boolean.pipe(
      S.annotations({ description: "Whether the request was redirected" }),
    ),
  ),
});
export type WebFetchOutput = S.Schema.Type<typeof WebFetchOutput>;

/**
 * A single search result.
 */
export const SearchResult = S.Struct({
  title: S.String,
  url: S.String,
  snippet: S.optional(S.String),
});
export type SearchResult = S.Schema.Type<typeof SearchResult>;

/**
 * Output from web search operations.
 */
export const WebSearchOutput = S.Struct({
  results: S.Array(SearchResult),
  query: S.String,
  total_results: S.optional(S.Number),
});
export type WebSearchOutput = S.Schema.Type<typeof WebSearchOutput>;

// =============================================================================
// Task Operation Outputs
// =============================================================================

/**
 * Output from todo write operations.
 */
export const TodoWriteOutput = S.Struct({
  message: S.String,
  todos_count: S.Number,
});
export type TodoWriteOutput = S.Schema.Type<typeof TodoWriteOutput>;

/**
 * Output from task/subagent operations.
 */
export const TaskOutput = S.Struct({
  result: S.String.pipe(
    S.annotations({ description: "Final result from the agent" }),
  ),
  success: S.Boolean,
  turns: S.Number.pipe(
    S.annotations({ description: "Number of conversation turns" }),
  ),
  files_modified: S.optional(
    S.Array(S.String).pipe(
      S.annotations({ description: "Files that were modified" }),
    ),
  ),
  error: S.optional(S.String),
});
export type TaskOutput = S.Schema.Type<typeof TaskOutput>;

// =============================================================================
// Notebook Operation Outputs
// =============================================================================

/**
 * Output from notebook edit operations.
 */
export const NotebookEditOutput = S.Struct({
  message: S.String,
  cell_id: S.optional(S.String),
  notebook_path: S.String,
});
export type NotebookEditOutput = S.Schema.Type<typeof NotebookEditOutput>;

// =============================================================================
// User Interaction Outputs
// =============================================================================

/**
 * Output from ask user question operations.
 */
export const AskUserQuestionOutput = S.Struct({
  answers: S.Record({ key: S.String, value: S.String }),
});
export type AskUserQuestionOutput = S.Schema.Type<typeof AskUserQuestionOutput>;

// =============================================================================
// Generic Tool Result
// =============================================================================

/**
 * Generic tool result wrapper matching SDK ToolResult.
 */
export const ToolResult = <Details extends S.Schema.Any = typeof S.Unknown>(
  detailsSchema?: Details,
) =>
  S.Struct({
    content: S.Array(ToolContent),
    details: S.optional(detailsSchema ?? S.Unknown),
  });

/**
 * Base tool result type.
 */
export const BaseToolResult = ToolResult();
export type BaseToolResult = S.Schema.Type<typeof BaseToolResult>;

// =============================================================================
// Tool Error Types
// =============================================================================

/**
 * Reasons a tool execution can fail.
 */
export const ToolErrorReason = S.Literal(
  "invalid_arguments",
  "not_found",
  "missing_old_text",
  "not_unique",
  "unchanged",
  "command_failed",
  "aborted",
  "permission_denied",
  "timeout",
);
export type ToolErrorReason = S.Schema.Type<typeof ToolErrorReason>;

/**
 * Tool execution error details.
 */
export const ToolError = S.Struct({
  reason: ToolErrorReason,
  message: S.String,
  details: S.optional(S.Unknown),
});
export type ToolError = S.Schema.Type<typeof ToolError>;

// =============================================================================
// Exports
// =============================================================================

/**
 * All tool output schemas keyed by tool name.
 */
export const ToolOutputSchemas = {
  Read: ReadOutput,
  Edit: EditOutput,
  Write: WriteOutput,
  Bash: BashOutput,
  BashOutput: BashOutputResult,
  KillShell: KillShellOutput,
  Grep: GrepOutput,
  Glob: GlobOutput,
  WebFetch: WebFetchOutput,
  WebSearch: WebSearchOutput,
  TodoWrite: TodoWriteOutput,
  Task: TaskOutput,
  NotebookEdit: NotebookEditOutput,
  AskUserQuestion: AskUserQuestionOutput,
} as const;

export type ToolOutputSchemas = typeof ToolOutputSchemas;
