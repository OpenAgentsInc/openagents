# OpenAgents And Khala Terminal Tool Decisions

Date: 2026-06-29

Status: decision record for the next OpenAgents-native Khala terminal-agent
tool runtime.

Sources reviewed:

- `docs/research/terminal-agents/codex.md`
- `docs/research/terminal-agents/gemini-cli.md`
- `docs/research/terminal-agents/hermes-agent.md`
- `docs/research/terminal-agents/opencode.md`
- `docs/research/terminal-agents/pi.md`
- `docs/research/terminal-agents/openagents-current-state.md`

This document turns those studies into product and architecture decisions. The
goal is not to clone Codex, Gemini CLI, Hermes Agent, OpenCode, or Pi. The goal
is to define the OpenAgents/Khala versions of the same tool families: what
should be built as first-party Khala tools, what should stay delegated to
external agents, what should be optional, and what authority boundaries must
exist before the tools become default product behavior.

## Summary Decisions

1. Build a shared OpenAgents-native tool package named
   `@openagentsinc/khala-tools`.
2. Make Khala's built-ins Effect Schema-first, with a central registry,
   materializer, executor, permission service, workspace service, process
   service, output store, and event stream.
3. Ship narrow file/search tools as first-class defaults. Shell is powerful,
   but it is not the file API.
4. Use OpenCode's Effect-shaped architecture as the closest implementation
   pattern, Codex's sandbox/approval/patch discipline as the authority pattern,
   Gemini CLI's validated invocation and UI/model output split as the UX
   pattern, Hermes' artifact output budgeting as the long-output pattern, and
   Pi's small catalog and exact-edit ergonomics as the first-user-facing shape.
5. Reuse existing OpenAgents implementation pieces instead of starting from a
   blank runtime: Probe for browser/PTY/scoped filesystem primitives, Pylon for
   workspace materialization, approval queue, assignment closeouts, and
   public-safe reporting.
6. Keep Codex and Claude SDK lanes as delegated execution lanes. The native
   Khala tool runtime is for Khala Code desktop, Khala CLI, no-external-agent
   local work, fallback execution, and eventually provider-neutral model/tool
   loops.
7. External MCP, plugin, project-discovered, skill, browser, memory, cron, and
   integration tools are not part of the default coding catalog. They are
   explicit, namespaced, policy-scoped additions.

## Product Surfaces

| Surface | Decision |
| --- | --- |
| Khala Code desktop | Becomes the primary visual host for the native Khala tools: chat, tool timeline, diff review, terminal output, approvals, and local artifacts. |
| Khala CLI | Uses the same `@openagentsinc/khala-tools` package in headless mode, with stable JSON output and non-interactive denial defaults. |
| Pylon | Keeps Codex/Claude SDK delegation for own-capacity assignments, then adds native Khala tools as a fallback and as the shared receipt/event shape. |
| Probe | Remains the browser/computer-use/eval substrate. Its filesystem, terminal, browser, tool contract, and timeline primitives should be wrapped or promoted, not duplicated. |
| openagents.com | Does not execute local shell or filesystem tools. It can review, approve, display artifacts, and ingest public-safe summaries/receipts. |

## Package Shape

Create `packages/khala-tools` with these core contracts:

- `ToolDefinition`: name, label, description, prompt snippet, prompt
  guidelines, input schema, output schema, authority class, execution mode,
  availability, renderer metadata, and model-output projection.
- `ToolInvocation`: decoded and semantically validated arguments, affected
  resources, approval request material, and execution plan.
- `ToolResult`: structured output, bounded model output, UI display payload,
  public-safe summary, private artifact refs, redaction metadata, and typed
  error variants.
- `ToolEvent`: model content, tool requested, approval requested, approval
  answered, tool started, tool progress, stdout/stderr chunk, diff chunk,
  artifact written, tool completed, tool failed, and tool cancelled.
- `ToolRegistry`: scoped registration and materialization of active tools for a
  model/session/mode.
- `ToolExecutor`: one choke point for validation, policy, hooks, checkpoints,
  parallelism, execution, output bounding, telemetry, and model-visible
  response construction.

