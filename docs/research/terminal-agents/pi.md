# Pi terminal-agent tool layer study

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-28
Source studied: `earendil-works/pi` public `main` branch, especially
`packages/coding-agent` and `packages/agent`.

This note studies Pi as input for OpenAgents' own built-in Khala desktop and
CLI tools. It focuses on the tool layer only: catalog, model-facing definitions,
execution, permissions, result shaping, and patterns worth harvesting for our
Bun + Effect + Effect Schema implementation.

## Executive summary

Pi's strongest pattern is a small, model-legible built-in tool catalog backed by
one definition module per tool. Each tool definition includes:

- a stable model-facing name such as `read`, `bash`, `edit`, or `write`;
- a TypeBox parameter schema with per-field descriptions;
- a short natural-language description;
- optional prompt snippets and prompt guidelines used in the default system
  prompt;
- an `execute` implementation returning typed content plus optional tool-specific
  details;
- optional terminal renderers for compact or expanded UI output.

The design is minimal but not toy-like. Search and shell output are aggressively
bounded; reads include continuation hints; edits use exact replacements with
diff previews; bash output streams partial updates and preserves full truncated
output in temp files.

The major gap for Khala is authority. Pi explicitly says it runs with the
permissions of the launching user/process by default and punts stronger
filesystem/process/network boundaries to containerization or extensions. Khala
should adopt Pi's small tool shapes and result formatting, but make permission
policy and sandbox routing first-class, Effect-modeled runtime services rather
than optional extension examples.

## Tool catalog

Pi's built-in catalog is centralized in
`packages/coding-agent/src/core/tools/index.ts`.

The complete named set is:

- `read`
- `bash`
- `edit`
- `write`
- `grep`
- `find`
- `ls`

`ToolName` is the union `"read" | "bash" | "edit" | "write" | "grep" | "find" |
"ls"`, and `allToolNames` is a matching `Set`. The module exposes constructor
pairs for every tool, for example `createReadToolDefinition` and
`createReadTool`.

Pi has three useful preset groupings:

- `createCodingToolDefinitions`: `read`, `bash`, `edit`, `write`.
- `createReadOnlyToolDefinitions`: `read`, `grep`, `find`, `ls`.
- `createAllToolDefinitions`: all seven tools as a named record.

That split is worth copying. Khala desktop should have a small default coding
set, a safe read-only set for inspect-only sessions, and an explicit all-tools
set for trusted local owner sessions.

## Tool definitions and model presentation

Pi uses TypeBox schemas inside each tool module. Examples:

- `packages/coding-agent/src/core/tools/read.ts` defines `readSchema` with
  `path`, optional 1-indexed `offset`, and optional `limit`.
- `packages/coding-agent/src/core/tools/bash.ts` defines `bashSchema` with
  `command` and optional `timeout`.
- `packages/coding-agent/src/core/tools/edit.ts` defines `editSchema` with
  `path` and an `edits` array of `{ oldText, newText }`; `additionalProperties:
  false` is set on the edit object and top-level schema.
- `packages/coding-agent/src/core/tools/write.ts` defines `writeSchema` with
  `path` and `content`.
- `packages/coding-agent/src/core/tools/grep.ts` defines `pattern`, optional
  `path`, `glob`, `ignoreCase`, `literal`, `context`, and `limit`.
- `packages/coding-agent/src/core/tools/find.ts` defines `pattern`, optional
  `path`, and optional `limit`.
- `packages/coding-agent/src/core/tools/ls.ts` defines optional `path` and
  optional `limit`.

`packages/coding-agent/src/core/extensions/types.ts` defines the reusable
`ToolDefinition` shape. Important fields:

- `name`: LLM tool-call name.
- `label`: human-readable UI label.
- `description`: model-facing tool description.
- `promptSnippet`: optional one-line summary for the default "Available tools"
  system-prompt section.
- `promptGuidelines`: optional bullets appended when the tool is active.
- `parameters`: TypeBox schema.
- `prepareArguments`: compatibility shim before schema validation.
- `executionMode`: optional sequential/parallel override.
- `execute`: the real implementation.
- `renderCall` and `renderResult`: terminal UI renderers.

`packages/coding-agent/src/core/system-prompt.ts` builds the default prompt from
the active tools. Only tools with a `promptSnippet` appear in the compact
"Available tools" list, and active tools contribute their `promptGuidelines`.
That keeps the model prompt aligned with the actual exposed tool set rather than
hardcoding a static list.

