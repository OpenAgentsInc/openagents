//! Main AutopilotShell component

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use autopilot::{ClaudeModel, LogLine, LogStatus, SessionCheckpoint, StartupSection};
use autopilot_service::{AutopilotRuntime, DaemonStatus, LogSection, RuntimeSnapshot, SessionEvent, SessionPhase};
use tracing::info;
use wgpui::{
    Bounds, Component, EventContext, EventResult, Hsla, InputEvent, PaintContext,
    components::Text,
    components::atoms::{ToolStatus, ToolType},
    components::hud::{Frame, StatusBar, StatusItem, StatusItemContent},
    components::molecules::{CollapsibleSection, SectionStatus},
    components::organisms::{ChildTool, ThreadEntry, ThreadEntryType, ToolCallCard},
    components::sections::ThreadView,
    keymap::{Keymap, KeyContext},
};

use crate::components::FullAutoToggle;
use crate::dock::{Dock, DockPosition, Panel};
use crate::hud::{HudBackground, StartupSequence};
use crate::keymap::shell_keymap;
use crate::panels::{SessionsPanel, SystemPanel, UsageLimit};
use crate::rate_limits::{RateLimitFetcher, RateLimitSnapshot};

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

    // Right sidebar panel (not in dock for direct access)
    system_panel: SystemPanel,

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
    last_section_count: usize,

    // Collapsible section state (which sections are expanded)
    expanded_sections: HashSet<StartupSection>,

    // Track in-flight tool calls: tool_key -> entry_index
    pending_tools: HashMap<String, usize>,

    // Active Task tool for nesting child tools: (tool_key, entry_index)
    active_task: Option<(String, usize)>,

    // Full Auto toggle
    full_auto_toggle: FullAutoToggle,

    // Input handling
    keymap: Keymap,
    key_context: KeyContext,

    // Window requests
    pending_fullscreen_toggle: bool,

    // Rate limit state
    #[allow(dead_code)]
    rate_limit_fetcher: RateLimitFetcher,
    rate_limits: Arc<Mutex<Option<RateLimitSnapshot>>>,

    // Working directory for checkpoint saves
    working_dir: PathBuf,
}