The package should not import desktop UI or Worker product code. It may expose
renderer metadata for clients to map into Electrobun, terminal, or web review
components.

## Naming Decisions

The internal registry ID should be namespaced, for example
`khala.file.read`, but the built-in model-visible names in Khala's default
coding profile should be short and stable:

| Tool family | Model-visible name | Internal class |
| --- | --- | --- |
| File read | `read` | `khala.file.read` |
| Directory list | `ls` | `khala.file.list` |
| Path glob | `glob` | `khala.search.glob` |
| Content grep | `grep` | `khala.search.grep` |
| Exact edit | `edit` | `khala.file.edit` |
| Full write | `write` | `khala.file.write` |
| Multi-file patch | `apply_patch` | `khala.file.apply_patch` |
| Process command | `exec_command` | `khala.process.exec` |
| Process stdin | `write_stdin` | `khala.process.stdin` |
| User question | `ask_user` | `khala.interaction.ask_user` |
| Session todos | `todo_write` | `khala.session.todo_write` |
| Image read | `view_image` | `khala.media.view_image` |
| Web fetch | `web_fetch` | `khala.network.web_fetch` |
| Web search | `web_search` | `khala.network.web_search` |

Adapters may expose compatibility aliases for imported prompts or provider
habits, such as `bash`, `run_shell_command`, `read_file`, `list_directory`,
`find`, `search_files`, or `question`, but aliases should lower into the same
canonical tool definitions and must not create separate authority rules.

External tools must be prefixed or namespaced. MCP, plugin, project-discovered,
and application tools cannot shadow first-party built-ins without an explicit
override path and a visible policy label.

## Tool-By-Tool Decisions

### `read`

Khala's `read` reads files, not directories. It accepts a workspace-relative or
approved absolute path, optional 1-indexed `offset`, and optional `limit`.

Decisions:

- Text reads use head truncation and continuation hints.
- Output includes line numbers in model output when useful, plus structured
  line ranges for UI.
- Common image reads should route to `view_image` or return an image content
  part only when the active model supports images.
- Rich document extraction is not a V1 default. It can be added behind typed
  media processors later.
- Reads outside the active workspace require an `external_directory` approval.
- Device files, pipes, sockets, `/dev/random`, `/dev/zero`, terminal fds, and
  credential paths are blocked by default.

Rejected alternatives:

- Do not combine file read and directory list into one tool. Separate tools
  produce clearer approvals and UI.
- Do not use shell `cat`, `head`, or `tail` as the normal read path.

### `ls`

Khala's `ls` lists one directory page. It accepts optional `path` and `limit`.

Decisions:

- Sort case-insensitively.
- Include dotfiles unless denied by policy.
- Append `/` to directories.
- Return stable structured entries for UI.
- Treat empty directories as successful results.
- Use bounded output and continuation hints.

Rejected alternatives:

- Do not make the model spend shell authority on `ls`.

### `glob`

Khala's `glob` finds paths. It accepts `pattern`, optional `path`, and
optional `limit`.

Decisions:

- Use a ripgrep/fd-backed implementation where available.
- Respect `.gitignore` by default.
- Return POSIX-style workspace-relative paths.
- Provide structured match counts and truncation reason.

Rejected alternatives:

- Do not merge path glob and content grep into one overloaded `search_files`
  primary contract. A compatibility alias may exist, but the native contract is
  two tools.

### `grep`

Khala's `grep` searches content. It accepts `pattern`, optional `path`,
optional `glob`, `ignore_case`, `literal`, `context`, and `limit`.

Decisions:

- Use ripgrep when available.
- Respect `.gitignore` by default.
- Bound per-line length, total matches, and total bytes.
- Return no-match as a successful empty result.
- Include refinement hints when limits are hit.
- Redact credential-looking matches in any public-safe summary.

Rejected alternatives:

- Do not encourage shell `grep`, `rg`, or `find` for routine navigation.

### `edit`

Khala's `edit` is exact replacement, not fuzzy patching. It accepts a target
path and one or more `{ old_text, new_text }` edits.

Decisions:

