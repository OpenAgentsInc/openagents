//! System panel for the right sidebar

use autopilot_service::DaemonStatus;
use wgpui::{
    Bounds, Component, EventContext, EventResult, InputEvent, PaintContext, theme,
    components::Text,
    components::atoms::{DaemonStatus as UiDaemonStatus, DaemonStatusBadge},
    components::molecules::PermissionBar,
};
use crate::dock::{DockPosition, Panel};

/// Right sidebar panel with daemon status and permissions
pub struct SystemPanel {
    daemon_status: Option<DaemonStatus>,
    pending_permissions: Vec<String>,
}

impl SystemPanel {
    pub fn new() -> Self {
        Self {
            daemon_status: None,
            pending_permissions: Vec::new(),
        }
    }

    pub fn set_daemon_status(&mut self, status: DaemonStatus) {
        self.daemon_status = Some(status);
    }

    pub fn set_pending_permissions(&mut self, pending: Vec<String>) {
        self.pending_permissions = pending;
    }
}

impl Default for SystemPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel for SystemPanel {
    fn panel_id(&self) -> &'static str {
        "system"
    }

    fn title(&self) -> &str {
        "System"
    }

    fn preferred_position(&self) -> DockPosition {
        DockPosition::Right
    }

    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;
        let mut y = bounds.origin.y + padding;

        // Section header: System
        let mut header = Text::new("System").font_size(theme::font_size::SM);
        Component::paint(
            &mut header,
            Bounds::new(bounds.origin.x + padding, y, bounds.size.width - padding * 2.0, 20.0),
            cx,
        );
        y += 26.0;

        // Daemon status
        if let Some(status) = self.daemon_status.clone() {
            let badge_status = map_daemon_status(&status);
            let mut badge = DaemonStatusBadge::new(badge_status).uptime(status.uptime_seconds);
            Component::paint(
                &mut badge,
                Bounds::new(bounds.origin.x + padding, y, bounds.size.width - padding * 2.0, 24.0),
                cx,
            );
        } else {
            let mut text = Text::new("Daemon: unknown").font_size(theme::font_size::XS);
            Component::paint(
                &mut text,
                Bounds::new(bounds.origin.x + padding, y, bounds.size.width - padding * 2.0, 20.0),
                cx,
            );
        }

        y += 36.0;

        // Section header: Permissions
        let mut perm_header = Text::new("Permissions").font_size(theme::font_size::SM);
        Component::paint(
            &mut perm_header,
            Bounds::new(bounds.origin.x + padding, y, bounds.size.width - padding * 2.0, 20.0),
            cx,
        );
        y += 26.0;

        // Pending permissions
        if let Some(message) = self.pending_permissions.first() {
            let mut bar = PermissionBar::new(message.clone());
            Component::paint(
                &mut bar,
                Bounds::new(bounds.origin.x + padding, y, bounds.size.width - padding * 2.0, 40.0),
                cx,
            );
        } else {
            let mut text = Text::new("No pending requests")
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            Component::paint(
                &mut text,
                Bounds::new(bounds.origin.x + padding, y, bounds.size.width - padding * 2.0, 20.0),
                cx,
            );
        }
    }

    fn event(&mut self, _event: &InputEvent, _bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        EventResult::Ignored
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