For Khala, the Effect Schema equivalent should preserve the same separation:
the tool registry should own schema, description, prompt snippet, policy class,
and renderer metadata together. Prompt construction should derive from active
tool definitions, not from a separate prompt copy that can drift.

## Execution model

Pi's low-level execution loop lives in `packages/agent/src/agent-loop.ts`.

The high-level flow:

1. Stream an assistant message from the model.
2. Extract `toolCall` content blocks from the assistant message.
3. Choose sequential execution when the global mode is sequential or any target
   tool declares `executionMode: "sequential"`; otherwise prepare calls in source
   order and execute allowed calls concurrently.
4. For each call, find the named tool in `currentContext.tools`.
5. Run `prepareArguments` if present.
6. Validate arguments with `validateToolArguments(tool, preparedToolCall)` from
   `@earendil-works/pi-ai/compat`.
7. Call the optional `beforeToolCall` hook. A hook may block and return a
   model-visible error result.
8. Execute `tool.execute(toolCall.id, args, signal, onUpdate)`.
9. Convert thrown errors into error tool results.
10. Call optional `afterToolCall` to replace content, details, `isError`, or
    `terminate`.
11. Emit `tool_execution_start`, `tool_execution_update`,
    `tool_execution_end`, then append a `role: "toolResult"` message with
    `toolCallId`, `toolName`, `content`, `details`, `isError`, and `timestamp`.

Parallel mode keeps final tool-result messages in assistant source order after
all executions settle, while `tool_execution_end` events are emitted as each
tool finalizes. That is a useful UI/runtime distinction: live UI can show
completion order, while the model sees deterministic source order.

`packages/coding-agent/src/core/tools/tool-definition-wrapper.ts` wraps
definition-first tools into the lower-level `AgentTool` shape and can synthesize
a minimal `ToolDefinition` from a plain `AgentTool`. That keeps extension and SDK
callers flexible without giving up a definition-first registry internally.

## Tool-specific behavior

### read

`packages/coding-agent/src/core/tools/read.ts` supports text and common image
types. For text files it:

- resolves paths relative to the current working directory through
  `resolveReadPathAsync`;
- checks readability;
- treats `offset` as 1-indexed;
- applies a user `limit` before default truncation;
- truncates from the head with `truncateHead`;
- returns continuation hints such as the next `offset` when more content remains;
- handles the edge case where the first line alone exceeds the byte limit by
  pointing the model at a targeted `bash` fallback.

Images are processed through `processImage`, optionally resized, and omitted with
a note when the active model lacks image input support.

The UI renderer has a compact classification for `AGENTS.md`, `CLAUDE.md`,
`SKILL.md`, and Pi docs/resources so common instruction reads do not flood the
terminal unless expanded.

### bash

`packages/coding-agent/src/core/tools/bash.ts` executes shell commands through a
pluggable `BashOperations` interface. The default backend:

- verifies the working directory exists;
- uses Pi's shell config and environment;
- spawns a detached process group on non-Windows systems;
- streams stdout and stderr into one output accumulator;
- supports an optional timeout;
- kills the process tree on abort or timeout;
- tracks detached child PIDs.

The tool definition says output is truncated to the last default line/byte limit
and that full truncated output is saved to a temp file. During execution it
streams throttled partial updates through `onUpdate`, then returns final output
or throws an error containing the captured output plus status text such as
timeout or nonzero exit code.

`packages/coding-agent/src/core/tools/output-accumulator.ts` is the key support
class. It maintains bounded decoded tail text, counts lines/bytes, writes a temp
file when output exceeds limits, and snapshots tail-truncated content for live
updates.

### edit

`packages/coding-agent/src/core/tools/edit.ts` is exact replacement, not patch
application. The model sends one or more replacements:

```json
{
  "path": "src/file.ts",
  "edits": [{ "oldText": "exact original", "newText": "replacement" }]
}
```

Important behavior:

- `prepareEditArguments` accepts legacy `oldText`/`newText` inputs and parses
  stringified `edits` arrays seen from some models.
- Validation requires a non-empty `edits` array.
- Edits are matched against the original normalized file, not incrementally.
- The tool strips BOM before matching, detects line endings, normalizes to LF,
  applies edits, then restores the original line endings.
- Mutations run under `withFileMutationQueue(absolutePath, ...)` so overlapping
  edits/writes to the same file serialize.
- The returned `details` include a display diff, unified patch, and first changed
  line.
- The renderer computes a preview diff once complete args are available.