impl AutopilotShell {
    pub fn new() -> Self {
        // Create left dock with sessions panel
        let mut left_dock = Dock::new(DockPosition::Left, 280.0);
        left_dock.add_panel(Box::new(SessionsPanel::new()));

        // Create right dock (empty - we render system panel directly for data access)
        let right_dock = Dock::new(DockPosition::Right, 300.0);

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

        // Keymap with default bindings, then shell overrides
        let mut keymap = wgpui::keymap::default_keymap();
        // Add shell bindings (these override defaults due to later-wins precedence)
        keymap.add_bindings(shell_keymap().bindings().iter().cloned());

        // Rate limit fetcher and shared state
        let rate_limit_fetcher = RateLimitFetcher::new();
        let rate_limits: Arc<Mutex<Option<RateLimitSnapshot>>> = Arc::new(Mutex::new(None));

        // Spawn background thread to fetch rate limits on startup
        if rate_limit_fetcher.can_fetch() {
            let fetcher = rate_limit_fetcher.clone();
            let limits_arc = rate_limits.clone();
            std::thread::spawn(move || {
                // Create tokio runtime for the async fetch
                let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
                if let Ok(snapshot) = rt.block_on(fetcher.fetch_rate_limits()) {
                    info!("Fetched initial rate limits: {:?}", snapshot.primary.as_ref().map(|p| p.used_percent));
                    if let Ok(mut guard) = limits_arc.lock() {
                        *guard = Some(snapshot);
                    }
                }
            });
        }

        // Create system panel and set initial subscription info from credentials
        let mut system_panel = SystemPanel::new();
        if rate_limit_fetcher.has_credentials() {
            let tier = rate_limit_fetcher.rate_limit_tier().unwrap_or("unknown");
            let sub = rate_limit_fetcher.subscription_type().unwrap_or("free");
            info!("Claude subscription: {} (tier: {})", sub, tier);

            // Extract multiplier from tier (e.g., "default_claude_max_20x" -> "20x")
            let multiplier = tier
                .rsplit('_')
                .next()
                .filter(|s| s.ends_with('x') && s.len() > 1 && s[..s.len()-1].chars().all(|c| c.is_ascii_digit()))
                .unwrap_or("");

            // Show subscription type in UI (actual usage % comes from API responses)
            let sub_display = match sub {
                "max" => {
                    if !multiplier.is_empty() {
                        format!("Claude Max ({})", multiplier)
                    } else {
                        "Claude Max".to_string()
                    }
                }
                "pro" => "Claude Pro".to_string(),
                _ => sub.to_string(),
            };
            system_panel.update_limits(vec![UsageLimit {
                name: sub_display,
                percent_used: 0.0, // Will update from API responses
                resets_at: "usage updates on API calls".to_string(),
            }]);
        }

        Self {
            left_dock,
            right_dock,
            bottom_dock,
            system_panel,
            thread_view,
            background: HudBackground::new(),
            startup: None, // Skip startup animation, show UI immediately
            status_bar,
            runtime: AutopilotRuntime::new(ClaudeModel::Sonnet),
            last_line_count: 0,
            last_section_count: 0,
            expanded_sections: HashSet::new(), // All sections start collapsed
            pending_tools: HashMap::new(),
            active_task: None,
            full_auto_toggle: FullAutoToggle::new(),
            keymap,
            key_context: KeyContext::new(),
            pending_fullscreen_toggle: false,
            rate_limit_fetcher,
            rate_limits,
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    /// Create a shell from a saved checkpoint.
    pub fn with_checkpoint(cp: SessionCheckpoint) -> Self {
        let working_dir = cp.working_dir.clone();

        // Create left dock with sessions panel
        let mut left_dock = Dock::new(DockPosition::Left, 280.0);
        left_dock.add_panel(Box::new(SessionsPanel::new()));

        // Create right dock (empty - we render system panel directly for data access)
        let right_dock = Dock::new(DockPosition::Right, 300.0);

        // Bottom dock (empty for now)
        let bottom_dock = Dock::new(DockPosition::Bottom, 200.0);

        // Thread view for center - will be populated from checkpoint state
        let mut thread_view = ThreadView::new().auto_scroll(true);
        thread_view.push_entry(ThreadEntry::new(
            ThreadEntryType::System,
            Text::new("Session resumed."),
        ));

        // Status bar
        let status_bar = StatusBar::new().items(vec![
            StatusItem::text("phase", "Resumed").left(),
            StatusItem::text("agent", "Claude").right(),
        ]);

        // Keymap
        let mut keymap = wgpui::keymap::default_keymap();
        keymap.add_bindings(shell_keymap().bindings().iter().cloned());

        // Rate limit fetcher
        let rate_limit_fetcher = RateLimitFetcher::new();
        let rate_limits: Arc<Mutex<Option<RateLimitSnapshot>>> = Arc::new(Mutex::new(None));

        // Fetch rate limits in background
        if rate_limit_fetcher.can_fetch() {
            let fetcher = rate_limit_fetcher.clone();
            let limits_arc = rate_limits.clone();
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
                if let Ok(snapshot) = rt.block_on(fetcher.fetch_rate_limits()) {
                    if let Ok(mut guard) = limits_arc.lock() {
                        *guard = Some(snapshot);
                    }
                }
            });
        }

        // Create system panel
        let mut system_panel = SystemPanel::new();
        if rate_limit_fetcher.has_credentials() {
            let tier = rate_limit_fetcher.rate_limit_tier().unwrap_or("unknown");
            let sub = rate_limit_fetcher.subscription_type().unwrap_or("free");
            let multiplier = tier
                .rsplit('_')
                .next()
                .filter(|s| s.ends_with('x') && s.len() > 1 && s[..s.len()-1].chars().all(|c| c.is_ascii_digit()))
                .unwrap_or("");

            let sub_display = match sub {
                "max" => {
                    if !multiplier.is_empty() {
                        format!("Claude Max ({})", multiplier)
                    } else {
                        "Claude Max".to_string()
                    }
                }
                "pro" => "Claude Pro".to_string(),
                _ => sub.to_string(),
            };
            system_panel.update_limits(vec![UsageLimit {
                name: sub_display,
                percent_used: 0.0,
                resets_at: "usage updates on API calls".to_string(),
            }]);
        }

        // Create runtime from checkpoint
        let runtime = AutopilotRuntime::from_checkpoint(cp);

        Self {
            left_dock,
            right_dock,
            bottom_dock,
            system_panel,
            thread_view,
            background: HudBackground::new(),
            startup: None,
            status_bar,
            runtime,
            last_line_count: 0,
            last_section_count: 0,
            expanded_sections: HashSet::new(),
            pending_tools: HashMap::new(),
            active_task: None,
            full_auto_toggle: FullAutoToggle::new(),
            keymap,
            key_context: KeyContext::new(),
            pending_fullscreen_toggle: false,
            rate_limit_fetcher,
            rate_limits,
            working_dir,
        }
    }