- Old text must match exactly and uniquely unless `replace_all` is set.
- Normalize line endings for matching and restore the original line endings.
- Preserve BOM where possible.
- Use per-path mutation queues.
- Use stale-content protection: write only if the file still matches the
  content/read version used for planning.
- Produce a diff preview and structured first-changed-line metadata.
- Fuzzy recovery can be an internal assistant helper later, but it must be
  explicit, inspectable, and never silent.

Rejected alternatives:

- Do not ship fuzzy edit correction as the V1 public contract.
- Do not let `edit` overwrite whole files. Use `write` or `apply_patch` for
  that.

### `write`

Khala's `write` writes full file content. It accepts `path`, `content`, and
eventually an expected file version for overwrites.

Decisions:

- It is best for new files and intentional complete rewrites.
- Creating parent directories is allowed in workspace-write mode.
- Overwriting an existing file outside owner-full mode requires an expected
  prior content/version or explicit approval.
- Writes use per-path mutation queues and structured diffs when overwriting.

Rejected alternatives:

- Do not expose `write` in read-only sessions.
- Do not let accidental whole-file overwrite be the default edit strategy.

### `apply_patch`

Khala's `apply_patch` is a first-party grammar/freeform patch tool, not a shell
command. It should accept a constrained patch grammar and apply multi-file
changes after validation.

Decisions:

- Keep it separate from `edit` and `write`.
- Validate grammar before any side effect.
- Resolve all touched paths before writing.
- Ask for one scoped edit approval over the affected resources.
- Emit streamed diff/progress events and a final structured patch receipt.
- State clearly that V1 patch application is not atomic unless an atomic
  backend is implemented.
- Use the UI diff renderer for review, including the `@pierre/diffs` backed
  parser already adopted in `packages/ui`.

Rejected alternatives:

- Do not model patch as JSON with giant escaped strings.
- Do not apply patches by asking the model to run `patch` or heredocs in shell.

### `exec_command`

Khala's `exec_command` is the process/shell tool. It accepts command text or a
future argv shape, optional `workdir`, `timeout_ms`, `yield_time_ms`,
`max_output_tokens`, optional PTY/session mode, and eventually background
execution flags.

Decisions:

- V1 should execute through a process service, not inline tool code.
- Default to workspace cwd.
- Enforce bounded timeout and output capture.
- Stream stdout/stderr chunks to UI while returning a bounded model preview.
- Use tail truncation for shell output.
- Persist full oversized output in a local managed output store, with private
  artifact refs.
- Block or ask for network and external-directory expansion explicitly.
- Detect obvious destructive patterns for approval prompts, but do not rely on
  regex detection as the only safety boundary.
- Be honest about authority. If it runs with host-user authority in owner-full
  mode, the UI and approval record must say so.

Rejected alternatives:

- Do not ship a shell tool that claims sandboxing without enforcement.
- Do not expose owner-full or danger modes as public/request-level fields.

### `write_stdin`

Khala's `write_stdin` writes to or polls an existing interactive process
session created by `exec_command`.

Decisions:

- It is visible only when interactive sessions are enabled.
- It accepts `session_id`, optional `chars`, `yield_time_ms`, and
  `max_output_tokens`.
- It uses the same output bounding, cancellation, and permission context as the
  original process session.

Rejected alternatives:

- Do not overload `exec_command` with hidden stdin semantics.

### `ask_user`

Khala's `ask_user` asks the local operator for information or approval-grade
input that is not an authority grant.

Decisions:

- It is separate from permission prompts.
- It supports non-blocking choices only in interactive desktop/CLI sessions.
- In non-interactive mode it returns a typed unavailable/needs-input result.

Rejected alternatives:

- Do not use `ask_user` as a back door for permissions.

### `todo_write`

Khala's `todo_write` updates session-local plan/todo state.

Decisions:

- It is session state, not persistent memory.
- It is enabled in coding and owner-local modes.
- It emits UI timeline updates but does not claim task completion authority.

Rejected alternatives:

- Do not promote todo state into product promises, payouts, accepted work, or
  public receipts.

### `view_image`

Khala's `view_image` reads a local image for the model/UI.

Decisions:

- It requires read permission on the path.
- It returns an image content part when the model supports it and a structured
  preview payload for the UI.
