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


## Wiring plan (ACP, Claude, Codex to WGPUI components)

### Data sources and bridges
- Autopilot engine (in-process): `autopilot::StartupState` and its `claude_events`, `exec_events`, `review_events`, `fix_events` streams from `crates/autopilot/src/startup.rs`.
- ACP sessions (out-of-process): `acp_adapter::AcpAgentConnection` plus `RlogStreamer` for event ingestion; `PermissionRequestManager` for UI-bound approvals.
- Claude/Codex controls: `claude_agent_sdk::QueryOptions` and `codex_agent_sdk::ThreadOptions` for model, budget, cwd, sandbox, and network access.
- Daemon status/control: poll socket used in `docs/legacy/gui-web/routes/daemon.rs` (reuse the same IPC logic but drive WGPUI state).
- Issues queue: `crates/issues` for list/create/claim/complete/block; keep the UI as the control surface for Autopilot runs.
- Trajectory/replay: tail rlog files using `crates/recorder` and feed into thread/tool call views.

### Component mapping (event to view)
- Session list
  - ACP session list -> `SessionCard` + `SessionSearchBar` (left pane).
  - Session selection -> update active session and thread view.
- Thread view
  - `acp::SessionUpdate::UserMessageChunk` -> `ThreadEntry(User)` with `UserMessage` or `Text`.
  - `acp::SessionUpdate::AgentMessageChunk` -> `ThreadEntry(Assistant)` with `AssistantMessage` or `Text`.
  - `acp::SessionUpdate::ToolCall` + `ToolCallUpdate` -> `ThreadEntry(Tool)` with `ToolCallCard`, `TerminalToolCall`, `DiffToolCall`, `SearchToolCall`.
  - Autopilot `StartupState.lines` -> `ThreadEntry(System)` for phase logs.
- Permissions
  - `PermissionRequestManager` -> `PermissionDialog` (modal) or `PermissionBar` (inline) with `PermissionHistoryItem` and `PermissionRuleRow` for policy feedback.
- Status + controls
  - `DaemonStatusBadge`, `StatusBar`, `CommandPalette`, `Notifications` for global status, command actions, and feedback.
  - `AgentStateInspector` for detailed agent runtime state (goals, tasks, memory).
- Metrics
  - `ApmTelemetry` (from ACP telemetry example) -> `ApmLeaderboard`, `ApmSessionRow`, `ApmGauge`.
- Issues and tasks
  - `IssueRow` for backlog and in-flight status; map issue actions to `issues` crate APIs.

### Event routing and state model
- Maintain a single UI state store (e.g., `AutopilotUiState`) holding:
  - Sessions list, active session id, thread entries, tool call state, permission queue, daemon status, and issue list.
- Background tasks publish `UiEvent` messages into a channel; the UI thread consumes and mutates `AutopilotUiState` and triggers redraw.
- WGPUI event loop takes `InputEvent` and dispatches to root `Component`; use `EventContext` for focus/action dispatch and `Keymap` for command bindings.

### Command and keybinding plumbing
- Use `wgpui::action` + `wgpui::keymap` to bind actions such as:
  - Start/stop session
  - Approve/deny permission
  - Toggle full auto
  - Open command palette
  - Focus next/previous session

## Implementation checklist (full Claude Code and agent controls)

### Phase 0 - Decide runtime boundaries
- Choose in-process Autopilot (direct `autopilot` crate calls) vs. spawning subprocess for isolation.
- Choose ACP session driver (use `acp-adapter` for Claude and Codex) and the source of truth for session logs (rlog vs live stream).

### Phase 1 - WGPUI shell and event loop
- Replace the manual scene rendering in `src/bin/autopilot.rs` with a root `Component` tree.
- Add WGPUI input mapping from winit events to `InputEvent` and route into `Component::event`.
- Add a global `AutopilotUiState` and `UiEvent` channel to update state from background tasks.

### Phase 2 - Session and chat plumbing
- Implement ACP session list and selection (left pane using `SessionCard`).
- Wire `MessageEditor` to send prompts through ACP session.
- Stream ACP events into `ThreadView` with tool call cards and assistant/user entries.

### Phase 3 - Claude/Codex controls
- Add model selector for Claude and Codex (model list + active model).
- Add toggles for permission mode, sandbox mode, network access, cwd.
- Add budget and max-turn settings tied to `QueryOptions` / `ThreadOptions`.

### Phase 4 - Permissions and policy
- Wire `PermissionRequestManager` to WGPUI `PermissionDialog` and `PermissionBar`.
- Surface permission history and rule overrides using `PermissionHistoryItem` and `PermissionRuleRow`.

### Phase 5 - Autopilot pipeline controls
- Add start/stop/pause/resume controls for the Autopilot run loop.
- Show phase status (preflight, planning, execution, review, verification) in `StatusBar`.
- Map `StartupState` and report outputs to visible panels.

### Phase 6 - Daemon and workers
- Add daemon status polling and controls (start/stop/restart worker) using the same IPC logic as legacy web UI.
- Display `DaemonStatusBadge` and worker metrics.

### Phase 7 - Issues and trajectories
- Add issue list view, detail view, and actions (create/claim/complete/block).
- Add trajectory/replay view backed by `crates/recorder` parsing.

### Phase 8 - Testing and stability
- Add WGPUI component tests for core panels.
- Validate performance on large thread logs and multi-session lists.

## Sample WGPUI shell (sketch for #3)

