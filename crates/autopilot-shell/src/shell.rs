//! Main AutopilotShell component

use autopilot::ClaudeModel;
use autopilot_service::{AutopilotRuntime, DaemonStatus, RuntimeSnapshot, SessionEvent, SessionPhase};
use tracing::info;
use wgpui::{
    Bounds, Component, EventContext, EventResult, InputEvent, PaintContext,
    components::Text,
    components::atoms::{ToolStatus, ToolType},
    components::hud::{StatusBar, StatusItem, StatusItemContent},
    components::organisms::{ThreadEntry, ThreadEntryType, ToolCallCard},
    components::sections::ThreadView,
    keymap::{Keymap, KeyContext},
};

use crate::dock::{Dock, DockPosition};
use crate::hud::{HudBackground, StartupSequence};
use crate::keymap::shell_keymap;
use crate::panels::{SessionsPanel, SystemPanel};

/// Layout dimensions calculated from current bounds
struct ShellLayout {
    left: Bounds,
    center: Bounds,
    right: Bounds,
    status: Bounds,
}

/// Main shell component combining HUD effects with dock-based layout
pub struct AutopilotShell {
    // Docks
    left_dock: Dock,
    right_dock: Dock,
    bottom_dock: Dock,

    // Center content
    thread_view: ThreadView,

    // HUD layers
    background: HudBackground,
    startup: Option<StartupSequence>,

    // Status
    status_bar: StatusBar,

    // Runtime
    runtime: AutopilotRuntime,
    last_line_count: usize,

    // Input handling
    keymap: Keymap,
    key_context: KeyContext,
}

impl AutopilotShell {
    pub fn new() -> Self {
        // Create left dock with sessions panel
        let mut left_dock = Dock::new(DockPosition::Left, 280.0);
        left_dock.add_panel(Box::new(SessionsPanel::new()));

        // Create right dock with system panel
        let mut right_dock = Dock::new(DockPosition::Right, 300.0);
        right_dock.add_panel(Box::new(SystemPanel::new()));

        // Bottom dock (empty for now)
        let bottom_dock = Dock::new(DockPosition::Bottom, 200.0);

        // Thread view for center
        let mut thread_view = ThreadView::new().auto_scroll(true);
        thread_view.push_entry(ThreadEntry::new(
            ThreadEntryType::System,
            Text::new("Autopilot Shell ready."),
        ));

        // Status bar
        let status_bar = StatusBar::new().items(vec![
            StatusItem::text("phase", "Idle").left(),
            StatusItem::text("agent", "Claude").right(),
        ]);

        // Keymap with shell bindings
        let mut keymap = shell_keymap();
        // Add default bindings
        keymap.add_bindings(wgpui::keymap::default_keymap().bindings().iter().cloned());

        Self {
            left_dock,
            right_dock,
            bottom_dock,
            thread_view,
            background: HudBackground::new(),
            startup: None, // Skip startup animation, show UI immediately
            status_bar,
            runtime: AutopilotRuntime::new(ClaudeModel::Sonnet),
            last_line_count: 0,
            keymap,
            key_context: KeyContext::new(),
        }
    }

    /// Apply a runtime snapshot to update the UI
    pub fn apply_snapshot(&mut self, snapshot: &RuntimeSnapshot) {
        self.status_bar.update_item(
            "phase",
            StatusItemContent::Text(format!("{:?}", snapshot.phase)),
        );

        // Add new log lines to thread
        if snapshot.lines.len() > self.last_line_count {
            for line in snapshot.lines.iter().skip(self.last_line_count) {
                if line.text.trim().is_empty() {
                    continue;
                }
                self.thread_view.push_entry(ThreadEntry::new(
                    ThreadEntryType::System,
                    Text::new(line.text.clone()),
                ));
            }
            self.last_line_count = snapshot.lines.len();
        }

        // Process events
        for event in &snapshot.events {
            self.push_event(event);
        }
    }

    /// Set daemon status on the system panel
    pub fn set_daemon_status(&mut self, status: DaemonStatus) {
        // Access system panel through dock
        // For now we can't easily access it, but the panel can be updated separately
        let _ = status;
    }

    fn push_event(&mut self, event: &SessionEvent) {
        match event {
            SessionEvent::Text { phase, content } => {
                let label = format!("[{}] {}", phase_label(*phase), content);
                self.thread_view.push_entry(ThreadEntry::new(
                    ThreadEntryType::Assistant,
                    Text::new(label),
                ));
            }
            SessionEvent::Tool {
                phase,
                name,
                params,
                done,
            } => {
                let tool_type = tool_type_from_name(name);
                let status = if *done {
                    ToolStatus::Success
                } else {
                    ToolStatus::Running
                };
                let tool_name = format!("{}::{}", phase_label(*phase), name);
                let card = ToolCallCard::new(tool_type, tool_name)
                    .status(status)
                    .input(params.clone());
                self.thread_view
                    .push_entry(ThreadEntry::new(ThreadEntryType::Tool, card));
            }
        }
    }