The prompt guidelines are unusually concrete and worth copying: exact old text
must match uniquely, related nearby changes should be merged into one edit, and
large unchanged padding should be avoided.

### write

`packages/coding-agent/src/core/tools/write.ts` writes complete file content,
creates parent directories, and overwrites existing files. Like `edit`, it uses
`withFileMutationQueue` per absolute path and checks aborts after each awaited
filesystem operation so the mutation queue is not released while a write may
still complete.

The prompt guideline says to use `write` only for new files or complete rewrites.
That is a useful model-facing division: targeted changes should use `edit`;
whole-file creation/replacement should use `write`.

### grep

`packages/coding-agent/src/core/tools/grep.ts` wraps ripgrep:

- ensures `rg` is available via `ensureTool("rg", true)`;
- runs `rg --json --line-number --color=never --hidden`;
- supports `ignoreCase`, fixed-string mode, globs, context lines, and a match
  limit;
- respects `.gitignore`;
- uses custom operations for path checks and file reads when injected;
- truncates long matching lines to `GREP_MAX_LINE_LENGTH`;
- truncates total output by bytes;
- returns notices when match limits, byte limits, or line truncation occur.

No-match is a successful tool result with "No matches found", while rg errors
other than normal no-match become error tool results.

### find

`packages/coding-agent/src/core/tools/find.ts` wraps `fd` by default and supports
a custom `glob` operation for alternate backends. It:

- resolves the search path under the current working directory;
- returns relative POSIX-style paths;
- respects `.gitignore`;
- applies a result limit;
- detects whether it is inside a git repo to decide whether `fd` should require
  git ignore behavior;
- uses `--full-path` and rewrites path-containing patterns with `**/` when
  needed;
- returns "No files found matching pattern" as a successful empty result.

### ls

`packages/coding-agent/src/core/tools/ls.ts` is intentionally simple:

- optional `path`, optional `limit`;
- verifies path exists and is a directory;
- sorts entries case-insensitively;
- appends `/` to directories;
- includes dotfiles;
- caps entries and byte output;
- returns "(empty directory)" for empty directories.

This is a good example of a tool that avoids forcing the model to spend a shell
call just to inspect one directory.

## Permissions, sandboxing, and approvals

Pi has hooks but not a built-in authority model.

`packages/agent/src/types.ts` defines `beforeToolCall` and
`afterToolCall`. `beforeToolCall` receives the assistant message, raw tool call,
validated args, and current context. It can return `{ block: true, reason }`.
`afterToolCall` can replace the result. `packages/coding-agent/src/core/agent-session.ts`
installs these hooks and routes them through extension events named `tool_call`
and `tool_result`.

The example `packages/coding-agent/examples/extensions/permission-gate.ts`
shows a pattern-based bash guard. It watches `tool_call`, detects commands like
recursive remove, `sudo`, and permissive chmod/chown, then prompts in
interactive mode or blocks by default in non-interactive mode.

The explicit containerization guide at
`packages/coding-agent/docs/containerization.md` says Pi runs with all
permissions by default and recommends three isolation patterns:

- route built-in tools and user `!` commands into the Gondolin local micro-VM
  extension;
- run the whole Pi process in Docker;
- run the whole Pi process in NVIDIA OpenShell.

This is the main design point Khala should not copy as-is. Khala needs a typed
authority boundary in the default product. Shell, write, network, credential,
and repo-mutation policies should be first-class Effect services with explicit
policy decisions, not extension-only regex checks.

## Result formatting and error handling

Pi consistently returns model-visible tool results as content arrays plus
tool-specific details:

- `read` returns text or image content and optional truncation details.
- `bash` returns stdout/stderr text and details containing truncation/full output
  path when applicable.
- `edit` returns success text plus diff/patch/first-changed-line details.
- `grep`, `find`, and `ls` return text plus details indicating limits or
  truncation.

Errors are normalized by the agent loop. Unknown tool, validation errors,
blocked calls, aborts, thrown tool exceptions, nonzero bash exits, and timeout
failures all become `toolResult` messages with `isError: true` and text content.

Output limits are centralized in
`packages/coding-agent/src/core/tools/truncate.ts`:

- default max lines: 2000;
- default max bytes: 50 KiB;
- grep max match line length: 500 characters;
- read/find/grep/ls usually keep the head;
- bash keeps the tail.

This head-vs-tail distinction is exactly right for a coding agent: file reads
usually need starts and continuation offsets, while shell failures usually need
the final lines.

## Architecture patterns worth harvesting

