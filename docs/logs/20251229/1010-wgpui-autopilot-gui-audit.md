# WGPUI Autopilot GUI Audit (2025-12-29 10:10)

## Scope
- WGPUI surface: `crates/wgpui` and the current GUI binary at `src/bin/autopilot.rs`.
- Autopilot runtime: `crates/autopilot`, `crates/claude-agent-sdk`, `crates/codex-agent-sdk`, `crates/acp-adapter`, `crates/recorder`.
- Legacy baseline: archived web GUI in `docs/legacy/gui-web` for feature parity.

## Current WGPUI Autopilot GUI (what actually runs)
- The only WGPUI desktop UI entrypoint is `src/bin/autopilot.rs`. It creates a winit window and renders a HUD-style splash screen + scrolling startup log. It is not a component-driven app.
- Rendering is manual: builds a `Scene`, paints `DotsGrid` + `Frame`, then draws text lines. No `Window`/`Component` tree or WGPUI event dispatch is used.
- Data source is `autopilot::StartupState` (`crates/autopilot/src/startup.rs`) which runs the preflight/plan/execute/review loop and appends log lines.
- Input support is minimal: mouse wheel scroll and `Esc` to exit. No text input, no clicks, no command palette, no selection.
- Autopilot GUI is not in the unified `openagents` CLI. It is invoked via cargo alias `autopilot` in `.cargo/config.toml` or instructions in `src/main.rs`.
- Docs reference a `crates/autopilot-gui` crate (`crates/README.md`, `docs/legacy/gui-web/README.md`), but it does not exist in the workspace.

## Existing WGPUI building blocks (available but not wired)
WGPUI already includes many components that map to Autopilot GUI needs, but they are not connected to any runtime data:
- Session/agent UI: `crates/wgpui/src/components/molecules/session_card.rs`, `crates/wgpui/src/components/molecules/session_search_bar.rs`, `crates/wgpui/src/components/organisms/agent_state_inspector.rs`.
- Tool call UI: `crates/wgpui/src/components/organisms/tool_call_card.rs`, `crates/wgpui/src/components/organisms/terminal_tool_call.rs`, `crates/wgpui/src/components/organisms/diff_tool_call.rs`, `crates/wgpui/src/components/organisms/search_tool_call.rs`.
- Permission UI: `crates/wgpui/src/components/organisms/permission_dialog.rs`, `crates/wgpui/src/components/molecules/permission_bar.rs`, `crates/wgpui/src/components/molecules/permission_rule_row.rs`, `crates/wgpui/src/components/molecules/permission_history_item.rs`.
- Thread/chat UI: `crates/wgpui/src/components/sections/thread_view.rs`, `crates/wgpui/src/components/sections/message_editor.rs`, `crates/wgpui/src/components/organisms/assistant_message.rs`, `crates/wgpui/src/components/organisms/user_message.rs`.
- Status + HUD: `crates/wgpui/src/components/hud/status_bar.rs`, `crates/wgpui/src/components/hud/command_palette.rs`, `crates/wgpui/src/components/hud/notifications.rs`, `crates/wgpui/src/components/atoms/daemon_status_badge.rs`, `crates/wgpui/src/components/atoms/tool_status_badge.rs`.
- Metrics: `crates/wgpui/src/components/organisms/apm_leaderboard.rs`, `crates/wgpui/src/components/molecules/apm_session_row.rs`, `crates/wgpui/src/components/atoms/apm_gauge.rs`.

## Backend surfaces already available for wiring
- Autopilot pipeline + events: `autopilot::StartupState` exposes phases, streaming Claude events, plan/report paths, and verification checklists (`crates/autopilot/src/startup.rs`).
- Claude execution is done via `claude-agent-sdk` with options for model, permission mode, budget, max turns, and cwd (`crates/claude-agent-sdk/README.md`).
- Codex execution supports model selection and sandbox configuration (`crates/codex-agent-sdk/src/options.rs`).
- ACP layer provides agent session abstraction + permission request manager suitable for UI routing (`crates/acp-adapter/README.md`).
- Trajectory logging and replay: `crates/autopilot/src/logger.rs` (rlog writer) and `crates/recorder` (rlog parser/replay).
- Issue queue: `crates/issues` + CLI tooling in `crates/issue-tool` (not yet surfaced in WGPUI).
- Daemon control: `autopilotd` + socket-based status/control in `docs/legacy/gui-web/routes/daemon.rs`.

