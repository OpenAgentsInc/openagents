//! Alert/notification component.

use crate::animator::HudAnimator;
use crate::easing::ease_out_expo;
use crate::theme::hud as colors;
use wgpui::{Bounds, Hsla, Point, Scene, Size, TextSystem};

/// Alert severity level.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum AlertLevel {
    /// Informational message.
    #[default]
    Info,
    /// Success message.
    Success,
    /// Warning message.
    Warning,
    /// Error message.
    Error,
}

impl AlertLevel {
    /// Get the accent color for this level.
    pub fn color(&self) -> Hsla {
        match self {
            AlertLevel::Info => Hsla::new(0.6, 0.5, 0.6, 0.8), // Cyan-ish
            AlertLevel::Success => Hsla::new(0.35, 0.6, 0.5, 0.8), // Green
            AlertLevel::Warning => Hsla::new(0.12, 0.7, 0.5, 0.8), // Orange
            AlertLevel::Error => Hsla::new(0.0, 0.7, 0.5, 0.8), // Red
        }
    }

    /// Get the icon for this level.
    pub fn icon(&self) -> &'static str {
        match self {
            AlertLevel::Info => "i",
            AlertLevel::Success => "+",
            AlertLevel::Warning => "!",
            AlertLevel::Error => "x",
        }
    }
}

/// Animated alert/notification component.
pub struct Alert {
    message: String,
    level: AlertLevel,
    animator: HudAnimator,
    title: Option<String>,

    // Styling
    text_color: Hsla,
    padding: f32,
    icon_size: f32,
    border_width: f32,
}

impl Alert {
    /// Create a new alert with a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: AlertLevel::Info,
            animator: HudAnimator::new().enter_duration(15),
            title: None,
            text_color: colors::TEXT,
            padding: 16.0,
            icon_size: 20.0,
            border_width: 1.0,
        }
    }

    /// Set alert level.
    pub fn level(mut self, level: AlertLevel) -> Self {
        self.level = level;
        self
    }

    /// Set title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set text color.
    pub fn text_color(mut self, color: Hsla) -> Self {
        self.text_color = color;
        self
    }

    /// Start enter animation.
    pub fn enter(&mut self) {
        self.animator.enter();
    }

    /// Start exit animation.
    pub fn exit(&mut self) {
        self.animator.exit();
    }

    /// Update animation state.
    pub fn tick(&mut self) {
        self.animator.tick();
    }

    /// Check if alert is visible.
    pub fn is_visible(&self) -> bool {
        self.animator.progress() > 0.0
    }

    /// Paint the alert.
    pub fn paint(&self, bounds: Bounds, scene: &mut Scene, text_system: &mut TextSystem) {
        let progress = ease_out_expo(self.animator.progress());
        if progress <= 0.0 {
            return;
        }

        let accent_color = self.level.color();
        let accent_alpha = accent_color.a * progress;

        // Draw background
        scene.draw_quad(
            wgpui::Quad::new(bounds)
                .with_background(Hsla::new(0.0, 0.0, 0.0, 0.8 * progress))
                .with_border(
                    Hsla::new(accent_color.h, accent_color.s, accent_color.l, accent_alpha),
                    self.border_width,
                ),
        );

        // Draw left accent bar
        let accent_bar = Bounds::from_origin_size(
            Point::new(bounds.x(), bounds.y()),
            Size::new(3.0, bounds.height() * progress),
        );
        scene.draw_quad(wgpui::Quad::new(accent_bar).with_background(Hsla::new(
            accent_color.h,
            accent_color.s,
            accent_color.l,
            accent_alpha,
        )));

        // Draw icon
        let icon_x = bounds.x() + self.padding;
        let icon_y = bounds.y() + self.padding + self.icon_size / 2.0;

        let icon_run = text_system.layout(
            self.level.icon(),
            Point::new(icon_x, icon_y + 4.0),
            self.icon_size,
            Hsla::new(accent_color.h, accent_color.s, accent_color.l, accent_alpha),
        );
        scene.draw_text(icon_run);

        let text_x = icon_x + self.icon_size + self.padding;
        let mut text_y = bounds.y() + self.padding;

        // Draw title if present
        if let Some(title) = &self.title {
            let title_color = Hsla::new(
                self.text_color.h,
                self.text_color.s,
                self.text_color.l,
                self.text_color.a * progress,
            );
            let title_run =
                text_system.layout(title, Point::new(text_x, text_y + 12.0), 14.0, title_color);
            scene.draw_text(title_run);
            text_y += 20.0;
        }

        // Draw message
        let message_color = Hsla::new(
            self.text_color.h,
            self.text_color.s,
            self.text_color.l,
            self.text_color.a * progress * 0.8,
        );
        let message_run = text_system.layout(
            &self.message,
            Point::new(text_x, text_y + 12.0),
            12.0,
            message_color,
        );
        scene.draw_text(message_run);
    }
}
