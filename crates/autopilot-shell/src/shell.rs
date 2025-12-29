//! Main AutopilotShell component

use autopilot::ClaudeModel;
use autopilot_service::{AutopilotRuntime, DaemonStatus, RuntimeSnapshot, SessionEvent, SessionPhase};
use tracing::info;
use wgpui::{
    Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext, Point,
    components::Text,
    components::atoms::{ToolStatus, ToolType},
    components::hud::{CornerConfig, Frame, StatusBar, StatusItem, StatusItemContent},
    components::organisms::{ThreadEntry, ThreadEntryType, ToolCallCard},
    components::sections::ThreadView,
    keymap::{Keymap, KeyContext},
};

use crate::components::FullAutoToggle;
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

    // Full Auto toggle
    full_auto_toggle: FullAutoToggle,

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
            Text::new("Autopilot ready."),
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
            full_auto_toggle: FullAutoToggle::new(),
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
            "shell::ToggleFullAuto" => {
                self.full_auto_toggle.toggle();
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }

    fn paint_hotkey_legend(&self, bounds: Bounds, cx: &mut PaintContext) {
        let hotkeys = [
            ("cmd-f", "Toggle Full Auto"),
            ("cmd-b", "Toggle left sidebar"),
            ("cmd-shift-b", "Toggle right sidebar"),
            ("cmd-\\", "Toggle all sidebars"),
            ("esc", "Exit"),
        ];

        let line_height = 18.0;
        let padding = 12.0;
        let legend_w = 220.0;
        let legend_h = (hotkeys.len() as f32 * line_height) + padding * 2.0;

        // Position in bottom left, above status bar
        let legend_x = bounds.origin.x + padding;
        let legend_y = bounds.origin.y + bounds.size.height - 28.0 - legend_h - padding;

        let legend_bounds = Bounds::new(legend_x, legend_y, legend_w, legend_h);

        // Draw HUD frame
        let line_color = Hsla::new(0.0, 0.0, 0.4, 0.5);
        let bg_color = Hsla::new(0.0, 0.0, 0.05, 0.9);

        let mut frame = Frame::nefrex()
            .line_color(line_color)
            .bg_color(bg_color)
            .stroke_width(1.0)
            .corner_config(CornerConfig::all())
            .square_size(4.0)
            .small_line_length(4.0)
            .large_line_length(12.0);
        frame.paint(legend_bounds, cx);

        // Draw hotkey text
        let text_color = Hsla::new(0.0, 0.0, 0.6, 0.9);
        let key_color = Hsla::new(180.0, 0.5, 0.6, 0.9); // Cyan for keys
        let font_size = 11.0;

        let mut y = legend_y + padding;
        for (key, desc) in &hotkeys {
            // Key
            let key_run = cx.text.layout(
                key,
                Point::new(legend_x + padding, y),
                font_size,
                key_color,
            );
            cx.scene.draw_text(key_run);

            // Description
            let desc_run = cx.text.layout(
                desc,
                Point::new(legend_x + padding + 90.0, y),
                font_size,
                text_color,
            );
            cx.scene.draw_text(desc_run);

            y += line_height;
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

        // 3. Only tick runtime when Full Auto is enabled
        if self.full_auto_toggle.is_enabled() {
            self.runtime.tick();
            let snapshot = self.runtime.snapshot();
            self.apply_snapshot(&snapshot);
        }

        // 4. Calculate layout
        let layout = self.calculate_layout(bounds);

        // 5. Paint docks
        self.left_dock.paint(layout.left, cx);
        self.right_dock.paint(layout.right, cx);

        // 5b. Paint Full Auto toggle at top of right sidebar
        if self.right_dock.is_open() {
            let toggle_bounds = Bounds::new(
                layout.right.origin.x + 16.0,
                layout.right.origin.y + 16.0,
                layout.right.size.width - 32.0,
                36.0,
            );
            self.full_auto_toggle.paint(toggle_bounds, cx);
        }

        // 6. Paint Kranox frame around center area
        let center_margin = 8.0;
        let center_frame_bounds = Bounds::new(
            layout.center.origin.x + center_margin,
            layout.center.origin.y + center_margin,
            layout.center.size.width - center_margin * 2.0,
            layout.center.size.height - center_margin * 2.0,
        );

        let frame_color = Hsla::new(0.0, 0.0, 0.3, 0.4);
        let frame_bg = Hsla::new(0.0, 0.0, 0.02, 0.8);
        let mut center_frame = Frame::kranox()
            .line_color(frame_color)
            .bg_color(frame_bg)
            .stroke_width(1.0);
        center_frame.paint(center_frame_bounds, cx);

        // 7. Paint center thread view inside frame
        let thread_padding = 16.0;
        let thread_bounds = Bounds::new(
            center_frame_bounds.origin.x + thread_padding,
            center_frame_bounds.origin.y + thread_padding,
            center_frame_bounds.size.width - thread_padding * 2.0,
            center_frame_bounds.size.height - thread_padding * 2.0,
        );
        self.thread_view.paint(thread_bounds, cx);

        // 8. Paint status bar
        self.status_bar.paint(layout.status, cx);

        // 9. Paint hotkey legend in bottom left
        self.paint_hotkey_legend(bounds, cx);
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