    fn calculate_layout(&self, bounds: Bounds) -> ShellLayout {
        let status_h = 28.0;

        let left_w = self.left_dock.effective_size();
        let right_w = self.right_dock.effective_size();
        let bottom_h = self.bottom_dock.effective_size();

        let left = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            left_w,
            bounds.size.height - status_h - bottom_h,
        );

        let right = Bounds::new(
            bounds.origin.x + bounds.size.width - right_w,
            bounds.origin.y,
            right_w,
            bounds.size.height - status_h - bottom_h,
        );

        let center = Bounds::new(
            bounds.origin.x + left_w,
            bounds.origin.y,
            bounds.size.width - left_w - right_w,
            bounds.size.height - status_h - bottom_h,
        );

        let status = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + bounds.size.height - status_h,
            bounds.size.width,
            status_h,
        );

        ShellLayout {
            left,
            center,
            right,
            status,
        }
    }

    fn handle_action(&mut self, action_name: &str) -> EventResult {
        match action_name {
            "shell::ToggleLeftSidebar" => {
                self.left_dock.toggle();
                EventResult::Handled
            }
            "shell::ToggleRightSidebar" => {
                self.right_dock.toggle();
                EventResult::Handled
            }
            "shell::ToggleBottomPanel" => {
                self.bottom_dock.toggle();
                EventResult::Handled
            }
            "shell::ToggleAllSidebars" => {
                // Toggle all - if any open, close all; else open all
                let any_open =
                    self.left_dock.is_open() || self.right_dock.is_open() || self.bottom_dock.is_open();
                if any_open {
                    if self.left_dock.is_open() {
                        self.left_dock.toggle();
                    }
                    if self.right_dock.is_open() {
                        self.right_dock.toggle();
                    }
                    if self.bottom_dock.is_open() {
                        self.bottom_dock.toggle();
                    }
                } else {
                    if !self.left_dock.is_open() {
                        self.left_dock.toggle();
                    }
                    if !self.right_dock.is_open() {
                        self.right_dock.toggle();
                    }
                }
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}

impl Default for AutopilotShell {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for AutopilotShell {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // 1. Always paint background dots
        self.background.paint(bounds, cx);

        // 2. If startup animation running, paint that and return
        if let Some(ref mut startup) = self.startup {
            startup.paint(bounds, cx);
            if startup.is_complete() {
                self.startup = None;
            }
            return;
        }

        // 3. Tick runtime and apply snapshot
        self.runtime.tick();
        let snapshot = self.runtime.snapshot();
        self.apply_snapshot(&snapshot);

        // 4. Calculate layout
        let layout = self.calculate_layout(bounds);

        // 5. Paint docks
        self.left_dock.paint(layout.left, cx);
        self.right_dock.paint(layout.right, cx);

        // 6. Paint center thread view
        self.thread_view.paint(layout.center, cx);

        // 7. Paint status bar
        self.status_bar.paint(layout.status, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        // During startup, ignore events (but also check if startup is complete)
        if let Some(ref startup) = self.startup {
            if !startup.is_complete() {
                info!("Shell: ignoring event during startup");
                return EventResult::Ignored;
            }
            // Startup complete, clear it
            info!("Shell: startup complete, clearing");
        }
        // Clear startup if complete
        if self.startup.as_ref().is_some_and(|s| s.is_complete()) {
            self.startup = None;
        }

        // Handle keyboard via keymap
        if let InputEvent::KeyDown { key, modifiers } = event {
            info!("Shell: KeyDown {:?} with modifiers {:?}", key, modifiers);
            info!("Shell: keymap has {} bindings", self.keymap.len());

            if let Some(action) = self.keymap.match_keystroke(key, modifiers, &self.key_context) {
                info!("Shell: matched action: {}", action.name());
                let result = self.handle_action(action.name());
                info!("Shell: action result: {:?}", result);
                if result.is_handled() {
                    return result;
                }
            } else {
                info!("Shell: no action matched");
            }
        }

        // Calculate layout for event routing
        let layout = self.calculate_layout(bounds);

        // Route to docks
        if self.left_dock.is_open() {
            if let EventResult::Handled = self.left_dock.event(event, layout.left, cx) {
                return EventResult::Handled;
            }
        }

        if self.right_dock.is_open() {
            if let EventResult::Handled = self.right_dock.event(event, layout.right, cx) {
                return EventResult::Handled;
            }
        }

        // Route to thread view
        self.thread_view.event(event, layout.center, cx)
    }
}

fn phase_label(phase: SessionPhase) -> &'static str {
    match phase {
        SessionPhase::Plan => "Plan",
        SessionPhase::Execute => "Exec",
        SessionPhase::Review => "Review",
        SessionPhase::Fix => "Fix",
    }
}

fn tool_type_from_name(name: &str) -> ToolType {
    match name.to_ascii_lowercase().as_str() {
        "read" => ToolType::Read,
        "write" => ToolType::Write,
        "edit" => ToolType::Edit,
        "bash" => ToolType::Bash,
        "search" => ToolType::Search,
        "glob" => ToolType::Glob,
        "grep" => ToolType::Grep,
        "list" => ToolType::List,
        "task" => ToolType::Task,
        "webfetch" | "web_fetch" => ToolType::WebFetch,
        _ => ToolType::Unknown,
    }
}
