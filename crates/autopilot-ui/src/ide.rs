use autopilot_service::{DaemonStatus, RuntimeSnapshot, SessionEvent, SessionPhase};
use wgpui::components::atoms::{DaemonStatus as UiDaemonStatus, ToolStatus, ToolType};
use wgpui::components::hud::{StatusBar, StatusItem, StatusItemContent};
use wgpui::components::molecules::{PermissionBar, SessionCard, SessionInfo, SessionSearchBar};
use wgpui::components::organisms::{ThreadEntry, ThreadEntryType, ToolCallCard};
use wgpui::components::sections::ThreadView;
use wgpui::components::{Component, EventContext, EventResult, Text};
use wgpui::{Bounds, InputEvent, PaintContext, Quad, theme};

pub struct AutopilotIde {
    session_search: SessionSearchBar,
    session_card: SessionCard,
    thread: ThreadView,
    status: StatusBar,
    last_line_count: usize,
    daemon_status: Option<DaemonStatus>,
    pending_permissions: Vec<String>,
}

impl AutopilotIde {
    pub fn new() -> Self {
        let session_search = SessionSearchBar::new();
        let session_card = SessionCard::new(
            SessionInfo::new("session", "Autopilot IDE")
                .model("claude-sonnet-4-5")
                .task_count(0),
        );

        let mut thread = ThreadView::new().auto_scroll(true);
        thread.push_entry(ThreadEntry::new(
            ThreadEntryType::System,
            Text::new("Autopilot IDE ready."),
        ));

        let status = StatusBar::new().items(vec![
            StatusItem::text("phase", "Idle").left(),
            StatusItem::text("agent", "Claude").right(),
        ]);

        Self {
            session_search,
            session_card,
            thread,
            status,
            last_line_count: 0,
            daemon_status: None,
            pending_permissions: Vec::new(),
        }
    }

    pub fn apply_snapshot(&mut self, snapshot: &RuntimeSnapshot) {
        self.status.update_item(
            "phase",
            StatusItemContent::Text(format!("{:?}", snapshot.phase)),
        );

        if snapshot.lines.len() <= self.last_line_count {
            return;
        }

        for line in snapshot.lines.iter().skip(self.last_line_count) {
            if line.text.trim().is_empty() {
                continue;
            }
            self.thread.push_entry(ThreadEntry::new(
                ThreadEntryType::System,
                Text::new(line.text.clone()),
            ));
        }

        self.last_line_count = snapshot.lines.len();

        for event in &snapshot.events {
            self.push_event(event);
        }
    }

    pub fn set_daemon_status(&mut self, status: DaemonStatus) {
        self.daemon_status = Some(status);
    }

    pub fn set_pending_permissions(&mut self, pending: Vec<String>) {
        self.pending_permissions = pending;
    }

    fn push_event(&mut self, event: &SessionEvent) {
        match event {
            SessionEvent::Text { phase, content } => {
                let label = format!("[{}] {}", phase_label(*phase), content);
                self.thread.push_entry(ThreadEntry::new(
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
                self.thread
                    .push_entry(ThreadEntry::new(ThreadEntryType::Tool, card));
            }
        }
    }
}

impl Default for AutopilotIde {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for AutopilotIde {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(Quad::new(bounds).with_background(theme::bg::APP));

        let left_w = 280.0;
        let right_w = 300.0;
        let status_h = 28.0;
        let search_h = 46.0;
        let left_padding = 12.0;

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

        cx.scene
            .draw_quad(Quad::new(left).with_background(theme::bg::SURFACE));

        let search_bounds = Bounds::new(
            left.origin.x + left_padding,
            left.origin.y + left_padding,
            left.size.width - left_padding * 2.0,
            search_h,
        );
        self.session_search.paint(search_bounds, cx);

        let card_bounds = Bounds::new(
            left.origin.x + left_padding,
            left.origin.y + left_padding + search_h + theme::spacing::SM,
            left.size.width - left_padding * 2.0,
            120.0,
        );
        self.session_card.paint(card_bounds, cx);
        self.thread.paint(center, cx);
        self.paint_right_panel(right, cx);

        self.status.paint(status, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let left_w = 280.0;
        let right_w = 300.0;
        let status_h = 28.0;
        let search_h = 46.0;
        let left_padding = 12.0;

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

        let search_bounds = Bounds::new(
            left.origin.x + left_padding,
            left.origin.y + left_padding,
            left.size.width - left_padding * 2.0,
            search_h,
        );
        if self.session_search.event(event, search_bounds, cx).is_handled() {
            return EventResult::Handled;
        }

        let card_bounds = Bounds::new(
            left.origin.x + left_padding,
            left.origin.y + left_padding + search_h + theme::spacing::SM,
            left.size.width - left_padding * 2.0,
            120.0,
        );
        if self.session_card.event(event, card_bounds, cx).is_handled() {
            return EventResult::Handled;
        }

        if self.thread.event(event, center, cx).is_handled() {
            return EventResult::Handled;
        }

        self.status.event(event, status, cx)
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

impl AutopilotIde {
    fn paint_right_panel(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene
            .draw_quad(Quad::new(bounds).with_background(theme::bg::SURFACE));

        let padding = 12.0;
        let mut y = bounds.origin.y + padding;

        let mut header = Text::new("System").font_size(theme::font_size::SM);
        header.paint(
            Bounds::new(bounds.origin.x + padding, y, bounds.size.width - padding * 2.0, 20.0),
            cx,
        );
        y += 26.0;

        if let Some(status) = self.daemon_status.clone() {
            let badge_status = map_daemon_status(&status);
            let mut badge =
                wgpui::components::atoms::DaemonStatusBadge::new(badge_status).uptime(status.uptime_seconds);
            badge.paint(
                Bounds::new(bounds.origin.x + padding, y, bounds.size.width - padding * 2.0, 24.0),
                cx,
            );
        } else {
            let mut text = Text::new("Daemon: unknown").font_size(theme::font_size::XS);
            text.paint(
                Bounds::new(bounds.origin.x + padding, y, bounds.size.width - padding * 2.0, 20.0),
                cx,
            );
        }

        y += 36.0;

        let mut perm_header = Text::new("Permissions").font_size(theme::font_size::SM);
        perm_header.paint(
            Bounds::new(bounds.origin.x + padding, y, bounds.size.width - padding * 2.0, 20.0),
            cx,
        );
        y += 26.0;

        if let Some(message) = self.pending_permissions.first() {
            let mut bar = PermissionBar::new(message.clone());
            bar.paint(
                Bounds::new(bounds.origin.x + padding, y, bounds.size.width - padding * 2.0, 40.0),
                cx,
            );
        } else {
            let mut text = Text::new("No pending requests")
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            text.paint(
                Bounds::new(bounds.origin.x + padding, y, bounds.size.width - padding * 2.0, 20.0),
                cx,
            );
        }
    }
}

fn map_daemon_status(status: &DaemonStatus) -> UiDaemonStatus {
    if !status.connected {
        return UiDaemonStatus::Offline;
    }

    match status.worker_status.as_str() {
        "running" => UiDaemonStatus::Online,
        "starting" => UiDaemonStatus::Starting,
        "restarting" => UiDaemonStatus::Restarting,
        "stopping" => UiDaemonStatus::Stopping,
        "error" | "failed" => UiDaemonStatus::Error,
        _ => UiDaemonStatus::Online,
    }
}
