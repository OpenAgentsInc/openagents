# OpenCode tool-layer study for Khala built-in tools

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Issue: [#6953](https://github.com/OpenAgentsInc/openagents/issues/6953)

Source studied: `anomalyco/opencode` public `dev` branch at commit
[`01a5c69244c8c683bee303e535e3e3d40c605b8f`](https://github.com/anomalyco/opencode/tree/01a5c69244c8c683bee303e535e3e3d40c605b8f)
(2026-06-29). The repository is the current home reached from the historical
`sst/opencode` name.

Purpose: extract concrete tool-layer patterns that should inform OpenAgents'
own built-in tool definitions for Khala desktop and the Khala CLI when no
external coding agent is installed. This is research only; it does not implement
OpenAgents tools.

## Executive takeaways

OpenCode is the closest studied terminal agent to OpenAgents' preferred stack:
Bun plus Effect, with model tools described and executed through Effect Schema.
The most harvestable pattern is a small typed core:

- each tool is a value made by `Tool.make(...)` with a model-facing
  description, Effect Schema input, Effect Schema output, optional structured
  output projection, and an Effect-based `execute` handler;
- each tool registers into a `Tools.Service`, while `ToolRegistry.Service`
  materializes the active tool definitions and settles tool calls;
- permission checks are not embedded in the model schema. Tools call
  `PermissionV2.Service.assert(...)` at the side-effect boundary using action,
  resources, source message/call refs, and optional save scopes;
- large outputs are bounded by `ToolOutputStore.Service.bound(...)`, with a
  preview returned to the model and the full content saved under managed local
  tool-output storage;
- mutating file tools are deliberately conservative: exact text edit, full-file
  write, and patch application are separate tools, and all share the `edit`
  permission class.

For Khala, this argues for a first-party catalog that is small, typed, and
service-composed. Avoid making shell the only primitive. Avoid letting dynamic
MCP or project tools blur the authority of built-ins. Keep model output,
structured output, user display, and durable audit refs separate from the start.

## Tool catalog

The built-in catalog is composed in
[`packages/core/src/tool/builtins.ts`](https://github.com/anomalyco/opencode/blob/01a5c69244c8c683bee303e535e3e3d40c605b8f/packages/core/src/tool/builtins.ts).
`locationLayer` merges these shipped tools:

- `apply_patch`
- `bash`
- `edit`
- `glob`
- `grep`
- `question`
- `read`
- `skill`
- `todowrite`
- `webfetch`
- `websearch`
- `write`

That list is intentionally Location-scoped. The same file wires the tool layer
to shared services: `ToolRegistry`, filesystem utilities, process execution,
configuration, current `Location`, `LocationMutation`, `FileMutation`,
permissions, ripgrep, image processing, questions, skills, session todos, and
HTTP client.

The design point for Khala is the static/dynamic split. OpenCode's built-ins
are first-class and service-backed. Dynamic or application tools are handled
elsewhere through `ApplicationTools` and `ToolRegistry`; they do not replace the
core catalog. Khala should follow that: ship a small reliable built-in catalog,
then layer MCP/project/plugin tools on top with clear names and policy.

## Tool definitions and model-facing schemas

The core definition constructor is
[`packages/core/src/tool/tool.ts`](https://github.com/anomalyco/opencode/blob/01a5c69244c8c683bee303e535e3e3d40c605b8f/packages/core/src/tool/tool.ts).
`Tool.make(...)` accepts:

- `description`: model-facing instructions;
- `input`: an Effect `Schema.Codec`;
- `output`: an Effect `Schema.Codec`;
- optional `structured` output schema;
- optional `toStructuredOutput(...)`;
- `execute(...)`, returning an `Effect`;
- optional `toModelOutput(...)`.

The runtime converts Effect Schema to JSON Schema with
`Schema.toJsonSchemaDocument(...)` and exposes the result as an
`@opencode-ai/llm` `ToolDefinition`. Tool input is decoded before execution;
tool output is encoded after execution; both failure cases become
`ToolFailure` messages rather than raw thrown errors.

The companion LLM package has the same general shape in
[`packages/llm/src/tool.ts`](https://github.com/anomalyco/opencode/blob/01a5c69244c8c683bee303e535e3e3d40c605b8f/packages/llm/src/tool.ts):
typed tools use Effect Schemas for parameters and success values, while dynamic
tools can accept raw JSON Schema. This is the right layering for OpenAgents:
our built-ins should be Effect Schema-first, while MCP/plugin bridges can remain
dynamic and explicitly less trusted.

Concrete schema examples:

- `read` in
  [`packages/core/src/tool/read.ts`](https://github.com/anomalyco/opencode/blob/01a5c69244c8c683bee303e535e3e3d40c605b8f/packages/core/src/tool/read.ts)
  accepts `path`, `offset`, and `limit`. The field descriptions say offset is a
  1-based directory entry or text line offset and that limit bounds entries or
  lines.
- `bash` in
  [`packages/core/src/tool/bash.ts`](https://github.com/anomalyco/opencode/blob/01a5c69244c8c683bee303e535e3e3d40c605b8f/packages/core/src/tool/bash.ts)
  accepts `command`, optional `workdir`, and optional `timeout`, with a maximum
  timeout enforced in schema.
- `edit` in
  [`packages/core/src/tool/edit.ts`](https://github.com/anomalyco/opencode/blob/01a5c69244c8c683bee303e535e3e3d40c605b8f/packages/core/src/tool/edit.ts)
  accepts `path`, exact `oldString`, `newString`, and optional `replaceAll`.
- `write` in
  [`packages/core/src/tool/write.ts`](https://github.com/anomalyco/opencode/blob/01a5c69244c8c683bee303e535e3e3d40c605b8f/packages/core/src/tool/write.ts)
  accepts `path` and full `content`.
- `grep` and `glob` in
  [`packages/core/src/tool/grep.ts`](https://github.com/anomalyco/opencode/blob/01a5c69244c8c683bee303e535e3e3d40c605b8f/packages/core/src/tool/grep.ts)
  and
  [`packages/core/src/tool/glob.ts`](https://github.com/anomalyco/opencode/blob/01a5c69244c8c683bee303e535e3e3d40c605b8f/packages/core/src/tool/glob.ts)
  accept pattern/path/limit shapes and delegate to ripgrep.

Khala should copy the "schema plus description in one definition" pattern. The
schema should be the contract. Prompt text should be derived from active tools,
not maintained as a separate static list.

## Registry and materialization

[`packages/core/src/tool/registry.ts`](https://github.com/anomalyco/opencode/blob/01a5c69244c8c683bee303e535e3e3d40c605b8f/packages/core/src/tool/registry.ts)
is the key runtime service. It exposes:

- `register(tools)`: a scoped registration capability. Registrations are removed
  by a finalizer when the scope ends.
- `materialize(permissions?)`: builds the current `ToolDefinition[]` and returns
  a `settle(...)` function for calls.

Materialization merges application tools and local scoped registrations. It also
removes tools that are wholly disabled by the provided permission ruleset. The
settlement path resolves the tool by call name, rejects unknown or stale tool
calls with typed model-visible errors, runs the tool through `Tool.settle(...)`,
then sends output through `ToolOutputStore.bound(...)`.

Two details are especially relevant to Khala:

- stale tool calls are detected by comparing an advertised registration identity
  with the current registration identity;
- output bounding is centralized after execution, so every tool benefits from
  the same line/byte policy and managed-output reference behavior.

## Execution model

The low-level LLM runtime path in
[`packages/llm/src/tool-runtime.ts`](https://github.com/anomalyco/opencode/blob/01a5c69244c8c683bee303e535e3e3d40c605b8f/packages/llm/src/tool-runtime.ts)
is intentionally narrow: `dispatch(tools, call)` finds the named tool, decodes
input, executes the handler, encodes output, projects it into a `ToolOutput`,
and emits canonical tool-result or tool-error events.

The core session runner resolves models through
[`packages/core/src/session/runner/model.ts`](https://github.com/anomalyco/opencode/blob/01a5c69244c8c683bee303e535e3e3d40c605b8f/packages/core/src/session/runner/model.ts).
It supports OpenAI Responses, Anthropic Messages, and OpenAI-compatible chat
routes, including `@ai-sdk/openai-compatible` models with custom base URLs.
That matters for Khala because OpenCode already treats provider routing and
tool execution as separable concerns: model resolution produces an LLM route,
while tools remain local typed services.

History lowering in
[`packages/core/src/session/runner/to-llm-message.ts`](https://github.com/anomalyco/opencode/blob/01a5c69244c8c683bee303e535e3e3d40c605b8f/packages/core/src/session/runner/to-llm-message.ts)
keeps assistant tool calls and local tool results in the LLM conversation. If a
provider executed the tool itself, provider metadata can be reused; otherwise
local tool output is converted into a `ToolResultPart`. Khala should preserve
this distinction because local tools, provider-hosted tools, and future MCP
tools need different authority and audit treatment.

## Permissions, sandboxing, and approvals

The authority boundary is
[`packages/core/src/permission.ts`](https://github.com/anomalyco/opencode/blob/01a5c69244c8c683bee303e535e3e3d40c605b8f/packages/core/src/permission.ts).
Permission rules are wildcard-matched by action and resource. Evaluation falls
back to `ask` when no rule matches. The service supports:

- `ask(input)`: evaluate and, if needed, create a pending permission request;
- `assert(input)`: allow, deny, or block until the pending request is answered;
- `reply(input)`: accept, reject, or accept always;
- `get`, `forSession`, and `list` for pending requests.

Saved "always" replies are stored through `PermissionSaved` as allow rules for
the current project. If an operator rejects a request, the implementation also
rejects other pending requests in the same session. If an operator grants
"always", the service re-evaluates matching pending requests and releases the
ones now covered by saved rules.

Tool-specific permission behavior:

- `read` asks for action `read` on the resolved resource. External absolute
  paths require a separate `external_directory` approval from
  `LocationMutation`.
- `grep` and `glob` ask on the search pattern and include metadata such as root,
  path, include, and limit.
- `write`, `edit`, and `apply_patch` all use permission action `edit`, which
  keeps file mutation under one policy class.
- `bash` asks on action `bash` with the command string as the resource. Its
  description is explicit that it executes with the host user's filesystem,
  process, and network authority. Absolute command-argument paths outside the
  active location produce warnings, but the scan is advisory.
- `todowrite` asks on action `todowrite` with resource `*`, even though it only
  mutates session todo state.

The sandboxing lesson is mixed. The permission architecture is strong and easy
to port. The current `bash` implementation is honest about being host-user
authority, and its TODOs explicitly leave deeper command parsing, sandbox
expansion, background jobs, and durable live progress as future work. Khala
should adopt the permission model, but not treat host-user shell as the final
sandbox story. The desktop and CLI should have explicit modes such as
read-only, workspace-write, and owner-full-access.

## File tools

### `read`

`read` resolves paths through `LocationMutation`, checks external-directory
approval, inspects the target, and either lists a directory page or reads a
file page. Text paging is delegated to `ReadToolFileSystem`; supported images
are normalized through `Image.Service` and returned as a file content part.
Unsupported binary files become a tool failure.

The model-facing description is doing real work: it tells the model that
relative paths resolve from the current Location, absolute paths inside the
Location are accepted, external absolute paths need approval, and large text can
be paged.

### `edit`

`edit` is exact replacement. It rejects identical old/new strings and empty
`oldString`. It normalizes line endings, preserves BOM handling, counts exact
occurrences, requires `replaceAll` when there are multiple matches, generates a
diff preview, and writes through `FileMutation.writeIfUnchanged(...)` with the
original bytes as the expected content. If the file changed after approval, the
tool returns a specific "read it again" failure.

This is a strong default for Khala. Exact replacement is easier to audit than a
fuzzy patcher, and stale-content protection should be mandatory for local
desktop tools where users may edit files concurrently.

### `write`

`write` writes full content through `FileMutation.writeTextPreservingBom(...)`
after resolving the target and asserting `edit` permission. The output says
whether the file was created or overwritten, and includes both target and
resource fields.

Khala should expose write separately from edit. Models learn different behavior
when a tool means "overwrite all content" versus "replace one exact span."

### `apply_patch`

`apply_patch` parses a full patch, rejects empty patches, rejects moves, resolves
all targets before reading target contents, collects external-directory
approvals, asserts one `edit` permission across the affected resources, prepares
all changes, and then applies them sequentially. It documents that rollback is
not atomic: earlier operations remain applied if a later operation fails.

For Khala, this is useful but should not be the only write primitive. Patch
application is excellent for agent-authored multi-file changes, but exact edit
and full write remain simpler and easier for models to recover from.

## Search tools

`grep` and `glob` delegate to `Ripgrep.Service`. They keep path arguments
relative to the active Location, expose a `limit`, and return concise
line-oriented model output. `grep` formats results as file headers plus
`Line N: text`; `glob` returns matched paths or "No files found."

The pattern to copy is first-class search, not shelling out to `grep` or `find`.
Search tools should be bounded, structured internally, and concise externally.
Khala should include at least file glob and content grep in the built-in catalog
so the model does not spend shell authority on routine code navigation.

## Shell tool

`bash` is intentionally small in the V2 core. It defines:

- default timeout: 120,000 ms;
- maximum timeout: 600,000 ms;
- maximum in-memory capture: 1 MiB;
- optional working directory resolved from the active Location;
- configured shell override from config, otherwise `/bin/sh` on POSIX and
  COMSPEC/cmd on Windows;
- detached process group on non-Windows;
- combined stdout/stderr capture;
- timeout handling that returns a model-visible timeout message.

The tool returns structured fields `exit`, `truncated`, and optional `timeout`,
with text output plus a concise status line for the model. It does not currently
provide the more advanced behavior some agents have, such as parser-based
command narrowing, robust background job observation, or full durable output
streaming. The TODO block in the file names those gaps explicitly.

Khala should copy the bounded timeout/capture defaults and the honest authority
description. It should avoid shipping a shell tool that silently implies a
sandbox it does not enforce.

## Result formatting and error handling

OpenCode separates three result layers:

- `structured`: encoded data that can be retained and rendered by clients;
- `content`: model-facing text or file parts;
- `result`: the provider-facing tool result value produced by
  `ToolOutput.toResultValue(...)`.

`Tool.make(...)` lets a tool customize `toModelOutput(...)` and
`toStructuredOutput(...)`. The registry then applies global output bounding in
[`packages/core/src/tool-output-store.ts`](https://github.com/anomalyco/opencode/blob/01a5c69244c8c683bee303e535e3e3d40c605b8f/packages/core/src/tool-output-store.ts).
Default limits are 2,000 lines and 50 KiB. Oversized output is written to a
managed `tool-output` directory for seven days, while the model gets a bounded
head/tail preview with a marker pointing at the saved local path.

Errors are generally converted into `ToolFailure` and then into model-visible
tool errors. Unknown tools, stale calls, invalid input, invalid output, and
tool-thrown failures all become ordinary tool results rather than crashing the
conversation loop.

Khala should preserve this separation. The desktop UI should be able to render
diffs, paths, output files, and permission requests from structured data, while
the model receives concise factual text.

## Architecture notable points for OpenAgents

OpenCode's useful architecture is not just "it has tools"; it has Effect
services around tool authority:

- `Tool.make` is pure definition plus effectful handler.
- `Tools.Service` is the registration interface.
- `ToolRegistry.Service` is materialization plus settlement.
- `PermissionV2.Service` is an independent approval service.
- `LocationMutation` and `FileMutation` centralize path resolution and mutation
  safety.
- `ToolOutputStore` centralizes truncation and managed-output references.
- `SessionRunnerModel` resolves provider/model transport separately from tools.

That maps cleanly to OpenAgents. A Khala built-in tool package should have
Effect Schema contracts at the boundary, Effect services for IO, and a
registry/materialization step that can produce model-specific JSON Schema
without giving the model direct access to service internals.

The main caveat is that OpenCode's V2 tool layer still has visible parity debt:
the `bash` TODOs cover parser-based approvals, environment augmentation,
durable progress, background jobs, and binary handling; `edit` and `write` TODOs
cover formatter, watcher, undo, and LSP integrations. Khala can use the shape
now, but should not copy every current limitation as a product decision.

## What OpenAgents should adopt / avoid (for Khala desktop + CLI tools)

### Adopt

- Adopt Effect Schema-first tool definitions: one tool value should include
  description, input schema, output schema, structured output projection, model
  output projection, and an Effect handler.
- Adopt a service registry: expose tools through a scoped `register(...)` and a
  `materialize(...)` phase that produces the exact tool definitions available
  to the current model/session.
- Adopt a small built-in catalog: `read`, `glob`, `grep`, `edit`, `write`,
  `apply_patch`, `bash`, `question`, and `todowrite` are enough for a first
  local coding agent.
- Adopt first-class search tools so routine navigation does not require shell
  authority.
- Adopt exact replacement editing with stale-content protection as the default
  file mutation path.
- Adopt separate permission actions for read/search, edit/write/patch, shell,
  external directories, question/user input, and todo/session metadata.
- Adopt output bounding globally, with structured output retained and a bounded
  preview returned to the model.
- Adopt typed stale/unknown/invalid tool-call errors as normal tool results.
- Adopt explicit source refs on permission requests: session id, assistant
  message id, tool call id, action, resources, metadata, and save scope.

### Avoid

- Avoid making `bash` the primary file/search/edit interface. Shell should be
  available for tests and real commands, not for every basic operation.
- Avoid pretending host-user shell execution is sandboxed. If Khala runs with
  host authority, say so and gate it; if it promises workspace-write or
  read-only, enforce that in services.
- Avoid fuzzy edit correction as the first public contract. Start exact and
  make recovery strategies internal, explainable, and auditable.
- Avoid one shared string result for all consumers. Model output, structured UI
  data, audit metadata, and managed-output refs should stay distinct.
- Avoid allowing MCP, project, or plugin tools to shadow core built-ins without
  policy visibility and namespacing.
- Avoid unbounded tool output in the conversation history. Large command output
  should be stored as local managed output with a bounded head/tail preview.
- Avoid saving broad "always allow" permissions without action and resource
  scopes. Saved rules must stay project/session appropriate.

## Suggested first Khala implementation slice

1. Define an `@openagentsinc/khala-tools` package with Effect Schema contracts
   and `Tool.make`-style constructors.
2. Implement `read`, `glob`, `grep`, `edit`, `write`, and `bash` first; keep
   `apply_patch`, `question`, and `todowrite` next.
3. Add a `ToolRegistry` service that materializes active definitions and rejects
   unknown/stale calls with typed results.
4. Add a `Permission` service before any mutating or shell tool ships in the
   desktop app.
5. Add a `ToolOutputStore` before exposing shell, so long test output does not
   poison model context or UI rendering.
6. Keep provider/model transport separate from local tool execution, matching
   OpenCode's split between `SessionRunnerModel` and tool settlement.
