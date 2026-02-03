# Pi plugins support plan

Plan to add support for pi-coding-agent–style extensions (“pi plugins”) in OpenAgents: map pi extension capabilities to our stack (Adjutant, Autopilot, dsrs, Pylon) and outline how we can support equivalent or bridged behavior.

**Reference:** [pi-mono/packages/coding-agent](https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent) README and [docs/extensions.md](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/extensions.md), plus [examples/extensions/](https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent/examples/extensions).

---

## 1. Pi extension model (recap)

Pi extensions are TypeScript modules that receive `ExtensionAPI` and can:

| Capability | Pi API | Purpose |
|------------|--------|---------|
| **Custom tools** | `pi.registerTool({ name, description, parameters, execute, renderCall?, renderResult? })` | LLM-callable tools with TypeBox schemas |
| **Events** | `pi.on("session_start" \| "tool_call" \| "before_agent_start" \| …)` | Lifecycle, tool intercept, context edit, compaction, etc. |
| **Commands** | `pi.registerCommand("name", { description, handler, getArgumentCompletions? })` | Slash commands (e.g. `/plan`, `/sandbox`) |
| **Shortcuts** | `pi.registerShortcut("ctrl+shift+p", { handler })` | Keyboard bindings |
| **Flags** | `pi.registerFlag("plan", { type: "boolean", default: false })` | CLI flags |
| **UI** | `ctx.ui.select`, `confirm`, `input`, `notify`, `setStatus`, `setWidget`, `setFooter`, `custom()`, `setEditorComponent` | Dialogs, status, widgets, overlay, custom editor |
| **Messages** | `pi.sendMessage`, `pi.sendUserMessage`, `pi.appendEntry` | Inject custom or user messages; persist state |
| **Session** | `ctx.sessionManager`, `ctx.newSession`, `ctx.fork`, `ctx.navigateTree` | Session metadata, fork, tree nav |
| **Tools control** | `pi.getActiveTools()`, `pi.setActiveTools(names)` | Enable/disable built-in tools |
| **Model** | `pi.setModel`, `pi.getThinkingLevel`, `pi.setThinkingLevel` | Model and thinking level |
| **Provider** | `pi.registerProvider(name, config)` | Custom/OAuth providers, proxy URLs |
| **Built-in overrides** | Register tool with same name as `read`/`bash`/`edit`/`write` | Wrap or replace built-in tools with custom ops (e.g. SSH, sandbox) |
| **Event bus** | `pi.events.on/emit` | Inter-extension communication |

Extensions are loaded from `~/.pi/agent/extensions/`, `.pi/extensions/`, or via `pi -e ./path`. They run in the same process (jiti for TS), with full system access.

---

## 2. Pi example extensions (inventory)

From pi’s `examples/extensions/` and docs:

| Category | Examples | What they do |
|----------|----------|----------------|
| **Tools** | hello, question, questionnaire, todo, truncated-tool, tool-override | Custom tools; user interaction in tools; stateful tools; truncation; override read |
| **Commands** | pirate, summarize, handoff, qna, send-user-message, shutdown-command | System prompt tweak; summary; model handoff; Q&A UI; inject user message; shutdown |
| **Events / gates** | permission-gate, protected-paths, confirm-destructive, dirty-repo-guard, input-transform, model-status, system-prompt-header, claude-rules, file-trigger | Block dangerous bash; block writes to paths; confirm session changes; warn dirty git; transform input; react to model change; compaction/session hooks |
| **Compaction / session** | custom-compaction, trigger-compact, git-checkpoint, auto-commit-on-exit | Custom compaction; manual compact; git stash on turn; commit on exit |
| **UI** | status-line, custom-footer, custom-header, modal-editor, rainbow-editor, widget-placement, overlay-test, overlay-qa-tests, notify, timed-confirm, mac-system-theme | Footer/header/widget/overlay; vim-style editor; theme switch |
| **Complex** | plan-mode/, preset, tools | Plan mode (read-only + todo extraction); saveable presets; toggle tools UI |
| **Remote / sandbox** | ssh, interactive-shell, sandbox/, subagent/ | SSH bash ops; persistent shell; OS sandbox (bubblewrap/sandbox-exec); spawn pi subagents |
| **Providers** | custom-provider-anthropic/, custom-provider-gitlab-duo/, custom-provider-qwen-cli/ | Custom Anthropic proxy; GitLab Duo OAuth; Qwen CLI |
| **Messages** | message-renderer, event-bus | Custom message renderer; inter-extension events |
| **Session metadata** | session-name, bookmark | Session name; labels for /tree |
| **Misc** | doom-overlay, snake, space-invaders, antigravity-image-gen, with-deps/ | Games in overlay; image gen; extension with npm deps |