```rust
use std::sync::Arc;
use wgpui::{
    Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext, Point, Quad,
    Scene, Size, TextSystem, theme,
};
use wgpui::components::Text;
use wgpui::components::hud::{StatusBar, StatusItem};
use wgpui::components::molecules::{SessionCard, SessionInfo};
use wgpui::components::organisms::{ThreadEntry, ThreadEntryType};
use wgpui::components::sections::ThreadView;
use wgpui::renderer::Renderer;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton as WinitMouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

struct AutopilotShell {
    session: SessionCard,
    thread: ThreadView,
    status: StatusBar,
}

impl AutopilotShell {
    fn new() -> Self {
        let session = SessionCard::new(
            SessionInfo::new("sess-1", "Autopilot Run")
                .model("claude-sonnet-4-5")
                .task_count(3)
        );

        let mut thread = ThreadView::new().auto_scroll(true);
        thread.push_entry(ThreadEntry::new(
            ThreadEntryType::Assistant,
            Text::new("Ready to start.")
        ));

        let status = StatusBar::new().items(vec![
            StatusItem::text("phase", "Idle").left(),
            StatusItem::text("agent", "Claude").right(),
        ]);

        Self { session, thread, status }
    }
}

impl Component for AutopilotShell {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(Quad::new(bounds).with_background(theme::bg::APP));

        let left_w = 280.0;
        let right_w = 300.0;
        let status_h = 28.0;

        let left = Bounds::new(bounds.origin.x, bounds.origin.y, left_w, bounds.size.height);
        let center = Bounds::new(
            bounds.origin.x + left_w,
            bounds.origin.y,
            bounds.size.width - left_w - right_w,
            bounds.size.height - status_h,
        );
        let right = Bounds::new(
            bounds.origin.x + bounds.size.width - right_w,
            bounds.origin.y,
            right_w,
            bounds.size.height,
        );
        let status = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + bounds.size.height - status_h,
            bounds.size.width,
            status_h,
        );

        self.session.paint(left, cx);
        self.thread.paint(center, cx);
        self.status.paint(status, cx);

        // Placeholder right rail
        cx.scene.draw_quad(
            Quad::new(right).with_background(theme::bg::SURFACE)
        );
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let left_w = 280.0;
        let right_w = 300.0;
        let status_h = 28.0;

        let left = Bounds::new(bounds.origin.x, bounds.origin.y, left_w, bounds.size.height);
        let center = Bounds::new(
            bounds.origin.x + left_w,
            bounds.origin.y,
            bounds.size.width - left_w - right_w,
            bounds.size.height - status_h,
        );
        let status = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + bounds.size.height - status_h,
            bounds.size.width,
            status_h,
        );

        if self.session.event(event, left, cx).is_handled() {
            return EventResult::Handled;
        }
        if self.thread.event(event, center, cx).is_handled() {
            return EventResult::Handled;
        }
        self.status.event(event, status, cx)
    }
}

struct App {
    state: Option<RenderState>,
}

struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    root: AutopilotShell,
    event_cx: EventContext,
}

impl Default for App {
    fn default() -> Self {
        Self { state: None }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .expect("create window")
        );

        let state = pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
            let surface = instance.create_surface(window.clone()).expect("surface");
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    ..Default::default()
                })
                .await
                .expect("adapter");

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("device");

            let size = window.inner_size();
            let caps = surface.get_capabilities(&adapter);
            let format = caps.formats[0];
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer: Renderer::new(&device, format),
                text_system: TextSystem::new(1.0),
                root: AutopilotShell::new(),
                event_cx: EventContext::new(),
            }
        });

        self.state = Some(state);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else { return; };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                state.config.width = size.width.max(1);
                state.config.height = size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let input = InputEvent::MouseMove { x: position.x as f32, y: position.y as f32 };
                let bounds = Bounds::new(0.0, 0.0, state.config.width as f32, state.config.height as f32);
                let _ = state.root.event(&input, bounds, &mut state.event_cx);
                state.window.request_redraw();
            }
            WindowEvent::MouseInput { state: btn_state, button, .. } => {
                let button = match button {
                    WinitMouseButton::Left => wgpui::MouseButton::Left,
                    WinitMouseButton::Right => wgpui::MouseButton::Right,
                    WinitMouseButton::Middle => wgpui::MouseButton::Middle,
                    _ => wgpui::MouseButton::Left,
                };
                let input = match btn_state {
                    ElementState::Pressed => InputEvent::MouseDown { button, x: 0.0, y: 0.0 },
                    ElementState::Released => InputEvent::MouseUp { button, x: 0.0, y: 0.0 },
                };
                let bounds = Bounds::new(0.0, 0.0, state.config.width as f32, state.config.height as f32);
                let _ = state.root.event(&input, bounds, &mut state.event_cx);
                state.window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 40.0,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32,
                };
                let input = InputEvent::Scroll { dx: 0.0, dy };
                let bounds = Bounds::new(0.0, 0.0, state.config.width as f32, state.config.height as f32);
                let _ = state.root.event(&input, bounds, &mut state.event_cx);
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed()
                    && matches!(event.physical_key, PhysicalKey::Code(KeyCode::Escape))
                {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                let mut scene = Scene::new();
                let mut cx = PaintContext::new(&mut scene, &mut state.text_system, 1.0);
                let bounds = Bounds::new(0.0, 0.0, state.config.width as f32, state.config.height as f32);
                state.root.paint(bounds, &mut cx);

                if state.text_system.is_dirty() {
                    state.renderer.update_atlas(
                        &state.queue,
                        state.text_system.atlas_data(),
                        state.text_system.atlas_size(),
                    );
                    state.text_system.mark_clean();
                }

                let output = state.surface.get_current_texture().expect("swapchain");
                let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder = state.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
                state.renderer.prepare(&state.device, &scene);
                state.renderer.render(&mut encoder, &view);
                state.queue.submit(Some(encoder.finish()));
                output.present();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("run app");
}
```