    /// Set the working directory for checkpoint saves.
    pub fn set_working_dir(&mut self, path: PathBuf) {
        self.working_dir = path;
    }

    /// Save current session to a checkpoint file.
    pub fn save_checkpoint(&self) -> Result<PathBuf, std::io::Error> {
        self.runtime.save_checkpoint(self.working_dir.clone())
    }

    /// Get the current session ID.
    pub fn session_id(&self) -> &str {
        self.runtime.session_id()
    }

    /// Check and consume pending fullscreen toggle request
    pub fn take_fullscreen_toggle(&mut self) -> bool {
        std::mem::take(&mut self.pending_fullscreen_toggle)
    }

    /// Apply a runtime snapshot to update the UI
    pub fn apply_snapshot(&mut self, snapshot: &RuntimeSnapshot) {
        self.status_bar.update_item(
            "phase",
            StatusItemContent::Text(format!("{:?}", snapshot.phase)),
        );

        // Update ClaudeUsage with model info from runtime
        let model_str = snapshot.model.as_str();
        let context_total = match snapshot.model {
            ClaudeModel::Sonnet => 200_000,
            ClaudeModel::Opus => 200_000,
        };
        // Context used would come from actual API response - for now show 0
        self.system_panel.update_usage(model_str, 0, context_total);

        // Rebuild thread view when sections change
        if snapshot.sections.len() != self.last_section_count {
            self.rebuild_with_sections(&snapshot.sections, &snapshot.lines);
            self.last_section_count = snapshot.sections.len();
            self.last_line_count = snapshot.lines.len();
        } else {
            // Add new Claude (non-section) lines incrementally
            self.add_claude_lines(&snapshot.lines);
        }

        // Process events
        for event in &snapshot.events {
            self.push_event(event);
        }
    }

    /// Rebuild thread view with collapsible sections for startup messages.
    fn rebuild_with_sections(&mut self, sections: &[LogSection], lines: &[LogLine]) {
        self.thread_view.clear();

        // Add "Autopilot ready." header
        self.thread_view.push_entry(ThreadEntry::new(
            ThreadEntryType::System,
            Text::new("Autopilot ready."),
        ));

        // Add collapsible sections for startup phases
        for section in sections {
            let is_expanded = self.expanded_sections.contains(&section.section);
            let status = match section.summary_status {
                LogStatus::Success => SectionStatus::Success,
                LogStatus::Error => SectionStatus::Error,
                LogStatus::Pending => SectionStatus::Pending,
                _ => SectionStatus::InProgress,
            };

            let details: Vec<String> = section
                .details
                .iter()
                .map(|l| l.text.clone())
                .filter(|t| !t.trim().is_empty())
                .collect();

            let collapsible = CollapsibleSection::new(&section.summary)
                .expanded(is_expanded)
                .status(status)
                .details(details);

            self.thread_view.push_entry(ThreadEntry::new(
                ThreadEntryType::System,
                collapsible,
            ));
        }

        // Add Claude (non-section) lines
        for line in lines {
            if line.section == Some(StartupSection::Claude) && !line.text.trim().is_empty() {
                self.thread_view.push_entry(ThreadEntry::new(
                    ThreadEntryType::System,
                    Text::new(line.text.clone()),
                ));
            }
        }
    }

