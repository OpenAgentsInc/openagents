# Coder Desktop vs. Claude Code: Detailed Gap Analysis & Implementation Order

- **Date**: 2026-11-19
- **Coder Desktop Baseline**: `openagents` `main` (`fba41b042f`, `apps/coder-desktop/`, `coder/`, `coax/lib/microcoder/`, `coax/lib/agent/`)
- **Claude Code Reference**: `~/work/projects/repos/cc` (TypeScript/Bun source tree, ~`2.1.296`–`2.1.312` era)

---

## 1. Executive Summary

Coder Desktop is already a native, GPU-rendered macOS app (`AppKit` + Coax UI) backed by a persistent daemon (`coder-host`), per-chat APFS copy-on-write git worktrees, multi-provider sub-agents (Gemini Hydrogen/Carbon/Neon, Claude Haiku/Sonnet/Opus/Fable, OpenRouter), a Foreman supervisor, and live mobile streaming over `NIP-HOST`. In several areas—native GPU rendering, built-in code review and file editor panes, multi-vendor sub-agents, Foreman orchestration, and zero-trust Nostr mobile control—Coder Desktop is **ahead of or architecturally distinct from** Claude Code CLI.

However, comparing Coder Desktop's **everyday coding-agent workflow** against Claude Code's full source tree (`~/work/projects/repos/cc`) reveals clear parity gaps in six high-leverage areas:

1. **Instruction & Memory Layer**: No automatic `CLAUDE.md` / `AGENTS.md` hierarchy loading in Microcoder, and no persistent cross-session auto-memory (`MEMORY.md` / `memdir`).
2. **Safety, Modes & Approvals**: No interactive permission/approval UI in Desktop, no `acceptEdits` / `auto` mode toggles (note: **admin has ruled out adding Plan Mode**), no macOS Seatbelt sandboxing (macOS runs unconfined `danger-full-access`), and no user-defined hooks beyond the built-in `gitguard`.
3. **Session Control & Recovery**: No turn-level undo/rewind (`fileHistory` snapshots), no session forking in Desktop UI (exists in `coax/lib/agent/session/fork.coax` for TUI), and no manual `/compact`.
4. **Extensibility & Ecosystem**: MCP, plugins, and voice dictation exist in `coder/tui/` but are **not wired into Coder Desktop**; no LSP tool/diagnostics; no deferred tool discovery (`ToolSearch`).
5. **Interactive UX & Multimodal**: Only 3 slash commands in Desktop (`/goal`, `/model`, `/new`) vs. ~100 in Claude Code; image attachments are path-only (`@path`) rather than true multimodal vision input; no `AskUserQuestion` structured prompt cards in Desktop; no top-level engine switch to run Claude Code directly as the primary session driver in Desktop (currently sub-agent only).
6. **Scheduling & Web Tools**: No native Microcoder `WebSearch`/`WebFetch` tools (only via Claude sub-agents) and no user-scheduled cron/recurring tasks.

---

## 2. Detailed Feature-by-Feature Gap Matrix

Status legend: **Implemented** (parity or ahead) · **Partial** (exists in TUI/SDK or limited subset in Desktop) · **Missing** (not yet built in Coder Desktop).

### 2.1 Core Agent Loop, Streaming, Thinking & Effort Control

| Capability | Claude Code (`~/work/projects/repos/cc`) | Coder Desktop (`openagents`) | Status |
| --- | --- | --- | --- |
| Streaming agent loop | `QueryEngine.ts`, `query.ts` with streaming content blocks, `max_output_tokens` continuation, PTL recovery | `coax/lib/microcoder/loop.coax` + `coder/tui/mcworker.coax` NDJSON wire (`mcworker-wire.coax`) | **Implemented** |
| Streaming tool execution | `StreamingToolExecutor` executes safe tools while `tool_use` streams in | Batch execution per `next_action` turn (`edits` → `commands` → `view`) | **Partial** |
| Thinking display | `utils/thinking.ts` (`adaptive` / `enabled` / `disabled`, `ultrathink`) | Summarized thinking blocks (`{"kind":"thought"}`) in `transcript.coax` & `claudeworker.coax` | **Implemented** |
| Effort level control | `utils/effort.ts`, `/effort` (`low`/`medium`/`high`/`max`), per-model capability detection | Router/model selection only; no user-facing effort slider or `/effort` toggle | **Missing** |
| Primary engine switching | Single Claude engine with aliases (`sonnet`, `opus`, `haiku`, `best`, `opusplan`, `[1m]`) & 3P (`bedrock`/`vertex`/`foundry`) | Microcoder is the primary Desktop driver; Claude Code is sub-agent only (`claudeworker.coax`). Top-level engine switch exists in TUI (`coder/tui/coder.coax`), not Desktop | **Partial** |
| Background tool-use summaries | `services/toolUseSummary/toolUseSummaryGenerator.ts` (Haiku summaries) | Skeleton shimmer & rolling working word (`fba41b042f`), raw tool rows | **Partial** |