---

## 3. OpenAgents current surface

| Layer | Where | What we have |
|-------|--------|----------------|
| **Tools** | `crates/adjutant/src/tools.rs` | `ToolRegistry`: fixed set (read_file, edit_file, write_file, bash, glob_file, grep). No plugin registration; schemas and execution are Rust. |
| **Execution** | Adjutant, coding_agent_loop | Single loop: DSPy decisions → tool calls → receipts. No event hooks for “before tool” / “after tool” or context mutation. |
| **Session** | Autopilot / Codex | Thread/turn state in app-server; session files in pi/Codex world, not yet a unified “session manager” in Rust with fork/tree. |
| **UI** | Autopilot (wgpui), apps/web | No extension-defined commands/shortcuts/widgets; no `ctx.ui` abstraction. |
| **Model / provider** | lm-router, model registry | Provider/model selection in Rust; no dynamic `registerProvider` from scripts. |
| **Sandbox / remote** | Autopilot container mode (partial), Pylon | No OS-level sandbox (bubblewrap/sandbox-exec) or SSH tool operations in Adjutant. |
| **Plan mode / presets** | — | No plan mode or saveable presets. |
| **Subagents** | — | No “spawn another agent” tool; RLM/tiered executor is internal. |

So today we have a closed tool set and no pi-style extension API.

---

## 4. Mapping pi extensions → OpenAgents

### 4.1 Custom tools

- **Pi:** `registerTool` with TypeBox schema and `execute(toolCallId, params, signal, onUpdate, ctx)`.
- **OpenAgents:** `ToolRegistry` is fixed (Read, Edit, Bash, Glob, Grep); schemas and execution are in Rust.

**Options:**

- **A) Rust plugin API:** Add a “plugin” or “dynamic tool” notion to Adjutant: load Rust crates or WASM modules that implement a `Tool` trait (name, description, schema, execute). Requires a story for loading (e.g. discover from `~/.openagents/tools/` or config).
- **B) Bridge to pi/Node:** Run a small Node/TS runtime (e.g. in a worker or sidecar) that loads pi extensions and exposes their tools via JSON-RPC or stdio; Adjutant calls out to the bridge for “plugin” tool names. Tool schemas and results cross the boundary as JSON.
- **C) MCP-style server:** Treat pi extensions as one possible implementation of an MCP server that Adjutant talks to (per AGENTS.md: “Build an extension that adds MCP support”). Then “pi plugins” are one source of MCP tools.

Recommendation: Start with **B** or **C** for fastest parity (reuse pi extensions as-is); **A** for long-term if we want plugins to be Rust-only and sandboxed (e.g. WASM).

### 4.2 Tool-call interception (permission gates, protected paths)

- **Pi:** `on("tool_call")` → return `{ block: true, reason }` or let through.
- **OpenAgents:** No hook before tool execution.

**Options:**

- **A) Event/callback in coding_agent_loop:** Before executing a tool, emit a “tool_call_request” or call a registered closure; if any returns “block”, skip execution and return a fixed error to the LLM. Requires an extension point in Rust (e.g. `Vec<Box<dyn Fn(ToolCall) -> Option<Block>>`).
- **B) Bridge:** If plugin tools run in a bridge, the bridge can implement permission checks; for built-in tools we’d still need a hook in Rust (as in A).

