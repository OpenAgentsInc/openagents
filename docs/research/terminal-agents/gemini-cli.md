# Gemini CLI tool-layer study for Khala built-in tools

Issue: [#6954](https://github.com/OpenAgentsInc/openagents/issues/6954)

Source studied: `google-gemini/gemini-cli` at commit
[`ae0a3aa7b928cc73bb09604bb9c2c020e6b647db`](https://github.com/google-gemini/gemini-cli/tree/ae0a3aa7b928cc73bb09604bb9c2c020e6b647db).

Purpose: extract concrete tool-layer patterns that should inform OpenAgents'
own built-in tools for Khala desktop and the Khala CLI when no external coding
agent is installed.

## Executive takeaways

Gemini CLI treats tools as a declarative contract plus a validated invocation.
The model-facing function declarations are centralized under
`packages/core/src/tools/definitions/`, while runtime behavior lives in per-tool
classes under `packages/core/src/tools/`. The useful pattern for OpenAgents is
not the exact API shape, but the split:

- a stable tool name, display name, kind, description, and JSON Schema;
- schema validation plus per-tool semantic validation before any side effect;
- a `ToolInvocation` object that can describe affected paths, ask for approval,
  execute, stream progress, and return both model-facing and UI-facing output;
- a scheduler that resolves tool names, builds invocations, runs confirmation
  loops, then executes and serializes results back to the model.

This is directly relevant to Khala because our first-party tool layer should be
usable by local models without Claude/Codex installed, but still have clear
authority boundaries for file writes, shell commands, network, and background
processes.

## Tool catalog

Gemini CLI's core catalog is defined through
`packages/core/src/tools/definitions/coreTools.ts`, which re-exports the tool
names and dispatches to a model-family-specific tool set. The catalog includes:

- File reads: `read_file`, `read_many_files`.
- File writes and edits: `write_file`, `replace`/edit.
- Search and navigation: `glob`, `grep_search`, `list_directory`.
- Shell execution: `run_shell_command`.
- Web and hosted tools: `google_web_search`, `web_fetch`.
- Planning and interaction: `enter_plan_mode`, `exit_plan_mode`, `ask_user`,
  `write_todos`, `update_topic`, `complete_task`.
- Extension/resource tools: MCP resource listing/reading and discovered tools.

`packages/core/src/tools/tool-names.ts` adds display names such as `ReadFile`,
`WriteFile`, `Edit`, `FindFiles`, `ReadFolder`, `SearchText`, `GoogleSearch`,
and `WebFetch`, and it groups write/edit tools in `EDIT_TOOL_NAMES`. It also
defines `TOOLS_REQUIRING_NARROWING` for tools whose approvals must be scoped by
arguments: glob, grep, read-many, read-file, list-directory, write-file, edit,
and shell.

The notable design point is that Gemini CLI does not rely on shell as the only
primitive. It provides first-class, narrow tools for common coding-agent work,
then keeps `run_shell_command` for open-ended actions, tests, package managers,
and other commands.

## Tool definitions and model-facing schema

The definitions layer is explicit JSON Schema, not inferred from TypeScript
types. `packages/core/src/tools/definitions/model-family-sets/gemini-3.ts`
contains a full `CoreToolSet` for Gemini 3 models. Each entry provides:

- `name`: the function name sent to the model.
- `description`: instruction text with usage guidance and constraints.
- `parametersJsonSchema`: an object schema with property descriptions,
  required fields, and simple numeric bounds.

Examples worth copying:

- `read_file` accepts `file_path`, `start_line`, and `end_line`. Its
  description tells the model to use surgical line ranges, warns about automatic
  truncation limits, and states that the tool can handle text plus common image,
  audio, and PDF formats.
- `write_file` accepts `file_path` and full `content`. Its description says it
  overwrites, creates parent directories, and should be reserved for new or
  small files; targeted edits should use `replace`.
- `grep_search_ripgrep` is described as the preferred alternative to
  `run_shell_command("grep ...")`, because it is faster and output-limited.

`packages/core/src/tools/definitions/coreTools.ts` resolves model-specific
definitions via `getToolSet(modelId)`. This lets Gemini CLI keep one runtime
implementation while tuning wording and schemas by model family. Khala should
keep the same escape hatch: schemas should be stable contracts, but the prompt
text around them may need model-family variants.

One specific ergonomic feature appears in
`packages/core/src/tools/tools.ts`: `DeclarativeTool.getSchema()` injects a
common `wait_for_previous` boolean into object-shaped tool schemas. The
description tells the model to set it when a tool depends on previous results
and omit it to allow parallel execution. This is a useful primitive for Khala's
planner because it makes dependency control model-visible without inventing a
separate scheduler DSL.

## Runtime abstraction

The core interfaces live in `packages/core/src/tools/tools.ts`:

- `ToolBuilder` exposes `name`, `displayName`, `description`, `kind`,
  `getSchema()`, `isReadOnly`, `canUpdateOutput`, and `build(params)`.
- `ToolInvocation` holds already-validated params and exposes
  `getDescription()`, optional display title/explanation, `toolLocations()`,
  `shouldConfirmExecute()`, `execute()`, and optional policy narrowing data.
- `ToolResult` separates `llmContent` from `returnDisplay`, with optional
  structured display metadata, `error`, `data`, and a tail-tool-call request.

`BaseDeclarativeTool.build()` first calls `validateToolParams()`, then creates a
tool-specific invocation. Validation is layered: JSON Schema validation is
performed by `SchemaValidator.validate(...)`, then individual tools override
`validateToolParamValues(...)` for semantic checks. This is the right direction
for OpenAgents, but we should express the same split with Effect Schema so the
wire contract, validation errors, and runtime types come from one source.

`ToolResult` is especially important for Khala. Gemini CLI deliberately keeps
model-visible content and user-visible display separate. The model can receive
full factual output, while the UI gets a summarized or structured display. That
is a strong fit for Electrobun: the desktop UI can render diffs, shell output,
and path chips without forcing the model-facing payload to be a UI string.

## Execution path

Tool calls enter the scheduler rather than each tool running itself directly.
`packages/core/src/scheduler/scheduler.ts` performs the batch flow:

1. Resolve each requested tool from the `ToolRegistry`.
2. Build a validated invocation with `tool.build(request.args)`.
3. Populate display metadata from the invocation and display name.
4. Enqueue the call into scheduler state.
5. Run policy and confirmation.
6. Execute the invocation and return function responses to the model.

If a tool name is missing, the scheduler creates a structured
`TOOL_NOT_REGISTERED` error and asks `getToolSuggestion(...)` for a likely
replacement. If validation fails, it creates an `INVALID_TOOL_PARAMS` response
before execution. Khala should mirror this: bad tool calls should be ordinary
typed tool results, not uncaught process errors.

Model responses are parsed in `packages/core/src/core/turn.ts`. The `Turn`
class emits stream events for content, thoughts, tool-call requests, tool-call
confirmations, tool-call responses, citations, retries, errors, and finish
metadata. This preserves the distinction between model streaming and tool
scheduling. For Khala, that suggests a clean event protocol between the local
agent loop and the desktop UI: content chunks, tool requests, approval requests,
live tool output, and tool responses should be separate event variants.

## Registry and extension model

`packages/core/src/tools/tool-registry.ts` stores all known tools by the name
seen by the LLM. Excluded tools remain registered so they can be enabled later.
The registry sorts built-ins before discovered project tools and MCP tools.

The same file supports project-discovered tools through a configured discovery
command. It executes the discovery command, enforces a 10 MB stdout/stderr cap,
expects a JSON array of function declarations, prefixes discovered tool names,
and wraps execution through a configured tool-call command. On failure, the
discovered tool returns stdout, stderr, error, exit code, and signal in the
model-visible content.

The harvestable point is the layering: built-ins are first-class and stable,
while project/MCP tools are additive, prefixed, and policy-visible. Khala should
ship a complete built-in catalog first, then add MCP/project extensions without
letting them shadow or blur core tool authority.

## File tools

`packages/core/src/tools/read-file.ts` resolves file paths relative to the
target directory, validates read access with `config.validatePathAccess(...)`,
then delegates content processing to `processSingleFileContent(...)`. If output
is truncated, `llmContent` includes a clear status line with the shown range,
total line count, and an instruction to call `read_file` again with
`start_line`/`end_line`. The returned display includes the display name,
shortened path, result summary, and text body.

`packages/core/src/tools/write-file.ts` accepts full file content, reads the
current file if it exists, optionally corrects the proposed content, creates a
patch with `diff`, opens an IDE diff when available, and uses edit-style
confirmation details before writing. It also exposes policy update options that
narrow approval to the specific file path.

`packages/core/src/tools/edit.ts` is more complex than a literal replace. It
normalizes line endings, supports exact replacement, flexible whitespace
matching, regex-based recovery, fuzzy recovery, line-ending restoration, diff
generation, and edit correction telemetry. For Khala's first version, the key
lesson is to avoid making `edit` too clever in the public contract. A reliable
literal replace with strong validation and diffs is better than silently
guessing. Recovery strategies can be internal and should surface clear
warnings.

## Shell tool

`packages/core/src/tools/shell.ts` defines `ShellToolParams` with:

- `command`;
- optional `description`;
- optional `dir_path`;
- optional `is_background`;
- optional `delay_ms`;
- optional additional sandbox permissions.

The shell invocation exposes a concise display title, a contextual explanation
with directory and description, and policy update options based on parsed root
commands and redirection. It blocks command substitution patterns before
execution, validates the working directory against workspace access rules,
streams live output through `updateOutput`, limits the retained live-output
buffer to 100,000 characters, detects binary streams, supports inactivity
timeouts, and can background a process after a short delay while returning the
PID and initial output.

Execution runs through `ShellExecutionService.execute(...)`, with a session ID,
pager set to `cat`, sanitization config, sandbox manager, additional permissions,
and background-completion behavior. This is the right level of indirection for
Khala: shell should be one tool, but process management, PTY/session handling,
output sanitization, and sandbox expansion should be services under it.

## Permissions, sandboxing, and approvals

Gemini CLI has several overlapping guardrails:

- Tool kind/read-only metadata in `tools.ts`.
- Path validation via `config.validatePathAccess(...)` in file and shell tools.
- Approval modes in `packages/core/src/policy/types.js`, including default,
  auto-edit, plan, and YOLO-style behavior.
- Scheduler-level policy checks and updates in `packages/core/src/scheduler/`.
- Tool-specific confirmation details for edit diffs, shell commands, and
  sandbox expansion requests.
- Argument narrowing for persistent/session approvals through
  `getPolicyUpdateOptions(...)`.

`BaseToolInvocation.shouldConfirmExecute(...)` first honors auto-edit for tools
that explicitly respect it, then asks the message bus for a policy decision.
Decision outcomes are allow, deny, or ask user. Deny throws before execution;
ask user returns confirmation details. `packages/core/src/scheduler/confirmation.ts`
then runs an interactive confirmation loop that can proceed, cancel, modify via
an external editor, or accept inline modifications from an IDE/TUI.

The shell tool adds sandbox-specific logic. If sandboxing is enabled and a
command is likely to need network or broader filesystem rights, it can generate
a sandbox-expansion confirmation. Approved expansions can be session-only or
persistent.

Khala should adopt the layered model, but with stricter terminology: public wire
requests should never carry "danger" flags. Local desktop settings can choose a
mode, the scheduler can request scoped approval, and any persisted approval
should be narrowed by command prefix, path, and network bit.

## Result formatting and error handling

The `ToolResult` contract in `tools.ts` has three useful lanes:

- `llmContent`: factual content added to model history.
- `returnDisplay`: markdown or display text for the user.
- `display`: structured UI metadata such as tool name, description, summary, and
  typed result body.

Errors are typed with `ToolErrorType`. Parameter validation failures become
`INVALID_TOOL_PARAMS`; missing tools become `TOOL_NOT_REGISTERED`; path issues
become `PATH_NOT_IN_WORKSPACE`; execution exceptions become
`EXECUTION_FAILED`; discovered-tool subprocess failures become
`DISCOVERED_TOOL_EXECUTION_ERROR`.

Large or unsafe outputs are shaped close to the tool:

- `read_file` reports truncation and points the model to line-range follow-up
  calls.
- `shell` streams live output at a bounded cadence, keeps only the tail of the
  live buffer, and switches to binary-progress messages when binary output is
  detected.
- discovered tool discovery has explicit stdout/stderr size caps before JSON
  parsing.

For Khala, each built-in should define a typed success shape and a typed failure
shape. The UI should render the structured display, while the agent loop should
receive concise, bounded, replayable model content.

## Architecture points worth harvesting for OpenAgents

The strongest architectural pattern is the validated invocation object. It is a
small seam between untrusted model arguments and side effects. Once the
invocation exists, it can describe itself, report affected locations, produce an
approval prompt, execute, stream output, and return policy-narrowing hints.

The second strongest pattern is model-family-specific definitions. OpenAgents
can keep the canonical schema in Effect Schema and derive JSON Schema from it,
but descriptions may still need variants for local models, hosted Gemini/OpenAI
models, or smaller models used in Khala offline modes.

The third is a typed event stream. Gemini CLI separates tool-call request,
approval request, tool progress, response, and model content events. Khala
desktop needs that separation so Electrobun can show a real local-agent console
without scraping terminal text.

The fourth is argument-narrowed approval persistence. A user who approves
`bun test` should not accidentally approve every shell command forever. A user
who approves writing one file should not approve all writes. Gemini CLI's
`PolicyUpdateOptions` points in the right direction.

## What OpenAgents should adopt / avoid for Khala desktop + CLI tools

### Adopt

- Ship a first-party core catalog: `read_file`, `read_many_files`, `write_file`,
  `edit_file`, `list_directory`, `glob`, `grep`, `run_shell_command`,
  `web_fetch`, `web_search` where available, `ask_user`, and a small planning
  or todo primitive.
- Define every built-in with Effect Schema, then derive JSON Schema for the
  model and TypeScript types for the executor.
- Split each tool into `Definition`, `Invocation`, and `Executor` layers. The
  invocation should be the only thing allowed to cross from validation into side
  effects.
- Return both model-facing and UI-facing output. Keep `llmContent` concise and
  factual; render richer diffs, shell output, and path summaries through
  structured UI payloads.
- Require path validation for every file operation and shell working directory.
- Make approvals argument-narrowed by default: file path patterns, command
  prefixes, redirection, and network permission should be explicit fields.
- Support live shell output, background processes, cancellation, inactivity
  timeouts, binary-output detection, and output truncation from the first shell
  implementation.
- Add a common scheduler field like `wait_for_previous` so models can express
  dependency without depending on hidden scheduling heuristics.
- Keep built-ins ahead of extensions. MCP and project-discovered tools should be
  additive, namespaced/prefixed, and unable to shadow core tools.

### Avoid

- Do not make shell the default answer for file/search operations. Narrow tools
  produce safer approvals and better bounded output.
- Do not persist broad approvals like "all shell commands" or "all writes"
  without argument narrowing.
- Do not expose owner-local danger/full-access modes as public wire fields.
  Keep them local configuration and reflect only scoped approval state.
- Do not collapse model output and UI output into one string. That makes the
  desktop app harder to render and makes replay/audit noisier.
- Do not rely only on JSON Schema for safety. Add semantic validation for
  workspace paths, line ranges, replacement uniqueness, command substitution,
  redirection, and network/file permission expansion.
- Do not silently apply fuzzy edit recovery without surfacing what changed.
  Start with reliable literal edits and diffs; add recovery only as an explicit,
  inspectable helper.
- Do not let extension discovery failures crash the core tool catalog. Treat
  extension errors as typed diagnostics and keep built-ins available.