- It must not include private image bytes in public summaries.

### `web_fetch` And `web_search`

Khala's web tools are optional network tools, not part of the offline/local
coding core.

Decisions:

- They are disabled unless the session has network permission.
- They return structured fetch/search metadata plus bounded model output.
- They should preserve source URLs and timestamps.
- They must not replace repository-local search for code navigation.

### Browser Tools

Khala browser tools should be promoted from Probe only as a separate browser
toolset: navigate, click, type, read text, read DOM, wait, screenshot, and
eventually console/CDP helpers.

Decisions:

- Browser tools are not default coding tools.
- They require a browser authority class distinct from shell and filesystem.
- Screenshots and DOM snapshots produce private artifacts plus public-safe
  summaries.
- They should reuse Probe's Playwright/browser abstractions where possible.

### Memory, Skills, Sessions, Delegation, Cron, And Integrations

These are not V1 core coding tools.

Decisions:

- Persistent memory writes need a separate `memory_write` authority class.
- Skill search/view can be added through progressive disclosure after core
  built-ins are stable.
- Delegation remains Pylon assignment/fleet behavior, not a model-visible
  local tool in the default catalog.
- Cron/scheduling and external integrations require explicit install/config
  and separate policy.

## Tool Presets

| Preset | Enabled tools | Default authority |
| --- | --- | --- |
| `inspect` | `read`, `ls`, `glob`, `grep`, `view_image` | Workspace read-only. External paths ask. No writes, no shell, no network. |
| `coding` | `inspect` plus `edit`, `write`, `apply_patch`, `exec_command`, `ask_user`, `todo_write` | Workspace-write with scoped approvals for writes and shell. Network disabled unless granted. |
| `owner_local_full` | `coding` plus broader shell/filesystem/network as locally configured | Local-only, explicit owner opt-in. Never accepted from public wire/config. |
| `browser` | Browser toolset plus optional web tools | Browser/network authority. Separate from shell and filesystem approvals. |
| `extension` | MCP/plugin/project tools | Namespaced, source-labeled, progressive disclosure by default. |

## Permission Decisions

Khala permissions are data, not prompt text. Use Effect Schema enums and typed
resources for:

- `read`
- `search`
- `edit`
- `write`
- `patch`
- `shell`
- `process_stdin`
- `network`
- `browser`
- `external_directory`
- `memory_write`
- `credential`
- `persistent_config_write`
- `owner_full_access`

Approval records must include session id, assistant message id, tool call id,
action, resources, working directory, network bit, authority mode, save scope,
and public-safety classification.

Saved approvals are scoped by action and resource. "Always allow" means
"always allow this action/resource pattern in this project/session scope", not
"always allow all tools."

Denied-read policy must be preserved. If running unsandboxed would drop a
read-deny boundary, reject or keep the command sandboxed.

## Workspace And Sandbox Decisions

Use Pylon's workspace materializer and workspace-boundary logic as the model for
assignment workspaces. Use Probe's workspace path resolver as the model for
local tool path scoping.

Decisions:

- The default local desktop mode is not Pi's "launcher permissions for
  everything." Default desktop coding mode is workspace-write plus prompts.
- Owner-local full access is available only from local config/UI and should be
  visibly labeled.
- Public OpenAgents requests can ask for work, not for danger authority.
- Codex/Claude delegated lanes may keep their current owner-local execution
  model, but native Khala tools should enforce pre-execution policy instead of
  relying only on post-hoc changed-file checks.

## Output And Event Decisions

Every tool result has four lanes:

1. Bounded model-visible output.
2. Structured UI output.
3. Private local artifact refs for full logs, screenshots, raw DOM, and large
   files.
4. Public-safe summary/receipt material, when a workflow needs it.

Default output limits:

- Text reads and search: head-oriented, 2,000 lines or 50 KiB unless the model
  asks for a smaller range.
- Shell/process output: tail-oriented, 50 KiB model preview, full output stored
  as a private local artifact when oversized.
- Per-turn aggregate output: budgeted, with the largest outputs spilled to
  artifacts first.

Large output is not silently truncated. The model receives a preview, a clear
truncation reason, and instructions for continuation or artifact inspection.