## Legacy Web GUI feature baseline (for parity)
The archived web UI implements several features that are currently missing in WGPUI:
- Agent selection (Claude/Codex), full-auto toggle, daemon status, Claude status/usage (`docs/legacy/gui-web/views/mod.rs`, `docs/legacy/gui-web/state.rs`).
- Start/stop Autopilot subprocess with streaming output and formatted Claude event rendering (`docs/legacy/gui-web/routes/autopilot.rs`).
- ACP session management: create/list sessions, send prompt, cancel, delete; permission request handling via `PermissionRequestManager` (`docs/legacy/gui-web/routes/acp.rs`).
- Daemon start/stop/restart worker controls with known-good binary handling (`docs/legacy/gui-web/routes/daemon.rs`).
- Parallel agent controls referenced in legacy UI (`docs/legacy/gui-web/routes/parallel.rs`), but the referenced `autopilot::parallel` module no longer exists in the current crate.

## Gaps vs “full Claude Code & agent controls”

### Core GUI runtime
- No WGPUI app shell: no component tree, no input routing, no window integration beyond manual drawing (`src/bin/autopilot.rs`).
- No connection to `wgpui::Window` or a unified event pipeline (`crates/wgpui/src/window/*`).
- Desktop platform in WGPUI is a stub and does not render or process OS input (`crates/wgpui/src/platform.rs`).

### Claude Code controls
- No UI for model selection, permission mode, budget, max turns, or working directory (all supported by `claude-agent-sdk`).
- No UI for permission prompts or policy rules despite WGPUI components existing (permission dialog/history/rules).
- Autopilot uses `PermissionMode::BypassPermissions` for execution (`crates/autopilot/src/claude.rs`), so current runtime bypasses interactive approvals entirely.

### Codex / agent controls
- No UI for sandbox modes, network access toggles, or thread settings in Codex (`crates/codex-agent-sdk/src/options.rs`).
- No multi-agent orchestration UI or session routing (agent orchestrator is not surfaced).

### Session management and chat
- No session list, session detail view, or prompt composer wired to ACP sessions.
- No streaming ACP event pipeline feeding thread view, tool call cards, or diff/terminal renders.
- No tool call status timeline, file diff viewer, or command output view despite WGPUI organisms existing.

### Autopilot operations
- No UI to start/stop the daemon/worker, view health, or restart (legacy web UI supports this).
- No issue queue view (create/claim/complete/block) even though the issues DB is the primary Autopilot driver.
- No trajectory/replay UI for rlog files, plans, or verification reports.

### Discoverability & routing
- No `openagents autopilot` command: GUI entrypoint is a cargo alias only (`.cargo/config.toml`, `src/main.rs`).
- Documented `autopilot-gui` crate does not exist; ownership is unclear.

## WGPUI readiness gaps for a full desktop GUI
- Input/event model is thin (no keymap/action system, limited text input handling, no IME/clipboard integration).
- Rendering assumes text measurement via fixed-width heuristics (`crates/wgpui/src/text.rs`), which will struggle with multi-line rich text, tool output, and logs.
- Missing image/svg rendering for icons or rich media tool outputs (Zed GPUI has svg/img/canvas; WGPUI does not).

## Summary: What Zed-like “full controls” would require
To meet “full Claude Code & agent controls” inside WGPUI, the current GUI must go beyond the startup splash and wire these layers end-to-end:
- ACP session lifecycle (create, prompt, cancel, close) + UI for agent selection.
- Permission request surface tied to `PermissionRequestManager` with Allow/Deny/Always flows.
- Live tool call rendering (terminal, diff, search) fed by ACP events or rlog stream.
- Model/sandbox/budget controls for Claude + Codex + local backends.
- Daemon status + start/stop/restart controls.
- Issue queue management and run controls (start, pause, stop, resume).
- Trajectory/replay viewer for rlogs and Autopilot reports.