    /// Add new Claude lines incrementally (for streaming updates).
    fn add_claude_lines(&mut self, lines: &[LogLine]) {
        if lines.len() > self.last_line_count {
            for line in lines.iter().skip(self.last_line_count) {
                // Only add Claude section lines (startup sections are in collapsible)
                if line.section == Some(StartupSection::Claude) && !line.text.trim().is_empty() {
                    self.thread_view.push_entry(ThreadEntry::new(
                        ThreadEntryType::System,
                        Text::new(line.text.clone()),
                    ));
                }
            }
            self.last_line_count = lines.len();
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
            SessionEvent::Text { .. } => {
                // Text content is already shown via log lines, skip duplicate display
            }
            SessionEvent::Tool {
                phase,
                name,
                params,
                done,
            } => {
                let tool_type = tool_type_from_name(name);
                // Use phase::name + truncated params as unique key (multiple tools can have same name)
                let params_hash = params.chars().take(60).collect::<String>();
                let tool_key = format!("{}::{}::{}", phase_label(*phase), name, params_hash);
                let display_name = format!("{}::{}", phase_label(*phase), name);
                let is_task = name == "Task";

                if *done {
                    // Tool completed - update existing entry's status
                    if let Some(entry_idx) = self.pending_tools.remove(&tool_key) {
                        if let Some(entry) = self.thread_view.entry_mut(entry_idx) {
                            // Replace with completed card
                            let card = ToolCallCard::new(tool_type, display_name)
                                .status(ToolStatus::Success)
                                .input(params.clone());
                            entry.set_content(card);
                        }
                    }

                    // If this was the active task, clear it
                    if let Some((ref active_key, _)) = self.active_task {
                        if *active_key == tool_key {
                            self.active_task = None;
                        }
                    }

                    // Also update child tool status if there's an active task
                    if let Some((_, parent_idx)) = self.active_task {
                        if let Some(entry) = self.thread_view.entry_mut(parent_idx) {
                            // Try to downcast to ToolCallCard and update child
                            // Note: This requires the entry content to be ToolCallCard
                            // We use a workaround by recreating with updated child
                        }
                    }
                } else {
                    // Tool starting
                    if is_task {
                        // Task tool - add to main thread and set as active
                        let entry_idx = self.thread_view.entry_count();
                        self.pending_tools.insert(tool_key.clone(), entry_idx);
                        self.active_task = Some((tool_key.clone(), entry_idx));

                        let card = ToolCallCard::new(tool_type, display_name)
                            .status(ToolStatus::Running)
                            .input(params.clone());
                        self.thread_view
                            .push_entry(ThreadEntry::new(ThreadEntryType::Tool, card));
                    } else if let Some((_, parent_idx)) = self.active_task {
                        // Non-Task tool with active parent - add as child
                        // We need to add child to the parent entry
                        // Since we can't easily modify BoxedComponent, track separately
                        // For now, still add to pending but don't create new entry
                        self.pending_tools.insert(tool_key.clone(), parent_idx);

                        // Add child to parent card if we can access it
                        if let Some(entry) = self.thread_view.entry_mut(parent_idx) {
                            // Create child tool
                            let child = ChildTool {
                                tool_type,
                                name: name.clone(),
                                params: params.clone(),
                                status: ToolStatus::Running,
                            };
                            // We need to access the ToolCallCard to add the child
                            // Since content is Box<dyn Component>, we need a workaround
                            // For now, we'll need to store children separately or modify architecture

                            // Store in a separate map: parent_entry_idx -> Vec<ChildTool>
                            // For now, let's just add children directly to main feed with indent marker
                        }

                        // Fallback: Add as indented entry in main thread
                        let card = ToolCallCard::new(tool_type, format!("  {}", name))
                            .status(ToolStatus::Running)
                            .input(params.clone());
                        // Override entry_idx to be the actual new entry
                        let actual_idx = self.thread_view.entry_count();
                        self.pending_tools.insert(tool_key.clone(), actual_idx);
                        self.thread_view
                            .push_entry(ThreadEntry::new(ThreadEntryType::Tool, card));
                    } else {
                        // No active task - add to main thread normally
                        let entry_idx = self.thread_view.entry_count();
                        self.pending_tools.insert(tool_key.clone(), entry_idx);

                        let card = ToolCallCard::new(tool_type, display_name)
                            .status(ToolStatus::Running)
                            .input(params.clone());
                        self.thread_view
                            .push_entry(ThreadEntry::new(ThreadEntryType::Tool, card));
                    }
                }
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
                let was_enabled = self.full_auto_toggle.is_enabled();
                self.full_auto_toggle.toggle();

                // Save checkpoint when Full Auto is toggled OFF
                if was_enabled && !self.full_auto_toggle.is_enabled() {
                    if let Err(e) = self.save_checkpoint() {
                        info!("Failed to save checkpoint: {}", e);
                    } else {
                        info!("Checkpoint saved for session {}", self.session_id());
                    }
                }

                EventResult::Handled
            }
            "shell::ToggleFullscreen" => {
                self.pending_fullscreen_toggle = true;
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }

    /// Update UI with current rate limits from shared state
    fn update_rate_limits_ui(&mut self) {
        if let Ok(guard) = self.rate_limits.lock() {
            if let Some(ref snapshot) = *guard {
                let mut limits = Vec::new();

                // Add primary limit (usually weekly)
                if let Some(ref primary) = snapshot.primary {
                    limits.push(UsageLimit {
                        name: primary.window_name().to_string(),
                        percent_used: primary.used_percent,
                        resets_at: primary.format_reset(),
                    });
                }

                // Add secondary limit (usually daily/session)
                if let Some(ref secondary) = snapshot.secondary {
                    limits.push(UsageLimit {
                        name: secondary.window_name().to_string(),
                        percent_used: secondary.used_percent,
                        resets_at: secondary.format_reset(),
                    });
                }

                self.system_panel.update_limits(limits);
            }
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

        // 3b. Check for rate limit updates and apply to UI
        self.update_rate_limits_ui();

        // 4. Calculate layout
        let layout = self.calculate_layout(bounds);

        // 5. Paint docks
        self.left_dock.paint(layout.left, cx);

        // 5b. Paint right sidebar with system panel (rendered directly for data access)
        if self.right_dock.is_open() {
            // Draw line frame for right sidebar
            let line_color = Hsla::new(0.0, 0.0, 0.3, 0.5);
            let bg_color = Hsla::new(0.0, 0.0, 0.05, 0.9);
            let mut frame = Frame::lines()
                .line_color(line_color)
                .bg_color(bg_color)
                .stroke_width(1.0);
            frame.paint(layout.right, cx);

            // Full Auto toggle at top - tight padding
            let toggle_bounds = Bounds::new(
                layout.right.origin.x + 8.0,
                layout.right.origin.y + 8.0,
                layout.right.size.width - 16.0,
                28.0,
            );
            self.full_auto_toggle.paint(toggle_bounds, cx);

            // System panel content below toggle - tight spacing
            let panel_bounds = Bounds::new(
                layout.right.origin.x,
                layout.right.origin.y + 40.0,
                layout.right.size.width,
                layout.right.size.height - 48.0,
            );
            self.system_panel.paint(panel_bounds, cx);
        }

        // 6. Paint center thread view
        let thread_padding = 24.0;
        let right_padding = 40.0; // extra right padding to prevent overflow
        let thread_bounds = Bounds::new(
            layout.center.origin.x + thread_padding,
            layout.center.origin.y + thread_padding,
            layout.center.size.width - thread_padding - right_padding,
            layout.center.size.height - thread_padding * 2.0,
        );
        self.thread_view.paint(thread_bounds, cx);

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

        // Route to Full Auto toggle first (it's painted over right sidebar)
        if self.right_dock.is_open() {
            let toggle_bounds = Bounds::new(
                layout.right.origin.x + 8.0,
                layout.right.origin.y + 8.0,
                layout.right.size.width - 16.0,
                28.0,
            );
            if let EventResult::Handled = self.full_auto_toggle.event(event, toggle_bounds, cx) {
                return EventResult::Handled;
            }
        }

        // Route to docks
        if self.left_dock.is_open() {
            if let EventResult::Handled = self.left_dock.event(event, layout.left, cx) {
                return EventResult::Handled;
            }
        }

        if self.right_dock.is_open() {
            // Route to system panel content area
            let panel_bounds = Bounds::new(
                layout.right.origin.x + 8.0,
                layout.right.origin.y + 60.0,
                layout.right.size.width - 16.0,
                layout.right.size.height - 68.0,
            );
            if let EventResult::Handled = self.system_panel.event(event, panel_bounds, cx) {
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