Recommendation: Add a **tool_call pre-hook** in Adjutant (e.g. optional callback or event channel) so that future Rust or bridged plugins can block or modify requests.

### 4.3 Commands and shortcuts

- **Pi:** `/plan`, `/sandbox`, etc., and Ctrl+Alt+P.
- **OpenAgents:** Autopilot has its own commands/shortcuts; no extension-registered commands.

**Options:**

- **A) Config-driven commands:** Define commands (and shortcuts) in config (e.g. YAML/JSON under `~/.openagents/` or project `.openagents/`): name, description, handler type (e.g. “toggle_plan_mode”), args. Autopilot UI binds them; “handler” is implemented in Rust (e.g. set a flag, change tools).
- **B) Bridge:** Commands and shortcuts implemented in a pi-compat layer; when user triggers one, bridge runs the extension handler and sends back an action (e.g. “set_plan_mode true”) that Autopilot applies.

Recommendation: **A** for a small set of built-in behaviors (plan mode, sandbox toggle); **B** if we want to run pi extensions that register commands unchanged.

### 4.4 Lifecycle events (session_start, before_agent_start, context, compaction, etc.)

- **Pi:** Rich event set (session_start, before_agent_start, context, turn_start/end, session_before_compact, session_tree, …).
- **OpenAgents:** No equivalent event bus in Adjutant or Autopilot.

**Options:**

- **A) Internal event bus in Rust:** Add an `EventBus` in Adjutant (or shared runtime): subscribe by event name, emit from coding_agent_loop and session/compaction code. Handlers are Rust only unless we add a bridge.
- **B) Bridge:** Pi extensions subscribe to events; bridge translates our internal events (if we add them) to pi-shaped events and calls extension handlers; handlers can return results (e.g. context mutation, compaction override) that the bridge translates back.

Recommendation: Add a minimal **internal event bus** (A) for tool_call, turn_start/end, session_switch, compaction — so we can later drive a bridge (B) or native Rust plugins from the same events.

### 4.5 UI (status, widgets, overlay, custom editor)

- **Pi:** `ctx.ui.setStatus`, `setWidget`, `setFooter`, `custom()` overlay, `setEditorComponent`.
- **OpenAgents:** Autopilot UI is wgpui; no extension slots for status line, widget above/below editor, or custom overlay.

**Options:**

- **A) Reserved UI slots:** In Autopilot, define “extension status”, “extension widget”, “extension overlay” slots; content is either provided by Rust (e.g. “plan mode: 3/5 steps”) or by a bridge that sends strings/structured data to the UI.
- **B) Full pi UI in a separate process:** Run pi in TUI mode and don’t replicate its UI in OpenAgents; “support” = run pi as the front-end and have it call OpenAgents for execution. That’s a different product shape.

Recommendation: **A** — add a small set of slots (e.g. status line keys, one widget area, one overlay) so that plan mode, sandbox indicator, etc. can be shown when we implement those features; bridge can fill slots when pi extensions are used.

### 4.6 Providers and model selection

- **Pi:** `registerProvider(name, config)` with baseUrl, apiKey, OAuth, models.
- **OpenAgents:** lm-router and model registry in Rust; providers configured via code/config, not by extensions.

**Options:**

- **A) Config-only:** Add a way to add providers from config (e.g. `~/.openagents/providers.json`) with same shape as pi’s provider config; no TypeScript.
- **B) Bridge:** Pi extension calls `registerProvider`; bridge sends provider config to a small “provider registry” service or to Autopilot; that service updates lm-router or the model list the UI uses. Requires a channel (e.g. RPC, HTTP) from bridge to runtime.

Recommendation: **A** for MVP (config-driven custom providers); **B** if we want pi packages that register providers to work unchanged.

### 4.7 Sandbox and remote execution

- **Pi:** Sandbox extension wraps bash with `@anthropic-ai/sandbox-runtime` (bubblewrap/sandbox-exec); SSH extension provides custom `BashOperations` so `!cmd` and bash tool run remotely.
- **OpenAgents:** No OS sandbox; no SSH in ToolRegistry.

