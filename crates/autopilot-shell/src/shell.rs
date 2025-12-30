//! Main AutopilotShell component

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Receiver, TryRecvError};

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
use crate::panels::{SessionAction, SessionInfo, SessionsPanel, SessionUsage, SystemPanel, UsageLimit};
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

    // Left sidebar panel (stored directly for action handling)
    sessions_panel: SessionsPanel,

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

    // Active Task tools for nesting child tools: Vec of (tool_key, entry_index)
    // Multiple Tasks can run in parallel, we track all of them
    active_tasks: Vec<(String, usize)>,

    // External children storage for Task tools (works around type erasure)
    // Maps parent entry_idx -> accumulated child tools
    task_children: HashMap<usize, Vec<ChildTool>>,

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

    // Session resume state (for Claude Code session continuation)
    resumed_session_id: Option<String>,

    // Channel for receiving messages from resumed SDK session
    sdk_message_rx: Option<Receiver<SdkMessageEvent>>,
}

/// Events from the Claude SDK session
#[derive(Debug)]
enum SdkMessageEvent {
    /// Text from assistant
    AssistantText(String),
    /// Tool use started
    ToolUse { name: String },
    /// Session completed
    Completed,
    /// Error occurred
    Error(String),
}

impl AutopilotShell {
    pub fn new() -> Self {
        // Create left dock (empty - we render sessions panel directly for action handling)
        let left_dock = Dock::new(DockPosition::Left, 280.0);

        // Create sessions panel directly (like system_panel)
        let mut sessions_panel = SessionsPanel::new();
        // Load recent sessions from ~/.claude/projects/
        let claude_sessions = crate::claude_sessions::list_claude_sessions();
        let sessions: Vec<SessionInfo> = claude_sessions
            .into_iter()
            .take(5)
            .map(|s| SessionInfo {
                id: s.session_id,
                timestamp: s.timestamp,
                model: "sonnet".to_string(), // Claude sessions don't store model
                is_current: false,
            })
            .collect();
        sessions_panel.set_sessions(sessions);

        // Create right dock (empty - we render system panel directly for data access)
        let right_dock = Dock::new(DockPosition::Right, 300.0);

        // Bottom dock (empty for now)
        let bottom_dock = Dock::new(DockPosition::Bottom, 200.0);

        // Thread view for center
        let mut thread_view = ThreadView::new().auto_scroll(true);
        thread_view.push_entry(
            ThreadEntry::new(ThreadEntryType::System, Text::new("Autopilot ready."))
                .copyable_text("Autopilot ready."),
        );

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
            sessions_panel,
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
            active_tasks: Vec::new(),
            task_children: HashMap::new(),
            full_auto_toggle: FullAutoToggle::new(),
            keymap,
            key_context: KeyContext::new(),
            pending_fullscreen_toggle: false,
            rate_limit_fetcher,
            rate_limits,
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            resumed_session_id: None,
            sdk_message_rx: None,
        }
    }

    /// Create a shell from a saved checkpoint.
    pub fn with_checkpoint(cp: SessionCheckpoint) -> Self {
        let working_dir = cp.working_dir.clone();
        let current_session_id = cp.session_id.clone();

        // Create left dock (empty - we render sessions panel directly)
        let left_dock = Dock::new(DockPosition::Left, 280.0);

        // Create sessions panel directly - load from ~/.claude/projects/
        let mut sessions_panel = SessionsPanel::new();
        let claude_sessions = crate::claude_sessions::list_claude_sessions();
        let sessions: Vec<SessionInfo> = claude_sessions
            .into_iter()
            .take(5)
            .map(|s| SessionInfo {
                id: s.session_id.clone(),
                timestamp: s.timestamp,
                model: "sonnet".to_string(),
                is_current: s.session_id == current_session_id,
            })
            .collect();
        sessions_panel.set_sessions(sessions);

        // Create right dock (empty - we render system panel directly for data access)
        let right_dock = Dock::new(DockPosition::Right, 300.0);

        // Bottom dock (empty for now)
        let bottom_dock = Dock::new(DockPosition::Bottom, 200.0);

        // Thread view for center - will be populated from checkpoint state
        let mut thread_view = ThreadView::new().auto_scroll(true);
        thread_view.push_entry(
            ThreadEntry::new(ThreadEntryType::System, Text::new("Session resumed."))
                .copyable_text("Session resumed."),
        );

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
            sessions_panel,
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
            active_tasks: Vec::new(),
            task_children: HashMap::new(),
            full_auto_toggle: FullAutoToggle::new(),
            keymap,
            key_context: KeyContext::new(),
            pending_fullscreen_toggle: false,
            rate_limit_fetcher,
            rate_limits,
            working_dir,
            resumed_session_id: None,
            sdk_message_rx: None,
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
        // Note: context_window from SDK is the model's capacity, NOT used context
        // Use input_tokens as a rough proxy for context used in this session
        let context_used = snapshot.session_usage.input_tokens;
        let context_total = snapshot.session_usage.context_window.max(200_000);
        self.system_panel.update_usage(model_str, context_used, context_total);

        // Update session stats from accumulated usage data
        let session = SessionUsage {
            input_tokens: snapshot.session_usage.input_tokens,
            output_tokens: snapshot.session_usage.output_tokens,
            cache_read_tokens: snapshot.session_usage.cache_read_tokens,
            cache_creation_tokens: snapshot.session_usage.cache_creation_tokens,
            total_cost_usd: snapshot.session_usage.total_cost_usd,
            duration_ms: snapshot.session_usage.duration_ms,
            duration_api_ms: snapshot.session_usage.duration_api_ms,
            num_turns: snapshot.session_usage.num_turns,
        };
        self.system_panel.update_session(session);

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
        self.thread_view.push_entry(
            ThreadEntry::new(ThreadEntryType::System, Text::new("Autopilot ready."))
                .copyable_text("Autopilot ready."),
        );

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
                .details(details.clone());

            self.thread_view.push_entry(
                ThreadEntry::new(ThreadEntryType::System, collapsible)
                    .copyable_text(details.join("\n")),
            );
        }

        // Add Claude (non-section) lines
        for line in lines {
            if line.section == Some(StartupSection::Claude) && !line.text.trim().is_empty() {
                self.thread_view.push_entry(
                    ThreadEntry::new(ThreadEntryType::System, Text::new(line.text.clone()))
                        .copyable_text(line.text.clone()),
                );
            }
        }
    }

    /// Add new Claude lines incrementally (for streaming updates).
    fn add_claude_lines(&mut self, lines: &[LogLine]) {
        if lines.len() > self.last_line_count {
            for line in lines.iter().skip(self.last_line_count) {
                // Only add Claude section lines (startup sections are in collapsible)
                if line.section == Some(StartupSection::Claude) && !line.text.trim().is_empty() {
                    self.thread_view.push_entry(
                        ThreadEntry::new(ThreadEntryType::System, Text::new(line.text.clone()))
                            .copyable_text(line.text.clone()),
                    );
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

    /// Rebuild a Task entry with all its accumulated children.
    /// This works around type erasure by creating a new ToolCallCard with children.
    fn rebuild_task_entry(&mut self, entry_idx: usize) {
        // Find the parent tool info from pending_tools (reverse lookup)
        let parent_info = self
            .pending_tools
            .iter()
            .find(|&(_, idx)| *idx == entry_idx)
            .map(|(key, _)| key.clone());

        if let Some(key) = parent_info {
            // Parse key to get phase::name::params_hash
            let parts: Vec<&str> = key.split("::").collect();
            if parts.len() >= 2 {
                let display_name = format!("{}::{}", parts[0], parts[1]);

                // Build new card with all children
                let mut card = ToolCallCard::new(ToolType::Task, display_name)
                    .status(ToolStatus::Running);

                // Add all accumulated children
                if let Some(children) = self.task_children.get(&entry_idx) {
                    for child in children {
                        card.add_child(child.clone());
                    }
                }

                // Replace entry content
                if let Some(entry) = self.thread_view.entry_mut(entry_idx) {
                    entry.set_content(card);
                }
            }
        }
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
                output,
                is_error,
            } => {
                let tool_type = tool_type_from_name(name);
                // Use phase::name + truncated params as unique key (multiple tools can have same name)
                let params_hash = params.chars().take(60).collect::<String>();
                let tool_key = format!("{}::{}::{}", phase_label(*phase), name, params_hash);
                let display_name = format!("{}::{}", phase_label(*phase), name);
                let is_task_tool = name == "Task";

                if *done {
                    // Tool completed - determine status from error flag
                    let status = if *is_error {
                        ToolStatus::Error
                    } else {
                        ToolStatus::Success
                    };

                    if is_task_tool {
                        // Task completed - update entry and clean up children
                        if let Some(entry_idx) = self.pending_tools.remove(&tool_key) {
                            // Build final card with all children marked complete
                            let mut card = ToolCallCard::new(tool_type, display_name.clone())
                                .status(status)
                                .input(params.clone());

                            // Add output if available
                            if let Some(output_text) = output {
                                card = card.output(output_text.clone());
                            }

                            // Add all children (they should already be marked complete)
                            if let Some(children) = self.task_children.get(&entry_idx) {
                                for child in children {
                                    card.add_child(child.clone());
                                }
                            }

                            if let Some(entry) = self.thread_view.entry_mut(entry_idx) {
                                entry.set_content(card);
                            }

                            // Clean up children tracking
                            self.task_children.remove(&entry_idx);
                        }

                        // Remove this Task from active_tasks
                        self.active_tasks.retain(|(key, _)| key != &tool_key);
                    } else {
                        // Non-Task tool completed - find which parent Task owns it
                        // Match by both name AND params prefix to handle multiple Tasks
                        // with children of the same tool type
                        let params_prefix: String = params.chars().take(50).collect();
                        let mut found_parent = None;
                        for (_, parent_idx) in &self.active_tasks {
                            if let Some(children) = self.task_children.get(parent_idx) {
                                // Try exact params match first, then name-only match
                                if children.iter().any(|c| {
                                    c.name == *name
                                        && c.status == ToolStatus::Running
                                        && c.params.starts_with(&params_prefix)
                                }) {
                                    found_parent = Some(*parent_idx);
                                    break;
                                }
                            }
                        }
                        // Fallback: if no exact match, try name-only match
                        if found_parent.is_none() {
                            for (_, parent_idx) in &self.active_tasks {
                                if let Some(children) = self.task_children.get(parent_idx) {
                                    if children.iter().any(|c| c.name == *name && c.status == ToolStatus::Running) {
                                        found_parent = Some(*parent_idx);
                                        break;
                                    }
                                }
                            }
                        }

                        if let Some(parent_idx) = found_parent {
                            // Update child status in the parent that owns it
                            if let Some(children) = self.task_children.get_mut(&parent_idx) {
                                for child in children.iter_mut().rev() {
                                    // Match by name and params prefix
                                    if child.name == *name
                                        && child.status == ToolStatus::Running
                                        && child.params.starts_with(&params_prefix)
                                    {
                                        child.status = status;
                                        break;
                                    }
                                }
                                // Fallback: if no exact match found, try name-only
                                let any_updated = children.iter().any(|c| c.name == *name && c.status != ToolStatus::Running);
                                if !any_updated {
                                    for child in children.iter_mut().rev() {
                                        if child.name == *name && child.status == ToolStatus::Running {
                                            child.status = status;
                                            break;
                                        }
                                    }
                                }
                            }
                            // Rebuild parent to show updated child status
                            self.rebuild_task_entry(parent_idx);
                        } else if self.active_tasks.is_empty() {
                            // Standalone tool (no parent Task) - update directly
                            if let Some(entry_idx) = self.pending_tools.remove(&tool_key) {
                                if let Some(entry) = self.thread_view.entry_mut(entry_idx) {
                                    let mut card = ToolCallCard::new(tool_type, display_name)
                                        .status(status)
                                        .input(params.clone());

                                    // Add output if available
                                    if let Some(output_text) = output {
                                        card = card.output(output_text.clone());
                                    }

                                    entry.set_content(card);
                                }
                            }
                        }
                    }
                } else {
                    // Tool starting
                    if is_task_tool {
                        // Task tool - add to main thread and push to active_tasks
                        let entry_idx = self.thread_view.entry_count();
                        self.pending_tools.insert(tool_key.clone(), entry_idx);
                        self.active_tasks.push((tool_key.clone(), entry_idx));

                        let card = ToolCallCard::new(tool_type, display_name)
                            .status(ToolStatus::Running)
                            .input(params.clone());
                        self.thread_view
                            .push_entry(ThreadEntry::new(ThreadEntryType::Tool, card));
                    } else if !self.active_tasks.is_empty() {
                        // Non-Task tool with active parent(s) - add as nested child
                        // Find the active Task with fewest children (distribute evenly)
                        let parent_idx = self.active_tasks.iter()
                            .map(|(_, idx)| *idx)
                            .min_by_key(|idx| self.task_children.get(idx).map(|c| c.len()).unwrap_or(0))
                            .unwrap(); // Safe: we checked active_tasks is not empty

                        let child = ChildTool {
                            tool_type,
                            name: name.clone(),
                            params: params.clone(),
                            status: ToolStatus::Running,
                        };

                        self.task_children
                            .entry(parent_idx)
                            .or_default()
                            .push(child);

                        // Rebuild parent entry with new child
                        self.rebuild_task_entry(parent_idx);
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

                if self.full_auto_toggle.is_enabled() {
                    // Full Auto turned ON - check if we have a session to resume
                    if let Some(session_id) = self.resumed_session_id.take() {
                        self.start_session_resume(session_id);
                    }
                } else {
                    // Full Auto turned OFF - save checkpoint
                    if was_enabled {
                        if let Err(e) = self.save_checkpoint() {
                            info!("Failed to save checkpoint: {}", e);
                        } else {
                            info!("Checkpoint saved for session {}", self.session_id());
                        }
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

    /// Handle actions from the sessions panel
    fn handle_session_action(&mut self, action: SessionAction) {
        match action {
            SessionAction::ResumeSession(session_id) => {
                // Load Claude Code session from ~/.claude/projects/
                if let Some(file_path) = crate::claude_sessions::find_session_file(&session_id) {
                    let messages = crate::claude_sessions::load_session_messages(&file_path);

                    // Clear current thread view
                    self.thread_view.clear();
                    self.pending_tools.clear();
                    self.active_tasks.clear();
                    self.task_children.clear();

                    // Add header
                    let header = format!("Loaded session: {}", &session_id[..8.min(session_id.len())]);
                    self.thread_view.push_entry(
                        ThreadEntry::new(ThreadEntryType::System, Text::new(&header))
                            .copyable_text(&session_id),
                    );

                    // Group child tools by parent Task ID for nesting
                    let mut task_children_map: HashMap<String, Vec<&crate::claude_sessions::SessionMessage>> = HashMap::new();
                    for msg in &messages {
                        if let Some(parent_id) = &msg.parent_task_id {
                            task_children_map.entry(parent_id.clone()).or_default().push(msg);
                        }
                    }

                    // Add all messages from the session
                    for msg in &messages {
                        // Skip child tools - they'll be nested under their parent Task
                        if msg.parent_task_id.is_some() {
                            continue;
                        }

                        if msg.is_tool_use {
                            // Create ToolCallCard for tool messages
                            let tool_name = msg.tool_name.as_deref().unwrap_or("unknown");
                            let tool_type = tool_type_from_name(tool_name);

                            // Determine status from result
                            let status = match (&msg.tool_output, msg.is_error) {
                                (Some(_), Some(true)) => ToolStatus::Error,
                                (Some(_), _) => ToolStatus::Success,
                                (None, _) => ToolStatus::Pending,
                            };

                            let mut card = ToolCallCard::new(tool_type, tool_name)
                                .status(status);

                            // Add input params
                            if let Some(input) = &msg.tool_input {
                                card = card.input(input.clone());
                            }

                            // Add output
                            if let Some(output) = &msg.tool_output {
                                card = card.output(output.clone());
                            }

                            // Add child tools for Task
                            if tool_name == "Task" {
                                if let Some(tool_id) = &msg.tool_id {
                                    if let Some(children) = task_children_map.get(tool_id) {
                                        for child_msg in children {
                                            let child_name = child_msg.tool_name.as_deref().unwrap_or("unknown");
                                            let child_type = tool_type_from_name(child_name);
                                            let child_status = match child_msg.is_error {
                                                Some(true) => ToolStatus::Error,
                                                Some(false) => ToolStatus::Success,
                                                None => if child_msg.tool_output.is_some() {
                                                    ToolStatus::Success
                                                } else {
                                                    ToolStatus::Pending
                                                },
                                            };

                                            card.add_child(ChildTool {
                                                tool_type: child_type,
                                                name: child_name.to_string(),
                                                params: child_msg.tool_input.clone().unwrap_or_default(),
                                                status: child_status,
                                            });
                                        }
                                    }
                                }
                            }

                            self.thread_view.push_entry(
                                ThreadEntry::new(ThreadEntryType::Tool, card),
                            );
                        } else {
                            // Text message
                            let entry_type = if msg.role == "user" {
                                ThreadEntryType::User
                            } else {
                                ThreadEntryType::Assistant
                            };

                            // Truncate very long messages for display
                            let display_content = if msg.content.len() > 500 {
                                format!("{}...", &msg.content[..500])
                            } else {
                                msg.content.clone()
                            };

                            self.thread_view.push_entry(
                                ThreadEntry::new(entry_type, Text::new(&display_content))
                                    .copyable_text(&msg.content),
                            );
                        }
                    }

                    // Store session ID for resume when Full Auto is toggled
                    self.resumed_session_id = Some(session_id.clone());

                    // Show ready to resume message
                    self.thread_view.push_entry(
                        ThreadEntry::new(ThreadEntryType::System,
                            Text::new("Session loaded. Toggle Full Auto (cmd-a) to continue."))
                            .copyable_text("Toggle Full Auto to continue"),
                    );

                    // Mark as current in the list
                    self.refresh_sessions_list(&session_id);
                } else {
                    let err_msg = format!("Session not found: {}", session_id);
                    self.thread_view.push_entry(
                        ThreadEntry::new(ThreadEntryType::Error, Text::new(&err_msg))
                            .copyable_text(&err_msg),
                    );
                }
            }
            SessionAction::SetModel(model) => {
                info!("Setting model to: {:?}", model);

                // Convert wgpui Model to ClaudeModel
                use wgpui::components::atoms::Model;
                let claude_model = match model {
                    Model::ClaudeSonnet => ClaudeModel::Sonnet,
                    Model::ClaudeOpus => ClaudeModel::Opus,
                    _ => ClaudeModel::Sonnet, // Fallback for other models
                };

                self.runtime.set_model(claude_model);
            }
            SessionAction::Interrupt => {
                info!("Interrupting current query");

                self.runtime.interrupt();
                self.sessions_panel.set_running(false);

                self.thread_view.push_entry(
                    ThreadEntry::new(ThreadEntryType::System, Text::new("Interrupted."))
                        .copyable_text("Interrupted."),
                );
            }
            SessionAction::NewSession => {
                info!("Creating new session");

                // Clear UI state
                self.thread_view.clear();
                self.pending_tools.clear();
                self.active_tasks.clear();
                self.task_children.clear();

                // Reset runtime
                self.runtime.reset(ClaudeModel::Sonnet);

                // Add welcome message
                self.thread_view.push_entry(
                    ThreadEntry::new(ThreadEntryType::System, Text::new("New session started."))
                        .copyable_text("New session started."),
                );

                // Refresh sessions list (no current session)
                self.refresh_sessions_list("");
            }
        }
    }

    /// Start resuming a Claude Code session via the SDK
    fn start_session_resume(&mut self, session_id: String) {
        use claude_agent_sdk::{QueryOptions, SdkMessage};
        use futures::StreamExt;

        info!("Starting session resume for: {}", session_id);

        // Create channel for SDK messages
        let (tx, rx) = mpsc::channel::<SdkMessageEvent>();
        self.sdk_message_rx = Some(rx);

        // Get model from runtime
        let model = self.runtime.model().as_str().to_string();

        // Spawn async task to resume session
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    let _ = tx.send(SdkMessageEvent::Error(format!("Failed to create runtime: {}", e)));
                    return;
                }
            };

            rt.block_on(async move {
                // Build options for resume
                let options = QueryOptions::new()
                    .resume(session_id.clone())
                    .model(&model)
                    .max_turns(100);

                // Resume the session
                let mut session = match claude_agent_sdk::unstable_v2_resume_session(session_id, options).await {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = tx.send(SdkMessageEvent::Error(format!("Failed to resume session: {}", e)));
                        return;
                    }
                };

                // Send empty message to continue (SDK will pick up from history)
                if let Err(e) = session.send("").await {
                    let _ = tx.send(SdkMessageEvent::Error(format!("Failed to send: {}", e)));
                    return;
                }

                // Stream messages from session
                while let Some(msg_result) = session.receive().next().await {
                    match msg_result {
                        Ok(msg) => {
                            match msg {
                                SdkMessage::Assistant(a) => {
                                    // Extract text content from assistant message (message is serde_json::Value)
                                    if let Some(content) = a.message.get("content").and_then(|c| c.as_array()) {
                                        for block in content {
                                            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                                let _ = tx.send(SdkMessageEvent::AssistantText(text.to_string()));
                                            }
                                            if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                                                let _ = tx.send(SdkMessageEvent::ToolUse { name: name.to_string() });
                                            }
                                        }
                                    }
                                }
                                SdkMessage::Result(_) => {
                                    let _ = tx.send(SdkMessageEvent::Completed);
                                    break;
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(SdkMessageEvent::Error(format!("Stream error: {}", e)));
                            break;
                        }
                    }
                }
            });
        });

        // Add status message
        self.thread_view.push_entry(
            ThreadEntry::new(ThreadEntryType::System, Text::new("Resuming session..."))
                .copyable_text("Resuming session..."),
        );
    }

    /// Poll for SDK messages and update thread view
    fn poll_sdk_messages(&mut self) {
        if let Some(ref rx) = self.sdk_message_rx {
            loop {
                match rx.try_recv() {
                    Ok(event) => {
                        match event {
                            SdkMessageEvent::AssistantText(text) => {
                                self.thread_view.push_entry(
                                    ThreadEntry::new(ThreadEntryType::Assistant, Text::new(&text))
                                        .copyable_text(&text),
                                );
                            }
                            SdkMessageEvent::ToolUse { name } => {
                                let msg = format!("Tool: {}", name);
                                self.thread_view.push_entry(
                                    ThreadEntry::new(ThreadEntryType::Tool, Text::new(&msg))
                                        .copyable_text(&msg),
                                );
                            }
                            SdkMessageEvent::Completed => {
                                self.thread_view.push_entry(
                                    ThreadEntry::new(ThreadEntryType::System, Text::new("Session completed."))
                                        .copyable_text("Session completed."),
                                );
                                self.sdk_message_rx = None;
                                self.full_auto_toggle.set_enabled(false);
                                break;
                            }
                            SdkMessageEvent::Error(e) => {
                                self.thread_view.push_entry(
                                    ThreadEntry::new(ThreadEntryType::Error, Text::new(&e))
                                        .copyable_text(&e),
                                );
                                self.sdk_message_rx = None;
                                self.full_auto_toggle.set_enabled(false);
                                break;
                            }
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.sdk_message_rx = None;
                        break;
                    }
                }
            }
        }
    }

    /// Refresh the sessions list in the sidebar
    fn refresh_sessions_list(&mut self, current_id: &str) {
        let claude_sessions = crate::claude_sessions::list_claude_sessions();
        let sessions: Vec<SessionInfo> = claude_sessions
            .into_iter()
            .take(5)
            .map(|s| SessionInfo {
                id: s.session_id.clone(),
                timestamp: s.timestamp,
                model: "sonnet".to_string(),
                is_current: s.session_id == current_id,
            })
            .collect();
        self.sessions_panel.set_sessions(sessions);
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

        // 3c. Poll for SDK session messages (from resumed sessions)
        self.poll_sdk_messages();

        // 4. Calculate layout
        let layout = self.calculate_layout(bounds);

        // 5. Paint left sidebar with sessions panel
        if self.left_dock.is_open() {
            // Draw line frame for left sidebar
            let line_color = Hsla::new(0.0, 0.0, 0.3, 0.5);
            let bg_color = Hsla::new(0.0, 0.0, 0.05, 0.9);
            let mut frame = Frame::lines()
                .line_color(line_color)
                .bg_color(bg_color)
                .stroke_width(1.0);
            frame.paint(layout.left, cx);

            // Sessions panel content
            let panel_bounds = Bounds::new(
                layout.left.origin.x + 8.0,
                layout.left.origin.y + 8.0,
                layout.left.size.width - 16.0,
                layout.left.size.height - 16.0,
            );
            self.sessions_panel.paint(panel_bounds, cx);
        }

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
                return EventResult::Ignored;
            }
        }
        // Clear startup if complete
        if self.startup.as_ref().is_some_and(|s| s.is_complete()) {
            self.startup = None;
        }

        // Handle keyboard via keymap
        if let InputEvent::KeyDown { key, modifiers } = event {
            if let Some(action) = self.keymap.match_keystroke(key, modifiers, &self.key_context) {
                let result = self.handle_action(action.name());
                if result.is_handled() {
                    return result;
                }
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

        // Route to left sidebar sessions panel
        if self.left_dock.is_open() {
            let panel_bounds = Bounds::new(
                layout.left.origin.x + 8.0,
                layout.left.origin.y + 8.0,
                layout.left.size.width - 16.0,
                layout.left.size.height - 16.0,
            );
            if let EventResult::Handled = self.sessions_panel.event(event, panel_bounds, cx) {
                // Process any pending actions from the panel
                for action in self.sessions_panel.take_actions() {
                    self.handle_session_action(action);
                }
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