### 2.2 Built-in Tool Surface

| Capability | Claude Code | Coder Desktop (Microcoder) | Status |
| --- | --- | --- | --- |
| File read/edit/write | `Read` (text, images, PDFs, notebooks), `Edit`, `Write`, `NotebookEdit` | `view` (text ranges, unchanged-file stubs), `edits` (exact replacement / whole-file write). No image/PDF/notebook native read | **Partial** |
| Shell execution | `Bash` (backgrounding, sandboxing, classifier), `PowerShell`, `REPL` | `commands` (`/bin/sh`, 120s timeout, head/tail truncation + full output file) | **Implemented** |
| Fast search (`Grep`/`Glob`) | Dedicated `GrepTool` (ripgrep) & `GlobTool` | Shell-based (`git grep` / `rg` via `commands`) + Coax compiler intel (`questions` via `aware.coax`) | **Partial** |
| Code intelligence (LSP) | `LSPTool` + `services/lsp/` (definitions, references, hover, symbols, diagnostics) | Coax compiler intel (`aware.coax`/`repo-tools.coax`) for Coax only; no general LSP | **Missing** |
| Web search & fetch | `WebSearchTool`, `WebFetchTool` | Missing natively in Microcoder; available only by delegating to a Claude sub-agent | **Partial** |
| Structured task board | `TaskCreate`/`TaskGet`/`TaskUpdate`/`TaskList` (+ legacy `TodoWrite`) | Working memory `open` list + Desktop goal bar (`page/goal.coax`) | **Partial** |
| Structured user questions | `AskUserQuestionTool` (multi-choice / interactive cards) | `ask` field in Microcoder schema, disabled/non-interactive in Desktop runs | **Missing** |
| Worktree tools | `EnterWorktreeTool`, `ExitWorktreeTool` | Automatic per-chat worktrees (`Worktree.swift`, `Window+Worktrees.swift`) | **Implemented** |
| Deferred tool search | `ToolSearchTool` (on-demand loading of large MCP/tool catalogs) | None (fixed Microcoder schema) | **Missing** |

### 2.3 Permissions, Sandboxing, Hooks & Modes

| Capability | Claude Code | Coder Desktop | Status |
| --- | --- | --- | --- |
| Permission modes | `default`, `acceptEdits`, `plan`, `dontAsk`, `bypassPermissions`, `auto` (2-stage LLM classifier in `yoloClassifier.ts`/`bashClassifier.ts`) | TUI/SDK have `--permission-mode`; Desktop sets `gated: true` only to render rows before auto-approving. No mode switcher (**Note: admin has ruled out adding Plan Mode**) | **Missing** |
| Interactive approval prompts | `PermissionRequest` UI, allow/deny/ask rules (`utils/permissions/permissions.ts`), `permissionExplainer.ts` | Exists in TUI (`coder/tui/approvals.coax`, `gate.coax`); **missing in Desktop UI** | **Partial** |
| OS sandboxing | `utils/sandbox/sandbox-adapter.ts` (`@anthropic-ai/sandbox-runtime`: macOS Seatbelt + Linux `bwrap` + FS/domain allowlists) | Linux `bwrap` only (`coax/lib/microcoder/fence.coax`); **macOS Desktop runs unconfined (`danger-full-access`)** | **Partial** |
| Git destructive-op guard | `dangerousPatterns.ts` + classifier | `git-guard.coax` / `coder/tui/gitguard.coax` (enforced in Microcoder + Claude `PreToolUse` hook) | **Implemented** |
| User-defined hooks | 27 `HOOK_EVENTS` (`PreToolUse`, `PostToolUse`, `SessionStart`, `Stop`, `PreCompact`, `FileChanged`, etc.) with shell, prompt, agent & HTTP executors | Only built-in `gitguard` `PreToolUse` in Desktop; TUI plugins have `turn_end` hooks; Coax Claude SDK marks `hooks` missing | **Partial** |

