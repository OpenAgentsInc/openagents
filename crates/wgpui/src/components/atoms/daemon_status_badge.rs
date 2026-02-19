//! Daemon status badge for autopilot daemon monitoring.
//!
//! Shows the health status of the autopilot daemon process.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Daemon status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DaemonStatus {
    #[default]
    Offline,
    Starting,
    Online,
    Restarting,
    Error,
    Stopping,
}

impl DaemonStatus {
    pub fn label(&self) -> &'static str {
        match self {
            DaemonStatus::Offline => "Offline",
            DaemonStatus::Starting => "Starting",
            DaemonStatus::Online => "Online",
            DaemonStatus::Restarting => "Restarting",
            DaemonStatus::Error => "Error",
            DaemonStatus::Stopping => "Stopping",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            DaemonStatus::Offline => "◯",
            DaemonStatus::Starting => "◐",
            DaemonStatus::Online => "●",
            DaemonStatus::Restarting => "↻",
            DaemonStatus::Error => "✕",
            DaemonStatus::Stopping => "◔",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            DaemonStatus::Offline => Hsla::new(0.0, 0.0, 0.4, 1.0), // Dark gray
            DaemonStatus::Starting => Hsla::new(200.0, 0.7, 0.55, 1.0), // Blue
            DaemonStatus::Online => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            DaemonStatus::Restarting => Hsla::new(45.0, 0.7, 0.5, 1.0), // Gold
            DaemonStatus::Error => Hsla::new(0.0, 0.8, 0.5, 1.0),   // Red
            DaemonStatus::Stopping => Hsla::new(30.0, 0.7, 0.5, 1.0), // Orange
        }
    }

    pub fn is_operational(&self) -> bool {
        matches!(
            self,
            DaemonStatus::Online | DaemonStatus::Starting | DaemonStatus::Restarting
        )
    }
}

/// Badge showing daemon status
pub struct DaemonStatusBadge {
    id: Option<ComponentId>,
    status: DaemonStatus,
    uptime_secs: Option<u64>,
    worker_count: Option<u32>,
    compact: bool,
}

impl DaemonStatusBadge {
    pub fn new(status: DaemonStatus) -> Self {
        Self {
            id: None,
            status,
            uptime_secs: None,
            worker_count: None,
            compact: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn uptime(mut self, secs: u64) -> Self {
        self.uptime_secs = Some(secs);
        self
    }

    pub fn worker_count(mut self, count: u32) -> Self {
        self.worker_count = Some(count);
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

/// Format uptime as human readable
fn format_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("{}d {}h", days, hours)
    } else if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

impl Component for DaemonStatusBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.status.color();
        let bg = Hsla::new(color.h, color.s * 0.2, 0.12, 0.95);

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        let padding = 6.0;
        let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;
        let mut x = bounds.origin.x + padding;

        // Icon
        let icon = self.status.icon();
        let icon_run = cx
            .text
            .layout(icon, Point::new(x, text_y), theme::font_size::SM, color);
        cx.scene.draw_text(icon_run);

        if !self.compact {
            x += 14.0;
            // Label
            let label = self.status.label();
            let label_run =
                cx.text
                    .layout(label, Point::new(x, text_y), theme::font_size::XS, color);
            cx.scene.draw_text(label_run);
            x += label.len() as f32 * 6.5 + 8.0;

            // Uptime
            if let Some(secs) = self.uptime_secs {
                let uptime = format_uptime(secs);
                let uptime_run = cx.text.layout_mono(
                    &uptime,
                    Point::new(x, text_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(uptime_run);
                x += uptime.len() as f32 * 6.5 + 6.0;
            }

            // Worker count
            if let Some(count) = self.worker_count {
                let workers_text = format!("{} workers", count);
                let workers_run = cx.text.layout_mono(
                    &workers_text,
                    Point::new(x, text_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(workers_run);
            }
        }
    }

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        if self.compact {
            (Some(28.0), Some(22.0))
        } else {
            let mut width = 12.0 + 14.0 + self.status.label().len() as f32 * 6.5;
            if self.uptime_secs.is_some() {
                width += 50.0;
            }
            if self.worker_count.is_some() {
                width += 70.0;
            }
            (Some(width), Some(22.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_status() {
        assert_eq!(DaemonStatus::Online.label(), "Online");
        assert!(DaemonStatus::Online.is_operational());
        assert!(!DaemonStatus::Offline.is_operational());
    }

    #[test]
    fn test_format_uptime() {
        assert_eq!(format_uptime(120), "2m");
        assert_eq!(format_uptime(3700), "1h 1m");
        assert_eq!(format_uptime(90000), "1d 1h");
    }
}
