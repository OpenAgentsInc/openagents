# OpenAI Codex tool-layer study for Khala built-in tools

Issue: [#6964](https://github.com/OpenAgentsInc/openagents/issues/6964)

Source studied: `openai/codex` public `main` branch at commit
[`ccdfb4f342a2e659be7ab878309cc5d81683d737`](https://github.com/openai/codex/tree/ccdfb4f342a2e659be7ab878309cc5d81683d737).

Purpose: extract concrete tool-layer patterns from OpenAI Codex's Rust coding
agent that should inform OpenAgents' own built-in tool definitions for Khala
desktop and the Khala CLI when no external coding agent is installed. This is
research only; it does not implement OpenAgents tools.

## Executive takeaways

Codex's current Rust tool layer is worth studying less for a large catalog of
file primitives and more for its execution boundary. It exposes a compact set
of model-visible tools, then routes calls through typed Rust handlers, hooks,
approval checks, sandbox transforms, streaming events, and structured protocol
items.

The most harvestable patterns are:

- model-visible tools are represented as `ToolSpec` variants that can become
  OpenAI Responses API functions, namespaces, hosted tools, deferred tools, or
  custom freeform tools;
- concrete schemas are plain JSON Schema values built in handler-specific spec
  files, while shared conversion helpers live in the extracted
  `codex-rs/tools` crate;
- the active tool catalog is feature-, model-, and environment-sensitive, so
  Codex can expose `exec_command` plus `write_stdin`, legacy `shell_command`,
  `apply_patch`, MCP resource tools, collaboration tools, dynamic tools,
  hosted web search, and image generation only when they make sense;
- every local tool implements a `CoreToolRuntime` handler and dispatches
  through `ToolRegistry`, which applies pre-tool hooks, optional input rewrite,
  telemetry, result logging, post-tool hooks, lifecycle events, and typed
  model-visible errors;
- shell and patch execution share a sandbox/approval framework that separates
  request parsing from policy decisions, cached approvals, runtime sandbox
  selection, managed network enforcement, and platform-specific sandbox
  wrappers;
- `apply_patch` is a custom freeform grammar tool, not a JSON function, and its
  handler verifies patch syntax before applying or delegating to the filesystem
  runtime;
- command output is streamed as protocol events and also returned as bounded
  model-visible output, with hard byte/event caps to avoid poisoning the agent
  loop.

For Khala, the big lesson is to design built-ins as a stable typed host layer,
not as prompt-only tool descriptions. Use Effect Schema instead of Rust structs,
but copy the shape: declarative spec, central registry, tool-specific runtime,
policy hooks, sandbox attempts, structured result lanes, and bounded streaming.

## Tool catalog

The active catalog is assembled in
[`codex-rs/core/src/tools/spec_plan.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/core/src/tools/spec_plan.rs).
`add_tool_sources(...)` layers tools from several sources:

- shell tools;
- MCP resource tools;
- core utilities;
- collaboration and multi-agent tools;
- MCP runtime tools;
- extension tools;
- dynamic tools;
- hosted model tools such as web search or image generation.

The core shell branch is feature-sensitive. `add_shell_tools(...)` exposes
`exec_command` and `write_stdin` when unified exec is selected, keeps the legacy
`shell_command` handler registered for dispatch compatibility, or exposes only
`shell_command` for older shell modes. If no execution environment is attached,
shell tools are not exposed.

The core utility branch adds:

- `update_plan` through `PlanHandler`;
- `wait_for_environment` when deferred execution is enabled;
- `request_user_input` when the experimental user-input feature is enabled;
- `request_permissions` when an environment exists and the feature is enabled;
- `new_context_window` and `get_context_remaining` when token-budget tooling is
  enabled;
- `current_time` and optional `sleep`;
- plugin discovery / install request helpers;
- `apply_patch` when the model metadata says an apply-patch tool is supported;
- `view_image` whenever an environment exists.

MCP resource helpers are separate built-ins:
`list_mcp_resources`, `list_mcp_resource_templates`, and `read_mcp_resource`.
MCP runtime tools and dynamic tools are added later and can be namespaced,
deferred, or routed through tool search.

This is a useful shape for Khala. The built-in catalog should be stable, but
the visible subset should depend on current mode, project/environment
availability, local permissions, model family, and whether external MCP or
plugin tools are attached.

## Tool definitions and model-facing schemas

The shared host-side tool model lives in
[`codex-rs/tools/src/tool_spec.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/tools/src/tool_spec.rs).
`ToolSpec` serializes to Responses API-compatible tool JSON and supports:

- `Function(ResponsesApiTool)`;
- `Namespace(ResponsesApiNamespace)`;
- `ToolSearch`;
- hosted `image_generation`;
- hosted `web_search`;
- custom freeform tools.

The common function shape in
[`codex-rs/tools/src/responses_api.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/tools/src/responses_api.rs)
contains `name`, `description`, `strict`, optional `defer_loading`,
`parameters`, and optional `output_schema`. `ToolDefinition` in
[`codex-rs/tools/src/tool_definition.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/tools/src/tool_definition.rs)
is the lower-level metadata shape that downstream adapters convert into a
Responses API function.

Concrete schema examples:

- `exec_command` in
  [`codex-rs/core/src/tools/handlers/shell_spec.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/core/src/tools/handlers/shell_spec.rs)
  accepts `cmd`, `workdir`, `tty`, `yield_time_ms`, `max_output_tokens`,
  optional `shell`, optional `login`, optional `environment_id`, and approval
  controls. Its output schema includes elapsed wall time, exit code, optional
  `session_id`, token count, and output text.
- `write_stdin` accepts `session_id`, `chars`, `yield_time_ms`, and
  `max_output_tokens`, allowing long-running unified exec sessions to be
  polled or fed incrementally.
- legacy `shell_command` accepts `command`, `workdir`, `timeout_ms`, optional
  login-shell mode, and the same approval controls.
- `request_permissions` accepts a reason, optional environment id, and a
  permission profile with `network` and `file_system` fields.
- `view_image` in
  [`codex-rs/core/src/tools/handlers/view_image_spec.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/core/src/tools/handlers/view_image_spec.rs)
  accepts a local path, optional detail mode, and optional environment id, and
  returns a data URL plus detail.

`apply_patch` is intentionally different. Its spec in
[`codex-rs/core/src/tools/handlers/apply_patch_spec.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/core/src/tools/handlers/apply_patch_spec.rs)
creates a custom freeform tool named `apply_patch` with a Lark grammar from
`apply_patch.lark`. The model sends raw patch text rather than JSON. In
multi-environment sessions, the grammar is amended to accept an environment id
header.

Khala should keep the same separation, but express it with Effect Schema:
shared tool metadata, model-family adapters, and handler-specific schemas.
Patch is a legitimate exception to JSON-only tools when grammar-constrained
freeform input gives the model a better editing surface.

## Tool search, dynamic tools, and code mode

Codex has first-class support for progressive disclosure. The helper in
[`codex-rs/tools/src/tool_search.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/tools/src/tool_search.rs)
builds searchable text from tool names, descriptions, and schema property
descriptions, then returns deferred `LoadableToolSpec` entries. Deferred tools
drop output schemas and set `defer_loading`.

Dynamic tools are parsed in
[`codex-rs/tools/src/dynamic_tool.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/tools/src/dynamic_tool.rs):
a dynamic function spec becomes a `ToolDefinition` with name, description,
input schema, no output schema, and a `defer_loading` bit. MCP tools follow the
same shared path through `mcp_tool_to_responses_api_tool(...)` and
`mcp_tool_to_deferred_responses_api_tool(...)`.

Code mode is an additional adapter layer in
[`codex-rs/tools/src/code_mode.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/tools/src/code_mode.rs).
It converts plain, namespaced, and freeform specs into `CodeModeToolDefinition`
values and augments descriptions with code-mode-specific samples. Namespaced
tools are flattened as `namespace__tool`.

Khala should copy the policy boundary, not necessarily the exact naming. Core
built-ins should be directly visible. MCP, plugin, project, or app tools can be
deferred and searchable, but they should not shadow first-party tools without a
clear namespace and policy label.

## Execution model

Runtime dispatch centers on
[`codex-rs/core/src/tools/registry.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/core/src/tools/registry.rs).
Each local tool implements `CoreToolRuntime`, which extends the shared
`ToolExecutor<ToolInvocation>` contract with Codex-specific behavior:

- payload kind matching for function, tool-search, or custom payloads;
- optional cancellation behavior;
- telemetry tags;
- optional pre-tool hook payload;
- optional post-tool hook payload;
- optional hook-input rewriting;
- optional streamed argument diff consumption.

`ToolRegistry::dispatch_any_with_terminal_outcome(...)` is the choke point. It:

1. increments active-turn tool-call accounting;
2. looks up the handler by `ToolName`;
3. rejects missing or incompatible tools with model-visible errors;
4. emits tool-start lifecycle notifications;
5. runs `PreToolUse` hooks and applies approved input rewrites;
6. executes the handler under telemetry logging;
7. emits read metrics and post-tool hook payloads;
8. optionally replaces model-visible output with post-hook feedback;
9. records the dispatch trace;
10. returns a `ResponseInputItem` through the tool's `ToolOutput`.

That flow is exactly the kind of central executor Khala needs. Tool handlers
should not each invent their own hook, policy, telemetry, output, and error
logic. They should implement a narrow runtime interface and pass through one
supervised dispatcher.

## Shell and exec tools

Codex has two shell surfaces:

- legacy `shell_command`, a JSON function taking a shell script string;
- unified `exec_command` plus `write_stdin`, which supports PTY sessions,
  polling, live output, and interactive stdin.

Feature selection happens in
[`codex-rs/tools/src/tool_config.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/tools/src/tool_config.rs).
`shell_type_for_model_and_features(...)` checks model metadata and feature
flags to choose disabled, shell command, or unified exec. Unified exec can run
directly or through a zsh-fork mode when the feature set and local shell support
it.

Actual process execution is in
[`codex-rs/core/src/exec.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/core/src/exec.rs).
Important details:

- default shell command timeout is 10 seconds;
- retained shell output is capped through `DEFAULT_OUTPUT_BYTES_CAP`;
- live `ExecCommandOutputDelta` events are capped at 10,000 per exec call;
- stdout/stderr drain after timeout is bounded to avoid hanging on inherited
  file descriptors;
- `ExecParams` carries command argv, cwd, environment, network proxy, sandbox
  permissions, Windows sandbox settings, justification, and optional argv0;
- `process_exec_tool_call(...)` turns params into an `ExecRequest`, then routes
  through the sandboxing module for a single execution path.

Khala should copy the split between shell command description and process
runtime. The model-facing schema should be small. Process groups, PTY sessions,
output caps, cancellation, sandbox transform, and network proxy injection
belong under a service boundary.

## `apply_patch` design

Codex treats patching as a first-class tool and as a special interception case.
The core policy path is in
[`codex-rs/core/src/apply_patch.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/core/src/apply_patch.rs).
`apply_patch(...)` calls `assess_patch_safety(...)`, then returns either:

- an immediate model-visible output/error; or
- `DelegateToRuntime(ApplyPatchRuntimeInvocation)` with the verified action,
  auto-approval flag, and exec approval requirement.

The handler in
[`codex-rs/core/src/tools/handlers/apply_patch.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/core/src/tools/handlers/apply_patch.rs)
handles freeform patch input, streamed patch diffs, verification errors, hook
payloads, runtime delegation, and legacy interception when a shell command is
really an `apply_patch` invocation.

Patch safety is assessed in
[`codex-rs/core/src/safety.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/core/src/safety.rs).
It rejects empty patches, checks approval policy, determines whether the patch
is constrained to writable roots, asks the user when policy requires it, and
rejects writes outside project or read-only sandbox when approval is disallowed.
It also notes a subtle hard-link issue: even writable-looking paths still need
to run through an enforceable sandbox when one is available.

For Khala, `apply_patch` should be a first-party built-in, not just "run a
patch command in shell." Keep grammar validation, multi-file approval keys,
diff events, and runtime sandboxing together.

## Permissions, sandboxing, and approvals

The sandbox policy model is in
[`codex-rs/protocol/src/permissions.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/protocol/src/permissions.rs).
The key primitives are:

- `NetworkSandboxPolicy`: restricted or enabled;
- `FileSystemSandboxKind`: restricted, unrestricted, or external sandbox;
- `FileSystemAccessMode`: read, write, or deny;
- filesystem entries for absolute paths, special paths, globs, project roots,
  tmp dirs, and protected metadata names such as `.git`, `.agents`, and
  `.codex`.

The approval/runtime abstraction is in
[`codex-rs/core/src/tools/sandboxing.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/core/src/tools/sandboxing.rs).
Important pieces:

- `ApprovalStore` caches serialized approval keys for the session.
- `with_cached_approval(...)` skips prompts when all keys are already approved
  and stores per-key approval when the user approves for session.
- `ExecApprovalRequirement` can be skip, needs approval, or forbidden.
- default exec approval depends on `AskForApproval` and whether the filesystem
  sandbox is restricted.
- `SandboxPermissions` can request defaults, additional sandbox permissions, or
  escalated/no-sandbox execution.
- denied-read filesystem policy prevents unsandboxed escalation, because
  dropping the sandbox would also drop read-deny enforcement.
- `Approvable`, `Sandboxable`, and `ToolRuntime` separate approval keys,
  sandbox preference, network approval requests, sandbox cwd, and actual run
  behavior.

The concrete sandbox transform path in `exec.rs` delegates to
`codex-rs/sandboxing`, which has platform implementations for Seatbelt,
bubblewrap, Landlock, and Windows. Managed network access is applied through a
network proxy when sandboxed execution enforces it.

Khala should adopt this as an Effect service graph:

- permissions are data;
- approval keys are typed and cacheable;
- a tool declares its approval and sandbox requirements;
- a runner materializes one or more sandbox attempts;
- public/request wire formats never get to smuggle in "danger" authority.

## Result formatting, truncation, and streaming

Codex separates several result channels:

- model history is `ResponseItem` / `ResponseInputItem` in
  `codex-rs/protocol/src/models.rs`;
- user/session events are `EventMsg` variants in
  [`codex-rs/protocol/src/protocol.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/protocol/src/protocol.rs);
- tool outputs implement `ToolOutput` and can produce response items,
  code-mode results, log previews, and success flags;
- telemetry and rollout traces record previews and structured lifecycle data.

For command output, `exec.rs` streams stdout/stderr chunks as
`ExecCommandOutputDeltaEvent` while still aggregating bounded output for the
tool result. The protocol encodes stream identity and raw bytes so clients can
render live terminal output without pretending it is ordinary assistant text.

The mapper in
[`codex-rs/core/src/client.rs`](https://github.com/openai/codex/blob/ccdfb4f342a2e659be7ab878309cc5d81683d737/codex-rs/core/src/client.rs)
maps provider response streams into internal response events and handles
stream-disconnect cases explicitly. The rollout trace crates store raw and
reduced protocol history as JSONL-style artifacts for replay and debugging.

Khala should preserve the result lanes from day one:

- live UI events for terminal chunks, diff chunks, approval prompts, and tool
  lifecycle;
- concise model-visible tool output;
- structured UI/audit metadata;
- bounded local artifacts for large logs;
- exact failure variants instead of string-only errors.

## Architecture notable points for OpenAgents

The Codex design is a Rust implementation, but the translatable architecture is
clear:

- `codex-rs/tools` is a shared host/tool-spec crate rather than the whole
  runtime.
- `codex-rs/core/src/tools/spec_plan.rs` is the planner for what the model can
  see this turn.
- Handler-specific spec files own concrete schemas.
- `ToolRegistry` is the central executor.
- `CoreToolRuntime` is the local tool handler interface.
- `Approvable` / `Sandboxable` / `ToolRuntime` are the policy/runtime split.
- `apply_patch` uses a grammar-constrained freeform surface and a verified
  runtime path.
- shell execution is a service with output caps, streaming, cancellation, and
  platform sandbox transforms.

For OpenAgents, that maps to a package such as `@openagentsinc/khala-tools`:
Effect Schema contracts for model-facing inputs/outputs, Effect services for
filesystem/process/network/runtime, and a planner that materializes tools for a
given mode and environment.

The caveat is also important: Codex optimizes for a coding-agent surface where
shell and patch cover most file work. Khala's first-party built-ins should add
explicit read/search/edit/write tools as narrow defaults so the model does not
spend shell authority on routine code navigation and file mutation.

## What OpenAgents should adopt / avoid (for Khala desktop + CLI tools)

### Adopt

- Adopt a `ToolSpec`-style host model that can represent functions,
  namespaces, deferred tools, hosted tools, and grammar/freeform tools.
- Adopt Effect Schema-first tool definitions with output schemas where the UI
  or audit layer benefits from structure.
- Adopt a per-turn tool planner that considers mode, model metadata, attached
  environments, feature flags, user permissions, MCP tools, and plugin tools.
- Adopt one `ToolRegistry`-style dispatcher for all local built-ins.
- Adopt pre-tool and post-tool hooks with typed input rewriting and typed
  feedback, not ad hoc string interceptors.
- Adopt a real `apply_patch` freeform tool with grammar validation, patch
  safety checks, multi-file approval keys, streamed diff progress, and sandboxed
  runtime application.
- Adopt separate process-runtime services for shell/exec: PTY, stdin polling,
  process groups, timeouts, cancellation, output caps, and live chunk events.
- Adopt cached per-key approvals so "allow for session" can be scoped to a
  command prefix or file path rather than a blanket tools flag.
- Adopt explicit denied-read preservation. If unsandboxed escalation would
  discard read-deny policy, keep the command sandboxed or reject it.
- Adopt progressive disclosure for external tools, while keeping first-party
  built-ins directly visible.

### Avoid

- Avoid making shell the only built-in interface. Khala should have first-class
  read, glob, grep, edit, write, patch, shell, image, plan, and user-input
  tools.
- Avoid exposing public/request-level danger flags. Owner-local full access can
  exist, but public wire data should express permission requests, not authority
  overrides.
- Avoid broad "approve all tools" state. Persist approvals by action, resource,
  environment, and session/project scope.
- Avoid JSON-only dogma. Patch grammar is a strong example where a custom
  freeform tool is safer and easier for models than a giant escaped JSON
  string.
- Avoid unbounded command output in model history. Stream to UI, store large
  artifacts, and return bounded previews.
- Avoid letting MCP, plugin, or project tools shadow built-ins without
  namespace, source, and policy visibility.
- Avoid mixing model-visible output, UI display, telemetry, and audit blobs into
  one string. They need separate contracts.

## Suggested first Khala implementation slice

1. Define a small `@openagentsinc/khala-tools` contract package with Effect
   Schema `ToolSpec`, `ToolInvocation`, `ToolOutput`, and `ToolEvent` types.
2. Implement direct built-ins first: `read_file`, `list_directory`, `glob`,
   `grep`, `edit`, `write_file`, `apply_patch`, `exec_command`,
   `write_stdin`, `view_image`, `update_plan`, and `request_user_input`.
3. Add a central registry/dispatcher with pre-tool hooks, approval checks,
   lifecycle events, result bounding, and typed error responses.
4. Add permission profiles for read-only, workspace-write, and owner-local full
   access, with denied-read preservation and network gating.
5. Add progressive disclosure only for MCP/plugin/project tools after the
   built-in catalog is stable.
6. Keep shell execution powerful but not magical: process services own PTY,
   stdin, timeouts, streaming, artifacts, sandbox transforms, and network
   proxying.