**Options:**

- **A) Sandbox in Rust:** Integrate a Rust sandbox (e.g. bubblewrap/sandbox-exec) and use it inside `ToolRegistry::bash` when a “sandbox” mode is on (flag or config). No pi extension needed; behavior parity.
- **B) SSH in Rust:** Add “remote workspace” or “SSH operations” to Adjutant (e.g. optional `BashOperations`-like trait) so bash (and maybe read/edit) run over SSH. Again, no pi extension.
- **C) Bridge:** Pi sandbox/SSH extensions run in Node; when Adjutant needs to run “bash”, it delegates to the bridge; bridge runs pi’s bash tool with sandbox/SSH. Heavy: every bash call crosses process boundary.

Recommendation: **A** and **B** natively in Rust for performance and simplicity; document as “pi plugin parity” without running pi’s extensions for these.

### 4.8 Plan mode

- **Pi:** Flag + command/shortcut; restrict tools to read-only; parse “Plan:” and “[DONE:n]”; widget with todo list.
- **OpenAgents:** No plan mode.

**Options:**

- **A) Implement in Rust + UI:** “Plan mode” flag in Autopilot; when on, `ToolRegistry` only exposes read/grep/glob (or a separate read-only registry); parsing of “Plan:” / “[DONE:n]” in the loop or in a small module; UI widget showing steps (data from session or from a dedicated structure). No pi extension.
- **B) Bridge:** Pi plan-mode extension runs; it calls `setActiveTools` and `setWidget` via bridge; we’d need to implement the same tool restriction and widget contract on our side.

Recommendation: **A** — implement plan mode as a first-class feature (tool subset + step extraction + widget); optional compatibility with pi’s step format so “same prompts” work.

### 4.9 Subagents

- **Pi:** Subagent extension: tool that spawns `pi` subprocess with a task and returns structured output (single / parallel / chain).
- **OpenAgents:** No “spawn another agent” tool; we have tiered executor and RLM internally.

**Options:**

- **A) “Subagent” tool in Rust:** A tool that starts another Autopilot/Adjutant run (or calls a separate service) with a task and returns the result. Orchestration (single/parallel/chain) can be in Rust or in DSPy.
- **B) Bridge:** Pi subagent extension runs; it spawns `pi` as today; if we want “subagent = OpenAgents run”, the extension could be taught to spawn our CLI instead of pi (then it’s “pi extension that calls OpenAgents” rather than “OpenAgents runs pi extension”).

Recommendation: **A** — add a first-class “subagent” or “delegate” capability (e.g. tool or tiered step) that runs a child task and returns result; document as equivalent to pi’s subagent pattern.

### 4.10 Custom compaction and session tree

- **Pi:** `session_before_compact` can return custom summary; `session_before_tree` / `session_tree` for /tree navigation.
- **OpenAgents:** Compaction and session tree are in Codex/pi world; Autopilot/Adjutant don’t yet have a unified compaction/tree API.

**Options:**

- **A) Events in our session layer:** When we have a session/compaction/tree abstraction in Rust, add events (e.g. `before_compact`, `before_tree`) and allow a registered handler to supply a custom summary or cancel. Then bridge can forward to pi extensions if we run them.
- **B) Bridge-only:** Compaction and tree stay in the Codex/pi process; if we run pi as front-end, no change; if we own the session, we need (A) first.

Recommendation: **A** when we consolidate session/compaction/tree in our stack; **B** as a temporary path if the UI is still pi/Codex.

### 4.11 Message injection and session metadata

- **Pi:** `sendMessage`, `sendUserMessage`, `appendEntry`, `setSessionName`, `setLabel`.
- **OpenAgents:** No equivalent in Adjutant; session naming/labels might live in app-server or Autopilot.

**Options:**