1. Definition-first tools. Pi keeps schema, description, prompt snippet,
   prompt-guidelines, execution, and renderer metadata together. Khala should do
   the same with Effect Schema.

2. Small default catalog. `read`, `bash`, `edit`, and `write` are enough for a
   capable default coding loop. `grep`, `find`, and `ls` are valuable read-only
   convenience tools that reduce shell usage and improve model ergonomics.

3. Read-only vs coding presets. Pi's `createReadOnlyToolDefinitions` and
   `createCodingToolDefinitions` map cleanly to Khala modes.

4. Pluggable operations per tool. `ReadOperations`, `BashOperations`,
   `WriteOperations`, `EditOperations`, `GrepOperations`, `FindOperations`, and
   `LsOperations` let Pi route the same model-facing tool through alternate
   backends. Khala can use this pattern to support local desktop, CLI, remote
   workspace, and sandbox backends without changing the model schema.

5. File mutation queue. Serializing writes/edits per absolute path is small and
   high-leverage. Khala should preserve that invariant.

6. Streaming partial shell output with bounded memory. `OutputAccumulator` is a
   good shape for Khala's desktop UX and CLI progress reporting.

7. Actionable truncation notices. "Use offset=N to continue", "increase limit",
   "refine pattern", and "use read for full lines" are better than opaque
   truncation banners.

8. Deterministic model-visible ordering. Parallel execution can help latency,
   but tool results should be appended in assistant source order unless a model
   protocol explicitly supports another ordering.

9. Separate UI rendering from model result content. Pi can compact terminal
   display without hiding content from the model-level result contract.

## Risks and gaps for OpenAgents

- Regex permission examples are not enough for Khala. They are helpful demos,
  not a product authority boundary.
- Bash is too broad as a first-class primitive unless paired with typed policy,
  sandbox routing, timeouts, environment redaction, and output controls.
- Temp-file paths for full bash output are useful locally but must not leak into
  public traces, issue comments, or product proofs.
- `write` overwrites existing files by design. Khala should consider an
  additional expected-state guard or require edit/patch for existing files in
  non-danger modes.
- TypeBox is fine for Pi, but OpenAgents should use Effect Schema so tool
  contracts compose with the rest of the monorepo.

### What OpenAgents should adopt / avoid (for Khala desktop + CLI tools)

Adopt:

- A small default built-in catalog: `read`, `edit`, `write`, `bash`, plus
  read-only `grep`, `find`, and `ls`.
- Definition-first tool modules containing Effect Schema parameters,
  descriptions, prompt snippets, policy class, execution service, renderer
  metadata, and tests.
- Derived system-prompt tool sections from the active registry, not duplicated
  prompt text.
- Tool presets: inspect/read-only, coding-local, and owner-danger-local.
- Per-tool backend interfaces so the same schema can target local filesystem,
  desktop sandbox, CLI workspace, or future remote workspace.
- Exact-replacement edit semantics with diff preview and unified patch details.
- Per-path mutation queues for `edit` and `write`.
- Head truncation for reads/searches and tail truncation for shell output.
- Continuation hints and limit-refinement hints in model-visible results.
- Streaming shell updates for desktop and CLI, with bounded memory and final
  canonical output accounting.
- Before/after hooks, but as typed policy/evidence services rather than the only
  safety mechanism.

Avoid:

- Shipping Pi's default "runs with launcher permissions" model as Khala's
  default behavior.
- Treating regex command guards as sufficient approval/sandboxing.
- Letting raw shell output, temp paths, private prompts, credentials, or local
  repo paths enter public traces or product proofs.
- Making `bash` the model's only way to search/list/read; dedicated read-only
  tools are safer and produce cleaner context.
- Allowing prompt text to drift from the tool registry.
- Exposing write/overwrite tools in read-only or untrusted sessions.

Recommended Khala implementation shape:

- `ToolDefinition<Schema, Details>` in Effect terms:
  `name`, `label`, `description`, `schema`, `promptSnippet`,
  `promptGuidelines`, `policy`, `executionMode`, `execute`, `render`.
- `ToolPolicy` as a required field, for example `readOnly`, `filesystemWrite`,
  `shell`, `network`, `credential`, `ownerDangerLocal`.
- `ToolBackend` services for local desktop, CLI workspace, and sandboxed
  workspace.
- `ToolResult` with `content`, `details`, `isError`, `visibility`, and
  redaction metadata so local UX and public evidence cannot accidentally share
  the same raw payload.