## Existing OpenAgents Reuse

Reuse or promote:

- Probe `ProbeLlmTool`/dispatch contracts as prior art for tool definition and
  execution.
- Probe `fs_read`, `fs_write`, `terminal_run`, browser tools, workspace
  resolver, PTY implementation, and timeline beats.
- Pylon workspace materialization, approval queue, session auto-approval,
  assignment closeouts, active-run tracking, Codex/Claude turn reporters, and
  proof redaction.
- `packages/ui` diff rendering for review surfaces.
- Khala CLI fleet onboarding and Pylon-backed spawn/fleet-run flows for
  delegated work.

Do not reuse as-is:

- Probe's default-allow permission handler for product defaults.
- Codex owner-local full-access assignment execution as the native default.
- Any research-agent regex-only shell guard as the product safety boundary.

## External Agent Decisions

| Studied agent/tool family | OpenAgents/Khala decision |
| --- | --- |
| Codex | Keep as delegated SDK/CLI lane through Pylon. Adopt its `exec_command`, `write_stdin`, `apply_patch`, approval-cache, sandbox, and bounded streaming ideas for native tools. |
| Gemini CLI | Do not depend on it. Adopt validated invocation objects, model-family descriptions, `wait_for_previous`-style dependency hints, and separate model/UI result lanes. |
| Hermes Agent | Do not clone its broad default surface. Adopt artifact-backed long-output handling, availability checks, deterministic result ordering, and distinct approval classes. |
| OpenCode | Use as the closest architectural north star for Effect Schema tools, service registries, permission service, output store, and provider/tool separation. |
| Pi | Use as the first UX shape for a small catalog, exact edit semantics, read-only/coding presets, per-path mutation queues, and actionable truncation hints. Do not copy its default host-permission stance. |
| Current OpenAgents | Consolidate existing Probe and Pylon pieces into a shared package before building new one-off desktop or CLI tools. |

## Implementation Order

1. Create `packages/khala-tools` with `ToolDefinition`, `ToolInvocation`,
   `ToolResult`, `ToolEvent`, `ToolRegistry`, and schema-to-provider adapters.
2. Implement `read`, `ls`, `glob`, and `grep` on top of a workspace service.
3. Add `edit` and `write` with per-path mutation queues, stale-content guards,
   structured diffs, and scoped write approvals.
4. Add `exec_command` with process service, timeouts, live chunk events,
   output store, cancellation, and explicit authority labels.
5. Add `apply_patch` as a grammar/freeform tool with multi-file resource
   approval and structured diff receipts.
6. Add `ask_user`, `todo_write`, and `view_image`.
7. Promote browser tools from Probe as an explicit `browser` preset.
8. Wire Khala Code desktop to render the event stream: messages, approvals,
   diffs, terminal chunks, artifacts, and tool receipts.
9. Wire Khala CLI to the same runtime with JSON/non-interactive modes.
10. Add Pylon fallback/native-tool assignment lane after desktop/CLI semantics
    are stable.

## Non-Decisions

- This record does not choose a model provider.
- This record does not replace Codex/Claude delegated Pylon lanes.
- This record does not authorize public shell execution.
- This record does not make memory, skills, browser control, cron, or external
  integrations default tools.
- This record does not define all UI details for Khala Code desktop. It defines
  the tool/event contract that the UI should consume.

## Acceptance Criteria For The First Native Khala Tool Runtime

The first implementation is acceptable when:

- The model-visible tool catalog is derived from the active registry.
- Every tool input is decoded with Effect Schema and semantically validated
  before side effects.
- File tools enforce workspace boundaries before execution.
- Mutating tools require scoped approval outside trusted local auto-approval
  settings.
- Shell output streams to UI and returns bounded model output.
- Oversized output becomes a private local artifact with a model-visible
  preview and continuation instruction.
- Tool results preserve separate model, UI, private artifact, and public-safe
  lanes.
- External tools are namespaced and cannot shadow built-ins.
- Khala Code desktop and Khala CLI can consume the same tool event protocol.
- Tests cover path escapes, stale edits, output bounding, permission denial,
  unknown/stale tool calls, and redaction of public-safe summaries.