- **A) API in session layer:** When we have a session manager in Rust (or a clear owner), add “append custom entry”, “set session name”, “set label” so that a bridge or Rust logic can mimic pi.
- **B) Bridge:** Extension calls pi API; bridge sends “append entry” / “set name” to our backend; backend must persist and expose this in session/UI.

Recommendation: **A** when session ownership is clear; **B** for bridge parity.

---

## 5. Proposed architecture for “pi plugins” support

### 5.1 Principle

- **Native first:** Reimplement high-value behaviors (sandbox, SSH, plan mode, subagent, permission gates) in Rust/Adjutant/Autopilot so we don’t depend on Node for core safety or UX.
- **Bridge optional:** A “pi compatibility” bridge can run pi extensions in a Node/TS process, translate our events and tool calls to pi’s API, and push UI/provider/command updates back. Use this for maximum pi package compatibility, not for core security.

### 5.2 Phased implementation

| Phase | Scope | Deliverables |
|-------|--------|--------------|
| **1 – Hooks and tools** | Tool pre-hook; optional “plugin tools” via config or bridge | `tool_call` pre-hook in coding_agent_loop; design for dynamic tool registration (config schema or RPC) |
| **2 – Native parity** | Sandbox, SSH, plan mode, permission gates | Sandbox (bubblewrap/sandbox-exec) in `ToolRegistry::bash`; SSH operations trait; plan mode (tool subset + step parsing + widget); tool_call hook used by a “gate” (e.g. block list) |
| **3 – Commands and UI slots** | Commands and shortcuts; status/widget/overlay | Config-driven commands/shortcuts; extension status line and one widget slot in Autopilot; overlay slot if needed for Q&A/plan |
| **4 – Session and events** | Session events; compaction/tree hooks | Internal event bus (session_start, turn_start/end, before_compact, before_tree); session name/labels and append entry API |
| **5 – Provider and model** | Custom providers without pi | Config-driven provider registration (baseUrl, apiKey, OAuth, models) consumed by lm-router / UI |
| **6 – Bridge (optional)** | Run pi extensions as plugins | Node/TS process that loads pi extensions; stdio or RPC to Adjutant; translates tool calls, events, UI updates, provider reg; document “pi package compatibility” |

### 5.3 File and ownership (suggested)

- **Adjutant:** `tools.rs` (sandbox/SSH ops); `coding_agent_loop.rs` (tool pre-hook, event emit); new `events.rs` or `plugin.rs` for event bus and optional plugin trait.
- **Autopilot:** Config for commands/shortcuts; UI slots (status, widget, overlay); plan mode state and widget.
- **Config:** `~/.openagents/` or `.openagents/` — `tools.json` (plugin tool list or bridge endpoint), `commands.json`, `providers.json`, `sandbox.json`.
- **Bridge (if built):** New repo or `apps/pi-bridge/` — Node app that loads pi extensions, talks to Adjutant via RPC/stdio, and pushes UI/provider updates to Autopilot or a small API.

### 5.4 Summary table

| Pi capability | OpenAgents approach |
|--------------|---------------------|
| Custom tools | Config or RPC for “plugin tools”; bridge can expose pi tools |
| Tool interception | Pre-hook in coding_agent_loop; gate (e.g. block list) in Rust |
| Commands / shortcuts | Config-driven in Autopilot; bridge can emulate pi.registerCommand |
| Lifecycle events | Internal event bus in Rust; bridge can subscribe and call pi handlers |
| UI (status, widget, overlay) | Reserved slots in Autopilot; bridge fills from pi ctx.ui |
| Providers | Config-driven provider registration; bridge can call registerProvider |
| Sandbox / SSH | Native in Rust (ToolRegistry ops) |
| Plan mode | Native (tool subset + parsing + widget) |
| Subagents | Native “delegate” tool or tiered step |
| Compaction / tree | Events + optional custom summary when we own session |
| Messages / session metadata | Session API (append entry, name, labels) when we own session |

This gives a single plan: native parity for safety and UX, optional bridge for pi package compatibility, and a clear sequence (hooks → native features → UI/commands → session/events → providers → bridge).