### 2.4 Context Management, Compaction & Memory

| Capability | Claude Code | Coder Desktop | Status |
| --- | --- | --- | --- |
| Rolling auto-compaction | `microCompact.ts`, `autoCompact.ts`, `compact.ts`, `sessionMemoryCompact.ts` (preserves top 5 files, active skills, active plan) | 3-tier rolling state (`state.coax`: 32 KB/step, 140 KB recent `actions[]`, compacted `earlier[]`, `left_out`) + context-fill ring in Desktop | **Implemented** |
| Manual `/compact` & `/context` | `commands/compact`, `commands/context`, `commands/ctx_viz` | Context-fill ring with token counts; **no manual `/compact`** | **Partial** |
| `CLAUDE.md` / `AGENTS.md` loading | `utils/claudemd.ts` (hierarchical repo/user/managed/conditional rules, 40 KB cap, nested dir discovery) | **Missing** in Microcoder/Desktop (Claude sub-agents inherit Claude Code's own loading) | **Missing** |
| Persistent auto-memory (`memdir`) | `memdir/memdir.ts` (`MEMORY.md`, 200 lines/25 KB), `extractMemories.ts`, `autoDream.ts` background consolidation | Per-run non-compacted working memory (`coax/lib/agent/workmem/`, `notes` with `PATH:A-B` auto-invalidation); **no cross-session auto-memory** | **Partial** |

### 2.5 Extensibility: Skills, Plugins, Custom Agents, MCP & LSP

| Capability | Claude Code | Coder Desktop | Status |
| --- | --- | --- | --- |
| Skills (`SKILL.md`) | `skills/loadSkillsDir.ts` + 14 bundled skills (`batch`, `debug`, `loop`, `simplify`, `verify`, etc.) + `SkillTool` | `Worker.swift` discovers `~/.claude/skills/*/SKILL.md` (first 16 skills, 4 KB cap) and prepends index to Microcoder tasks | **Implemented** |
| Plugins & marketplace | `utils/plugins/` (install, marketplace, dependency resolution, plugin agents/commands/hooks/MCP/LSP) | TUI has manifest + Nostr-signed WASM plugins (`coder/tui/plugins.coax`); **not wired into Desktop** | **Partial** |
| Custom agents & output styles | `loadAgentsDir.ts` (`.claude/agents/*.md`), built-in `explore`/`plan`/`verification` agents, `outputStyles/` | Fixed sub-agent model tiers (`hydrogen`/`carbon`/`neon`/`haiku`/`sonnet`/`opus`/`fable`/`openrouter`); no user-defined agent profiles | **Missing** |
| MCP client & auth | `services/mcp/` (stdio, SSE, HTTP, WebSocket, OAuth, elicitation, resources, prompts) | Implemented in TUI (`coder/tui/mcp.coax`, `mcp_http.coax`); **missing in Desktop/Microcoder** (Claude sub-agents inherit local MCP config) | **Partial** |
| LSP integration | `services/lsp/` + `LSPTool` + passive diagnostics feedback | Missing (Coax compiler intel only) | **Missing** |

### 2.6 Sessions, Worktrees, Undo/Rewind & Diff Review

| Capability | Claude Code | Coder Desktop | Status |
| --- | --- | --- | --- | --- |
| Session persistence & resume | `utils/sessionStorage.ts`, `sessionRestore.ts`, `/resume`, agentic session search | `Store.swift` + `page/persist.coax` (`~/Library/Application Support/Coder/chats/ID.json`) + live daemon reattach + Cmd+K search | **Implemented** |
| Session fork / branch | `commands/branch`, `forkedAgent.ts` | Exists in `coax/lib/agent/session/fork.coax` (TUI); **missing in Desktop UI** | **Partial** |
| Per-chat git worktrees | `utils/worktree.ts`, `EnterWorktreeTool`/`ExitWorktreeTool` | Automatic `~/.openagents/worktrees/REPO/CHAT` with APFS CoW clones (`.coder/worktree.json`) & shared caches (`Worktree.swift`) | **Implemented** (Ahead) |
| Undo / rewind / file history | `utils/fileHistory.ts` (`fileHistoryTrackEdit`, `fileHistoryMakeSnapshot`, `fileHistoryRewind`, `/rewind`) | **Missing** (no per-turn checkpoint or file rollback) | **Missing** |
| Diff review UI | Terminal diff views in Ink components | Dedicated right-pane Review/Changes tab (`page/layout/review.coax`, `components/app/diff.coax`) + in-place file editor (`page/filepane.coax`) | **Implemented** (Ahead) |

### 2.7 Subagents, Teams, Foreman & Background Tasks

| Capability | Claude Code | Coder Desktop | Status |
| --- | --- | --- | --- |
| Parallel sub-agents | `AgentTool` (foreground/background/worktree), `TaskOutput`/`TaskStop` | Up to 6 parallel/step, 12/turn across Gemini/Claude/OpenRouter; live right-pane transcript; 100-tool-call pause; `stop_agent` | **Implemented** (Ahead) |
| Multi-agent teams / swarm | `TeamCreate`/`TeamDelete`, `SendMessageTool`, `coordinator/` | **Foreman** supervisor (`Foreman.swift`, `Command-\`, HUD, `foreman-ctl`) manages multiple chats | **Implemented** (Different model) |
| Persistent background daemon | Background tasks within CLI process; remote triggers for persistence | Bundled `coder-host` daemon (`DaemonClient.swift`, `host-daemon.coax`) runs up to 12 workers outliving app quit | **Implemented** (Ahead) |

### 2.8 Remote, Mobile, Teleport & Bridge

| Capability | Claude Code | Coder Desktop | Status |
| --- | --- | --- | --- |
| Mobile / remote control | `bridge/` (`replBridge.ts`, `trustedDevice.ts`), `remote/`, `/mobile`, `/teleport` | `NIP-HOST` Nostr relay pairing (`host-desktop.coax`) streams Desktop chats live to OpenAgents Mobile with steer/queue/stop grants | **Implemented** |
| Remote execution hosts | Cloud environments / teleport | TUI supports `--on ssh:|cloud|boat:` (`remote.coax`, `daemon.coax`); **not exposed in Desktop UI** | **Partial** |

### 2.9 Desktop UI, Slash Commands, Multimodal, Voice & IDE Integration

| Capability | Claude Code | Coder Desktop | Status |
| --- | --- | --- | --- |
| Native desktop UI & shortcuts | CLI (Ink/React) + separate Electron Claude Desktop | Native GPU-rendered macOS AppKit + Coax UI (`Cmd+K`, `Ctrl+Tab`, `Esc`, sliding sidebar/filepane, toast stack, shimmer) | **Implemented** (Ahead) |
| Slash commands | ~100 built-in commands (`commands/`) | Only 3 in Desktop (`/goal`, `/model`, `/new` in `page/mention.coax`) | **Partial** |
| Multimodal image/PDF attachments | `Read` tool + prompt image paste sends pixels/PDFs directly to model | `page/attach.coax` attaches images as `@path` text notes only; Microcoder cannot inspect pixels | **Partial** |
| Voice / dictation | `voice/`, `services/voice/` | Implemented in TUI (`coder/tui/dictate.coax`); **missing in Desktop** | **Partial** |
| External IDE bridge | `vscodeSdkMcp.ts`, IDE selection/diagnostics sync | Built-in file viewer/editor pane (`filepane.coax`); no VS Code / JetBrains bridge | **Partial** |

### 2.10 Scheduling / Cron, Auth, Cost & Updates

| Capability | Claude Code | Coder Desktop | Status |
| --- | --- | --- | --- |
| Cron / scheduled tasks | `ScheduleCronTool` (`CronCreate`/`CronDelete`/`CronList`), `cronScheduler.ts`, `RemoteTriggerTool` | **Missing** | **Missing** |
| Auth / BYO subscription | OAuth + API key + Bedrock/Vertex/Foundry | OpenAgents account + BYO Claude key/login + OpenRouter (`settings.coax`, `Window+Dialogs.swift`) | **Implemented** |
| Cost / usage tracking | `cost-tracker.ts`, `/cost`, `/usage` | Per-chat cost/budget tracking, budget dialog, context-fill ring | **Implemented** |
| Auto-updater | CLI auto-updater | Native macOS `Updater.swift` + `Window+Update.swift` | **Implemented** |

---

## 3. Recommended Implementation Order to Rough Parity

This roadmap prioritizes **highest daily-driver leverage per unit of engineering effort**, reusing code that already exists in `coder/tui/` and `coax/lib/agent/` before building net-new subsystems.

### Phase 0 — Foundation & Zero-Friction Parity (1–2 weeks)
*Close the most noticeable day-one gaps for users coming from Claude Code.*

1. **`CLAUDE.md` / `AGENTS.md` Auto-Loading in Microcoder**
   - **Why first**: Every real repo relies on `CLAUDE.md` / `AGENTS.md` instructions. Without it, Microcoder misses project conventions that Claude Code follows automatically.
   - **Where**: `apps/coder-desktop/Worker.swift` (alongside existing `SKILL.md` discovery) and `coax/lib/microcoder/state.coax`.
   - **Scope**: Walk from repo root to `cwd` plus `~/.claude/CLAUDE.md`, cap at 40 KB, inject into initial task/system context, and invalidate/note on edit.
2. **Expand Desktop Slash Commands & Command Palette Actions**
   - **Why**: Desktop currently only exposes `/goal`, `/model`, `/new`. Adding `/compact`, `/fork`, `/rewind`, `/effort`, `/mode`, `/mcp`, `/clear`, `/cost` unlocks power-user workflows immediately.
   - **Where**: `apps/coder-desktop/page/mention.coax`, `page/palette.coax`, `Window+Palette.swift`.
3. **Manual `/compact` + Session Forking in Desktop UI**
   - **Why**: Compaction (`state.coax`) and session forking (`coax/lib/agent/session/fork.coax`) already exist in the engine/TUI—just wire them to Desktop actions and sidebar/palette entries.
   - **Where**: `apps/coder-desktop/Window+Chats.swift`, `Store.swift`, `coder/tui/mcworker.coax`.

### Phase 1 — Safety, Modes & Trustworthy Autonomy (2–3 weeks)
*Make Coder Desktop safe to run on real repositories without supervision.*

4. **Agent To-Do / Task List UI (Live Checklist)**
   - **Why**: Surfaces the agent's working plan and step-by-step progress directly in Desktop (like Claude Code's `TodoWrite` / `TaskCreate`–`TaskUpdate` list) without introducing a separate Plan Mode.
   - **Where**: Add a structured `todos` / task-list field in Microcoder (`coax/lib/microcoder/loop.coax` & `coder/tui/mcworker-wire.coax`) and render a live collapsible checklist in `apps/coder-desktop/page/transcript.coax` (or pinned above the composer).
5. **Permission Modes (`default`, `acceptEdits`, `bypassPermissions`) + Interactive Approval Cards** *(Plan Mode ruled out by admin)*
   - **Why**: Today Desktop silently auto-approves gated commands. **Note: Admin has ruled out adding Plan Mode**—scope this strictly to approval modes and interactive approval cards.
   - **Where**: Wire `coder/tui/approvals.coax` & `gate.coax` over `mcworker-wire.coax` to a new approval card + mode chip in `apps/coder-desktop/page/input.coax` and `page/transcript.coax`.
6. **macOS Seatbelt Sandboxing (`sandbox-exec`)**
   - **Why**: Linux has `bwrap` (`coax/lib/microcoder/fence.coax`), but macOS Desktop runs `danger-full-access`. Adding a Seatbelt profile restricting writes to the chat's worktree + `/tmp` + shared caches closes the biggest security gap.
   - **Where**: `coax/lib/microcoder/fence.coax` and `tools.coax`.
7. **Per-Turn File History & `/rewind` (Undo)**
   - **Why**: Users need a one-click safety net to roll back a bad agent turn without manual `git` surgery.
   - **Where**: Snapshot touched files before each turn's `edits`/`commands` in `Store.swift` / `Worktree.swift` (or lightweight git tree/ref snapshots per turn inside the worktree) and expose a Rewind action on every turn in `page/transcript.coax`.

### Phase 2 — Ecosystem & Multimodal Parity (2–3 weeks)
*Bring existing TUI/Claude capabilities into Desktop so users don't lose their tools.*

8. **Wire MCP Client into Desktop & Microcoder**
   - **Why**: `coder/tui/mcp.coax` and `mcp_http.coax` already exist. Exposing configured MCP servers (from `~/.claude.json` / `.mcp.json`) in Desktop settings and as callable Microcoder tools closes the biggest ecosystem gap.
   - **Where**: `apps/coder-desktop/settings.coax`, `coder/tui/mcworker.coax`, `coax/lib/microcoder/tools.coax`.
9. **True Multimodal Image Attachments**
   - **Why**: Pasting screenshots of UI bugs is a core desktop coding workflow; `@path` text notes don't let Microcoder see pixels.
   - **Where**: `apps/coder-desktop/page/attach.coax`, `Window+Attach.swift`, `coax/lib/microcoder/loop.coax` (pass base64 image blocks to vision-capable models).
10. **Native `WebSearch` / `WebFetch` + Effort Control (`/effort`)**
    - **Why**: Avoid spawning a full Claude sub-agent just to fetch docs, and let users dial reasoning effort (`low`/`medium`/`high`/`max`) per chat.
    - **Where**: `coax/lib/microcoder/tools.coax`, `apps/coder-desktop/page/input.coax`, `page/models.coax`.
11. **Top-Level Engine Switcher in Desktop (Microcoder vs. Direct Claude Code)**
    - **Why**: TUI already supports driving Claude Code directly (`coder/tui/coder.coax`). Letting Desktop users pick Claude Code as the primary chat engine (not just sub-agent) gives instant 100% tool/MCP/hook parity for users who want it.
    - **Where**: `apps/coder-desktop/Worker.swift`, `page/models.coax`, `coder/tui/claudeworker.coax`.

### Phase 3 — Power-User Workflows & Persistence (2–4 weeks)
*Match Claude Code's long-horizon memory, hooks, and interactive polish.*

12. **Cross-Session Auto-Memory (`MEMORY.md` / `memdir`)**
    - **Where**: Persist distilled repo/user memories under `~/.openagents/coder/memory/<repo>/MEMORY.md`, inject into `Worker.swift`, and add a lightweight post-turn/end-of-session memory extractor.
13. **User-Defined Hooks (`PreToolUse`, `PostToolUse`, `Stop`, `SessionStart`)**
    - **Where**: Generalize `coder/tui/gitguard.coax` into a hook runner reading `.coder/hooks.json` (and optionally `.claude/settings.json` hooks) for both Microcoder and `claudeworker`.
14. **Interactive `AskUserQuestion` Cards & Voice Dictation in Desktop**
    - **Where**: Enable `ask` structured question cards in `page/transcript.coax`; wire `coder/tui/dictate.coax` (or `apps/openagents-desktop` dictation) to a mic button in `page/input.coax`.
15. **Remote Host Picker in Desktop (`--on ssh:|cloud|boat:`)**
    - **Where**: Surface `coder/tui/remote.coax` targets in the new-chat / worktree picker (`Window+Worktrees.swift`).

### Phase 4 — Advanced / Optional Parity (Backlog)
*High complexity, lower urgency for rough parity; build as demand warrants.*

16. **LSP Integration** (`LSPTool` + passive diagnostics for non-Coax languages: TypeScript, Rust, Swift, Python).
17. **Scheduled / Cron Agent Tasks** (local `coder-host` cron scheduler + UI tab).
18. **Custom Agent Profiles (`.claude/agents/*.md`) & Deferred `ToolSearch`** (needed once MCP catalogs grow large).
19. **Plugin Marketplace / WASM Plugins in Desktop** (wire `coder/tui/plugins.coax` into Desktop settings).
20. **External IDE Bridges** (VS Code / JetBrains extension — lower priority since Coder Desktop has a built-in file editor & diff review pane).

---

## 4. What NOT to Copy Verbatim

- **Plan Mode (`/plan`)**: **Ruled out by admin.** Instead of a separate read-only Plan Mode, surface the agent's live to-do / task checklist directly in the UI.
- **Ink/Terminal TUI chrome**: Coder Desktop's native GPU-rendered AppKit + Coax UI (`apps/coder-desktop/`) is already superior for visual diff review, file editing, and multi-chat management.
- **Claude Code's `TeamCreate` / `SendMessage` swarm**: Coder Desktop's **Foreman** (`Foreman.swift`, `foreman-ctl`) + parallel multi-vendor sub-agents already solve multi-chat and multi-agent orchestration more cleanly.
- **Claude Code's `bridge/` / `teleport`**: Coder Desktop's `coder-host` daemon + `NIP-HOST` mobile bridge already provide persistent background runs and mobile steering without proprietary Anthropic relay infrastructure.
