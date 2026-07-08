# Hermes Agent tool-layer study for Khala built-in tools

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Issue: [#6956](https://github.com/OpenAgentsInc/openagents/issues/6956)

Source studied: `NousResearch/hermes-agent` at commit
[`10043c6d0cd942487f7ef94231e22d91e1734a20`](https://github.com/NousResearch/hermes-agent/tree/10043c6d0cd942487f7ef94231e22d91e1734a20).

Purpose: extract concrete tool-layer patterns that should inform OpenAgents'
own built-in tools for Khala desktop and the Khala CLI when no external coding
agent is installed.

## Executive takeaways

Hermes Agent is a Python-first, registry-driven terminal agent. Its key
tool-layer pattern is: each tool module self-registers a model-facing OpenAI
function schema plus a handler into `tools.registry`; `model_tools.py` resolves
active toolsets into a filtered schema list; `agent/tool_executor.py` parses
tool calls, applies middleware and guardrails, then dispatches sequentially or
concurrently.

The strongest ideas for OpenAgents are:

- keep tool schemas, handlers, availability checks, metadata, and per-tool
  output budgets together at registration time;
- expose a small coding core as first-class tools (`read_file`, `write_file`,
  `patch`, `search_files`, `terminal`, `process`) rather than making shell the
  only primitive;
- route all tool calls through one execution layer that can apply policy,
  checkpoints, middleware, progress callbacks, output persistence, and
  deterministic result ordering;
- treat large tool results as first-class artifacts by spilling them to a
  readable file with a preview, instead of silently truncating context.

The main caution for Khala is that Hermes' system evolved into a large dynamic
surface with many platform and plugin tools. Khala's built-in tool layer should
start narrower and more typed, using Effect Schema for the contract, while
leaving MCP/plugin expansion behind an explicit capability boundary.

## Tool catalog

Hermes groups tools in `toolsets.py`. The default shared core list
`_HERMES_CORE_TOOLS` includes:

- Web: `web_search`, `web_extract`.
- Terminal and process management: `terminal`, `process`, plus desktop-only
  `read_terminal`.
- File manipulation: `read_file`, `write_file`, `patch`, `search_files`.
- Vision and image generation: `vision_analyze`, `image_generate`.
- Skills: `skills_list`, `skill_view`, `skill_manage`.
- Browser automation: navigate, snapshot, click, type, scroll, back, press,
  images, vision, console, CDP, and dialog tools.
- Planning and memory: `todo`, `memory`.
- Session recall and interaction: `session_search`, `clarify`.
- Code execution and delegation: `execute_code`, `delegate_task`.
- Scheduling and integrations: `cronjob`, Home Assistant, kanban, and computer
  use when their checks pass.

The code-focused presets are more useful for Khala than the full platform
bundles. `toolsets.py` defines `file` as `read_file`, `write_file`, `patch`,
and `search_files`; `terminal` as `terminal` plus `process`; `coding` as a
workspace posture containing files, terminal, web docs, skills, todo, memory,
session search, clarify, execute-code, delegate, vision, and browser tools; and
`hermes-acp` as an editor integration set without broad messaging/audio tools.

The pattern to copy is the separation between individual tool categories,
scenario presets, and platform presets. Khala should have a small default
coding set, a read-only inspection set, and an explicit owner-local full set.

## Tool definitions and model-facing schemas

Tool definitions are OpenAI-style function declarations. `tools/registry.py`
stores each `ToolEntry` with `name`, `toolset`, `schema`, `handler`,
`check_fn`, environment requirements, async flag, description, emoji,
`max_result_size_chars`, and optional dynamic schema overrides. Tool modules
call `registry.register(...)` at import time, and `discover_builtin_tools()`
imports only modules with top-level `registry.register(...)` calls.

The file tools in `tools/file_tools.py` are representative:

- `READ_FILE_SCHEMA` describes `read_file` as a line-numbered, paginated text
  read. Parameters are `path`, 1-indexed `offset`, and `limit` capped at 2000
  lines. The description tells the model to use it instead of `cat/head/tail`,
  says output is `LINE_NUM|CONTENT`, warns about a roughly 100K character cap,
  and names auto-extracted formats such as notebooks, Word, and Excel.
- `WRITE_FILE_SCHEMA` takes `path`, full `content`, and a `cross_profile`
  escape hatch. The description is explicit that it overwrites the whole file,
  creates parent directories, and should be replaced by `patch` for targeted
  edits.
- `PATCH_SCHEMA` supports `mode: "replace" | "patch"`. Replace mode takes
  `path`, `old_string`, `new_string`, and `replace_all`; patch mode takes a V4A
  patch string. The description tells the model to prefer it over `sed/awk` and
  says the implementation uses fuzzy matching and returns a unified diff.
- `SEARCH_FILES_SCHEMA` combines grep and file-find behavior through
  `target: "content" | "files"`, plus `path`, `file_glob`, `limit`, `offset`,
  `output_mode`, and `context`. The model-facing description says it is
  ripgrep-backed and should replace shell `grep/rg/find/ls`.

`tools/terminal_tool.py` defines `TERMINAL_SCHEMA` with `command`,
`background`, `timeout`, `workdir`, `pty`, `notify_on_complete`, and
`watch_patterns`. Its descriptions are unusually operational: foreground
timeouts are bounded, background bounded jobs are expected to set
`notify_on_complete`, and `watch_patterns` are discouraged unless they are rare
signals from long-lived processes.

Hermes also rebuilds some schemas dynamically. In `model_tools.py`,
`get_tool_definitions()` reconstructs `execute_code` so its description lists
only the tools actually available in the current session, and adjusts Discord
schemas based on detected runtime privileges. Khala should preserve this idea:
schema text must be derived from active capabilities, not from stale static
prompt copy.

## Toolset resolution and availability

`model_tools.py` is the schema provider. It accepts enabled and disabled
toolsets, resolves names through `toolsets.resolve_toolset()`, subtracts
disabled sets, then asks `registry.get_definitions(...)` for the model-facing
function list.

Availability is checked at registry time through each tool's `check_fn`.
`tools/registry.py` caches these checks for 30 seconds and suppresses one-off
false results for a short grace window after a recent success. This prevents a
flaky Docker or terminal backend probe from silently stripping critical tools
mid-session.

After filtering, `model_tools.py` sanitizes schemas for backend compatibility
and optionally applies Tool Search progressive disclosure. The comment in that
file says core Hermes tools are never deferred, while MCP/plugin tools can be
hidden behind `tool_search`, `tool_describe`, and `tool_call` when their schema
surface is too large. Khala should adopt the principle but keep the boundary
crisper: built-ins should always be directly visible; external tools should be
discoverable, prefixed, and policy-scoped.

## Execution model

Tool calls are executed in `agent/tool_executor.py`. The concurrent path shows
the full contract:

1. Parse the model's JSON arguments.
2. Unwrap Tool Search bridge calls only if the underlying tool is in the
   session's granted scope.
3. Apply request middleware.
4. Run plugin pre-call blocks and loop guardrails.
5. Take checkpoints before file mutations or destructive terminal commands.
6. Emit progress callbacks.
7. Execute runnable calls in a thread pool, propagating context variables and
   thread-local approval/sudo callbacks.
8. Collect results in the original tool-call order before appending tool
   result messages for the model.

`agent/tool_dispatch_helpers.py` decides when batches can run concurrently.
Read-only tools such as `read_file`, `search_files`, `web_search`,
`web_extract`, `session_search`, and skill viewing are parallel-safe. File
mutators are path-scoped and can run concurrently only when their paths do not
overlap. Terminal commands default to sequential unless the system can prove
they are safe; destructive command patterns include `rm`, `rmdir`, `cp`, `mv`,
`sed -i`, `truncate`, `dd`, `shred`, `git reset`, `git clean`, `git checkout`,
and overwrite redirection.

Actual dispatch goes through `tools.registry.ToolRegistry.dispatch()`. Missing
tools return `{"error": "Unknown tool: ..."}`. Async handlers are bridged by
`model_tools._run_async()`. Exceptions are caught, sanitized, and returned as
JSON tool errors instead of escaping the agent loop.

Khala should copy the single execution choke point. Tool implementations should
not decide policy or result persistence on their own; they should return typed
results into a central executor that owns ordering, approvals, observability,
and failure shape.

## Permissions, sandboxing, and approvals

Hermes has several layers:

- `tools/terminal_tool.py` supports local, Docker, Singularity, SSH, Modal, and
  Daytona-style terminal backends. `check_terminal_requirements()` gates schema
  exposure on the selected backend's availability.
- `tools/terminal_tool.py` validates `workdir` with an allowlist regex, caps
  foreground timeout, supports PTY mode for local/SSH, and can run background
  tasks with process notifications.
- `tools/approval.py` is the single source of truth for dangerous command
  detection, per-session approval state, plugin approval hooks, a frozen
  `HERMES_YOLO_MODE` read, hardline blocklists, sensitive path patterns, and
  permanent approval allowlists.
- `acp_adapter/permissions.py` bridges dangerous-command approvals into the
  Agent Client Protocol permission request flow.
- `acp_adapter/edit_approval.py` builds pre-execution edit proposals for
  `write_file` and `patch`, including old/new text, and maps editor approval
  responses back into Hermes' gate.
- `tools/write_approval.py` separately gates persistent memory and skill writes
  by staging pending records for approval, especially for background review
  writes where inline prompting is impossible.

The most important design point is that Hermes treats command approval,
editor-edit approval, and memory/skill write approval as different policies.
Khala should do the same. A binary "tools allowed" flag is too blunt; file
edits, shell commands, networked browser actions, and persistent memory writes
need separate Effect-modeled authority decisions.

## Result formatting and error handling

Hermes returns tool results as strings, usually JSON strings for structured
tools. The file tools shape their own output:

- `read_file` returns line-numbered text and blocks device paths like
  `/dev/zero`, `/dev/random`, `/dev/stdin`, `/dev/tty`, `/dev/stdout`, and
  file-descriptor aliases before reading.
- `search_files` redacts sensitive text from matched content, paginates by
  `offset` and `limit`, appends a next-offset hint when truncated, and warns or
  blocks when the model repeats the exact same search too many times.
- `write_file` and `patch` validate missing or malformed arguments explicitly
  before touching disk, which helps models recover from dropped arguments under
  context pressure.

The broader output budget lives in `tools/tool_result_storage.py` and
`tools/budget_config.py`. Hermes uses three layers:

1. Tool-local caps such as search pagination.
2. Per-result persistence: if a result exceeds its threshold, write the full
   output to `/tmp/hermes-results/<tool_call_id>.txt` or the active sandbox temp
   directory, then return a `<persisted-output>` block with a preview and a
   `read_file` instruction.
3. Per-turn aggregate budgeting: if the total tool output in one turn exceeds
   the configured budget, spill the largest results until the turn is under
   budget.

`read_file` is pinned to never persist, which avoids a persist-read-persist
loop. Budgets scale down for small context-window models while preserving the
historic 100K per-result and 200K per-turn defaults for large models.

Khala should adopt this artifact-first output model. A local desktop agent can
show full logs in UI panes while sending the model a bounded preview plus a
stable local artifact ref.

## Architecture notable points for OpenAgents

`tools/registry.py` is the core abstraction: one registration point owns schema,
handler, toolset, availability, UI emoji, dynamic overrides, and output budget.
In OpenAgents, this should be an Effect Schema-backed `ToolDefinition` with:

- `name`, `displayName`, `description`, and optional model-family prompt text;
- `inputSchema` and `outputSchema`;
- `authorityClass` such as read-only, workspace-write, shell, network, browser,
  memory-write, or persistent-config-write;
- `availability` as an Effect that can fail with a typed reason;
- `execute` as an Effect returning a typed result or typed tool error;
- `render` metadata for Khala desktop.

`toolsets.py` is useful but too stringly typed. Khala should model toolsets as
data with explicit composition and policy tags, not just arrays of names.

`agent/tool_executor.py` shows why the executor should be separate from tools:
the executor is where checkpoints, policy, loop guardrails, middleware,
parallelism, progress, and result persistence naturally belong.

Hermes' dynamic Tool Search is a good answer to huge extension surfaces, but
Khala should not need it for the built-ins. First-party coding tools should
remain directly callable; discovered MCP/plugin tools can use progressive
disclosure later.

## What OpenAgents should adopt / avoid (for Khala desktop + CLI tools)

Adopt:

- A first-party coding catalog: `read_file`, `write_file`, `patch`,
  `search_files`, `terminal`, `process`, plus optional `todo` and browser/web
  tools.
- Definition-first tools where schemas, descriptions, policy class,
  availability, handler, renderer metadata, and output budget are registered
  together.
- Toolset presets for read-only, coding, and owner-local full authority.
- A central executor that parses, validates, gates, checkpoints, executes,
  budgets output, and returns deterministic model messages.
- Path-scoped parallelism for file tools and conservative sequential behavior
  for shell and mutating tools.
- Per-result and per-turn output persistence with model-visible previews and
  local artifact refs.
- Separate approval classes for shell commands, file edits, persistent memory
  writes, and external/plugin tools.
- Dynamic schema text generated from active capabilities, so the model never
  sees unavailable tools in descriptions.

Avoid:

- A giant default tool surface. Hermes supports many platforms and integrations,
  but Khala's no-external-agent built-ins should start with the narrow coding
  core and require explicit opt-in for browser, cron, memory, and integrations.
- String-only policy decisions. Use Effect Schema enums and typed decisions
  instead of ad hoc name lists wherever possible.
- Treating shell as the universal file API. Hermes correctly tells the model to
  prefer file/search tools over `cat`, `grep`, `sed`, and heredocs; Khala should
  do the same.
- Silent truncation. Large outputs should become inspectable artifacts.
- Letting plugin/MCP tools shadow built-ins. External tools should be prefixed,
  scoped to a granted toolset, and unable to replace core tools without an
  explicit override path.
- A single "approval mode" for every side effect. Shell, edits, memory, config,
  network, and browser control need distinct authority boundaries.
