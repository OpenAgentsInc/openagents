use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotificationLevel {
    #[default]
    Info,
    Success,
    Warning,
    Error,
}

impl NotificationLevel {
    pub fn color(&self) -> Hsla {
        match self {
            NotificationLevel::Info => theme::accent::PRIMARY,
            NotificationLevel::Success => theme::status::SUCCESS,
            NotificationLevel::Warning => theme::status::WARNING,
            NotificationLevel::Error => theme::status::ERROR,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            NotificationLevel::Info => "\u{2139}",
            NotificationLevel::Success => "\u{2713}",
            NotificationLevel::Warning => "\u{26A0}",
            NotificationLevel::Error => "\u{2717}",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NotificationPosition {
    TopLeft,
    #[default]
    TopRight,
    BottomLeft,
    BottomRight,
    TopCenter,
    BottomCenter,
}

#[derive(Clone)]
pub struct Notification {
    pub id: u64,
    pub title: String,
    pub message: Option<String>,
    pub level: NotificationLevel,
    pub duration: Option<Duration>,
    pub dismissible: bool,
    pub created_at: Instant,
}

impl Notification {
    pub fn new(id: u64, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            message: None,
            level: NotificationLevel::Info,
            duration: Some(Duration::from_secs(5)),
            dismissible: true,
            created_at: Instant::now(),
        }
    }

    pub fn message(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }

    pub fn level(mut self, level: NotificationLevel) -> Self {
        self.level = level;
        self
    }

    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn persistent(mut self) -> Self {
        self.duration = None;
        self
    }

    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.dismissible = dismissible;
        self
    }

    pub fn info(id: u64, title: impl Into<String>) -> Self {
        Self::new(id, title).level(NotificationLevel::Info)
    }

    pub fn success(id: u64, title: impl Into<String>) -> Self {
        Self::new(id, title).level(NotificationLevel::Success)
    }

    pub fn warning(id: u64, title: impl Into<String>) -> Self {
        Self::new(id, title).level(NotificationLevel::Warning)
    }

    pub fn error(id: u64, title: impl Into<String>) -> Self {
        Self::new(id, title).level(NotificationLevel::Error)
    }

    pub fn is_expired(&self) -> bool {
        if let Some(duration) = self.duration {
            self.created_at.elapsed() >= duration
        } else {
            false
        }
    }
}

pub struct Notifications {
    id: Option<ComponentId>,
    notifications: Vec<Notification>,
    position: NotificationPosition,
    max_visible: usize,
    notification_width: f32,
    notification_height: f32,
    spacing: f32,
    next_id: u64,
    hovered_dismiss: Option<u64>,
    on_dismiss: Option<Box<dyn FnMut(u64)>>,
}

impl Notifications {
    pub fn new() -> Self {
        Self {
            id: None,
            notifications: Vec::new(),
            position: NotificationPosition::TopRight,
            max_visible: 5,
            notification_width: 320.0,
            notification_height: 72.0,
            spacing: theme::spacing::SM,
            next_id: 1,
            hovered_dismiss: None,
            on_dismiss: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn position(mut self, position: NotificationPosition) -> Self {
        self.position = position;
        self
    }

    pub fn max_visible(mut self, max: usize) -> Self {
        self.max_visible = max;
        self
    }

    pub fn notification_width(mut self, width: f32) -> Self {
        self.notification_width = width;
        self
    }

    pub fn on_dismiss<F>(mut self, f: F) -> Self
    where
        F: FnMut(u64) + 'static,
    {
        self.on_dismiss = Some(Box::new(f));
        self
    }

    pub fn push(&mut self, notification: Notification) -> u64 {
        let id = notification.id;
        self.notifications.push(notification);
        id
    }

    pub fn notify(&mut self, title: impl Into<String>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.push(Notification::new(id, title))
    }

    pub fn info(&mut self, title: impl Into<String>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.push(Notification::info(id, title))
    }

    pub fn success(&mut self, title: impl Into<String>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.push(Notification::success(id, title))
    }

    pub fn warning(&mut self, title: impl Into<String>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.push(Notification::warning(id, title))
    }

    pub fn error(&mut self, title: impl Into<String>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.push(Notification::error(id, title))
    }

    pub fn dismiss(&mut self, id: u64) {
        self.notifications.retain(|n| n.id != id);
        if let Some(callback) = &mut self.on_dismiss {
            callback(id);
        }
    }

    pub fn clear(&mut self) {
        self.notifications.clear();
    }

    pub fn tick(&mut self) {
        let expired: Vec<u64> = self
            .notifications
            .iter()
            .filter(|n| n.is_expired())
            .map(|n| n.id)
            .collect();

        for id in expired {
            self.dismiss(id);
        }
    }

    pub fn count(&self) -> usize {
        self.notifications.len()
    }

    fn notification_bounds(&self, bounds: &Bounds, index: usize) -> Bounds {
        let padding = theme::spacing::MD;
        let total_height = self.notification_height + self.spacing;

        let x = match self.position {
            NotificationPosition::TopLeft | NotificationPosition::BottomLeft => {
                bounds.origin.x + padding
            }
            NotificationPosition::TopRight | NotificationPosition::BottomRight => {
                bounds.origin.x + bounds.size.width - self.notification_width - padding
            }
            NotificationPosition::TopCenter | NotificationPosition::BottomCenter => {
                bounds.origin.x + (bounds.size.width - self.notification_width) / 2.0
            }
        };

        let y = match self.position {
            NotificationPosition::TopLeft
            | NotificationPosition::TopRight
            | NotificationPosition::TopCenter => {
                bounds.origin.y + padding + index as f32 * total_height
            }
            NotificationPosition::BottomLeft
            | NotificationPosition::BottomRight
            | NotificationPosition::BottomCenter => {
                bounds.origin.y + bounds.size.height
                    - padding
                    - self.notification_height
                    - index as f32 * total_height
            }
        };

        Bounds::new(x, y, self.notification_width, self.notification_height)
    }

    fn dismiss_button_bounds(&self, notif_bounds: &Bounds) -> Bounds {
        let btn_size = 20.0;
        let padding = theme::spacing::XS;
        Bounds::new(
            notif_bounds.origin.x + notif_bounds.size.width - btn_size - padding,
            notif_bounds.origin.y + padding,
            btn_size,
            btn_size,
        )
    }
}

impl Default for Notifications {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Notifications {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        self.tick();

        let visible_count = self.notifications.len().min(self.max_visible);

        for (index, notification) in self.notifications.iter().take(visible_count).enumerate() {
            let notif_bounds = self.notification_bounds(&bounds, index);

            cx.scene.draw_quad(
                Quad::new(notif_bounds)
                    .with_background(theme::bg::SURFACE)
                    .with_border(notification.level.color(), 2.0),
            );

            let padding = theme::spacing::SM;
            let icon_size = theme::font_size::LG;

            let icon_run = cx.text.layout(
                notification.level.icon(),
                Point::new(
                    notif_bounds.origin.x + padding,
                    notif_bounds.origin.y + padding,
                ),
                icon_size,
                notification.level.color(),
            );
            cx.scene.draw_text(icon_run);

            let title_x = notif_bounds.origin.x + padding + icon_size + padding;
            let title_run = cx.text.layout(
                &notification.title,
                Point::new(title_x, notif_bounds.origin.y + padding),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(title_run);

            if let Some(msg) = &notification.message {
                let msg_run = cx.text.layout(
                    msg,
                    Point::new(
                        title_x,
                        notif_bounds.origin.y + padding + theme::font_size::SM + 4.0,
                    ),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(msg_run);
            }

            if notification.dismissible {
                let dismiss_bounds = self.dismiss_button_bounds(&notif_bounds);
                let is_hovered = self.hovered_dismiss == Some(notification.id);

                let dismiss_bg = if is_hovered {
                    theme::bg::HOVER
                } else {
                    Hsla::transparent()
                };

                cx.scene
                    .draw_quad(Quad::new(dismiss_bounds).with_background(dismiss_bg));

                let x_symbol = "\u{2715}";
                let x_run = cx.text.layout(
                    x_symbol,
                    Point::new(
                        dismiss_bounds.origin.x
                            + (dismiss_bounds.size.width - theme::font_size::SM * 0.6) / 2.0,
                        dismiss_bounds.origin.y
                            + (dismiss_bounds.size.height - theme::font_size::SM) / 2.0,
                    ),
                    theme::font_size::SM,
                    if is_hovered {
                        theme::text::PRIMARY
                    } else {
                        theme::text::MUTED
                    },
                );
                cx.scene.draw_text(x_run);
            }

            if let Some(duration) = notification.duration {
                let elapsed = notification.created_at.elapsed();
                let progress = 1.0 - (elapsed.as_secs_f32() / duration.as_secs_f32()).min(1.0);

                if progress > 0.0 {
                    let bar_height = 3.0;
                    let bar_width = notif_bounds.size.width * progress;
                    let bar_bounds = Bounds::new(
                        notif_bounds.origin.x,
                        notif_bounds.origin.y + notif_bounds.size.height - bar_height,
                        bar_width,
                        bar_height,
                    );

                    cx.scene.draw_quad(
                        Quad::new(bar_bounds)
                            .with_background(notification.level.color().with_alpha(0.5)),
                    );
                }
            }
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let visible_count = self.notifications.len().min(self.max_visible);

                self.hovered_dismiss = None;

                for (index, notification) in
                    self.notifications.iter().take(visible_count).enumerate()
                {
                    if notification.dismissible {
                        let notif_bounds = self.notification_bounds(&bounds, index);
                        let dismiss_bounds = self.dismiss_button_bounds(&notif_bounds);

                        if dismiss_bounds.contains(point) {
                            self.hovered_dismiss = Some(notification.id);
                            return EventResult::Handled;
                        }
                    }
                }
            }
            InputEvent::MouseUp { x, y, .. } => {
                let point = Point::new(*x, *y);
                let visible_count = self.notifications.len().min(self.max_visible);

                for (index, notification) in
                    self.notifications.iter().take(visible_count).enumerate()
                {
                    if notification.dismissible {
                        let notif_bounds = self.notification_bounds(&bounds, index);
                        let dismiss_bounds = self.dismiss_button_bounds(&notif_bounds);

                        if dismiss_bounds.contains(point) {
                            let id = notification.id;
                            self.dismiss(id);
                            return EventResult::Handled;
                        }
                    }
                }
            }
            _ => {}
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bounds, EventContext, EventResult, InputEvent, MouseButton, Point};
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Instant;

    #[test]
    fn test_notification_level_colors() {
        assert_eq!(NotificationLevel::Info.color(), theme::accent::PRIMARY);
        assert_eq!(NotificationLevel::Success.color(), theme::status::SUCCESS);
        assert_eq!(NotificationLevel::Warning.color(), theme::status::WARNING);
        assert_eq!(NotificationLevel::Error.color(), theme::status::ERROR);
    }

    #[test]
    fn test_notification_new() {
        let notif = Notification::new(1, "Test");
        assert_eq!(notif.id, 1);
        assert_eq!(notif.title, "Test");
        assert_eq!(notif.level, NotificationLevel::Info);
        assert!(notif.dismissible);
    }

    #[test]
    fn test_notification_builder() {
        let notif = Notification::new(1, "Title")
            .message("Details here")
            .level(NotificationLevel::Warning)
            .duration(Duration::from_secs(10))
            .dismissible(false);

        assert_eq!(notif.message, Some("Details here".to_string()));
        assert_eq!(notif.level, NotificationLevel::Warning);
        assert_eq!(notif.duration, Some(Duration::from_secs(10)));
        assert!(!notif.dismissible);
    }

    #[test]
    fn test_notification_helpers() {
        let info = Notification::info(1, "Info");
        assert_eq!(info.level, NotificationLevel::Info);

        let success = Notification::success(2, "Success");
        assert_eq!(success.level, NotificationLevel::Success);

        let warning = Notification::warning(3, "Warning");
        assert_eq!(warning.level, NotificationLevel::Warning);

        let error = Notification::error(4, "Error");
        assert_eq!(error.level, NotificationLevel::Error);
    }

    #[test]
    fn test_notification_persistent() {
        let notif = Notification::new(1, "Test").persistent();
        assert!(notif.duration.is_none());
        assert!(!notif.is_expired());
    }

    #[test]
    fn test_notifications_new() {
        let notifs = Notifications::new();
        assert_eq!(notifs.count(), 0);
        assert_eq!(notifs.max_visible, 5);
    }

    #[test]
    fn test_notifications_push() {
        let mut notifs = Notifications::new();
        let id = notifs.push(Notification::new(1, "Test"));

        assert_eq!(id, 1);
        assert_eq!(notifs.count(), 1);
    }

    #[test]
    fn test_notifications_helpers() {
        let mut notifs = Notifications::new();

        notifs.info("Info");
        notifs.success("Success");
        notifs.warning("Warning");
        notifs.error("Error");

        assert_eq!(notifs.count(), 4);
    }

    #[test]
    fn test_notifications_dismiss() {
        let mut notifs = Notifications::new();
        let id = notifs.notify("Test");
        assert_eq!(notifs.count(), 1);

        notifs.dismiss(id);
        assert_eq!(notifs.count(), 0);
    }

    #[test]
    fn test_notifications_clear() {
        let mut notifs = Notifications::new();
        notifs.notify("One");
        notifs.notify("Two");
        notifs.notify("Three");
        assert_eq!(notifs.count(), 3);

        notifs.clear();
        assert_eq!(notifs.count(), 0);
    }

    #[test]
    fn test_notifications_builder() {
        let notifs = Notifications::new()
            .with_id(1)
            .position(NotificationPosition::BottomRight)
            .max_visible(3)
            .notification_width(400.0);

        assert_eq!(notifs.id, Some(1));
        assert_eq!(notifs.position, NotificationPosition::BottomRight);
        assert_eq!(notifs.max_visible, 3);
        assert_eq!(notifs.notification_width, 400.0);
    }

    #[test]
    fn test_notification_expired_logic() {
        let mut notif = Notification::new(1, "Test").duration(Duration::from_millis(5));
        notif.created_at = Instant::now() - Duration::from_millis(10);
        assert!(notif.is_expired());
    }

    #[test]
    fn test_notifications_tick_removes_expired() {
        let dismissed = Rc::new(RefCell::new(Vec::new()));
        let dismissed_clone = Rc::clone(&dismissed);

        let mut notifs = Notifications::new().on_dismiss(move |id| {
            dismissed_clone.borrow_mut().push(id);
        });

        let mut expired = Notification::new(1, "Expired").duration(Duration::from_millis(1));
        expired.created_at = Instant::now() - Duration::from_millis(5);
        let fresh = Notification::new(2, "Fresh").duration(Duration::from_secs(60));

        notifs.push(expired);
        notifs.push(fresh);
        notifs.tick();

        assert_eq!(notifs.count(), 1);
        assert_eq!(notifs.notifications[0].id, 2);
        assert_eq!(&*dismissed.borrow(), &[1]);
    }

    #[test]
    fn test_notifications_bounds_positions() {
        let bounds = Bounds::new(0.0, 0.0, 800.0, 600.0);
        let padding = theme::spacing::MD;

        let notifs = Notifications::new().position(NotificationPosition::TopLeft);
        let tl = notifs.notification_bounds(&bounds, 0);
        assert!((tl.origin.x - padding).abs() < 0.01);
        assert!((tl.origin.y - padding).abs() < 0.01);

        let notifs = Notifications::new().position(NotificationPosition::TopCenter);
        let tc = notifs.notification_bounds(&bounds, 0);
        let expected_center_x = (bounds.size.width - notifs.notification_width) / 2.0;
        assert!((tc.origin.x - expected_center_x).abs() < 0.01);
        assert!((tc.origin.y - padding).abs() < 0.01);

        let notifs = Notifications::new().position(NotificationPosition::BottomRight);
        let br = notifs.notification_bounds(&bounds, 0);
        let expected_right_x = bounds.size.width - notifs.notification_width - padding;
        let expected_bottom_y = bounds.size.height - notifs.notification_height - padding;
        assert!((br.origin.x - expected_right_x).abs() < 0.01);
        assert!((br.origin.y - expected_bottom_y).abs() < 0.01);

        let notifs = Notifications::new().position(NotificationPosition::BottomCenter);
        let bc = notifs.notification_bounds(&bounds, 0);
        assert!((bc.origin.x - expected_center_x).abs() < 0.01);
        assert!((bc.origin.y - expected_bottom_y).abs() < 0.01);
    }

    #[test]
    fn test_notifications_event_hover_and_dismiss() {
        let dismissed = Rc::new(RefCell::new(Vec::new()));
        let dismissed_clone = Rc::clone(&dismissed);
        let mut notifs = Notifications::new().on_dismiss(move |id| {
            dismissed_clone.borrow_mut().push(id);
        });

        let id = notifs.notify("Hello");
        let bounds = Bounds::new(0.0, 0.0, 800.0, 600.0);
        let notif_bounds = notifs.notification_bounds(&bounds, 0);
        let dismiss_bounds = notifs.dismiss_button_bounds(&notif_bounds);
        let point = Point::new(dismiss_bounds.origin.x + 1.0, dismiss_bounds.origin.y + 1.0);

        let mut cx = EventContext::new();
        let hover = InputEvent::MouseMove {
            x: point.x,
            y: point.y,
        };
        let result = notifs.event(&hover, bounds, &mut cx);
        assert_eq!(result, EventResult::Handled);
        assert_eq!(notifs.hovered_dismiss, Some(id));

        let click = InputEvent::MouseUp {
            button: MouseButton::Left,
            x: point.x,
            y: point.y,
        };
        let result = notifs.event(&click, bounds, &mut cx);
        assert_eq!(result, EventResult::Handled);
        assert_eq!(notifs.count(), 0);
        assert_eq!(&*dismissed.borrow(), &[id]);
    }

    #[test]
    fn test_notifications_max_visible_limits_events() {
        let mut notifs = Notifications::new().max_visible(1);
        let _first = notifs.notify("First");
        let _second = notifs.notify("Second");
        let bounds = Bounds::new(0.0, 0.0, 800.0, 600.0);

        let second_bounds = notifs.notification_bounds(&bounds, 1);
        let dismiss_bounds = notifs.dismiss_button_bounds(&second_bounds);
        let point = Point::new(dismiss_bounds.origin.x + 1.0, dismiss_bounds.origin.y + 1.0);

        let mut cx = EventContext::new();
        let click = InputEvent::MouseUp {
            button: MouseButton::Left,
            x: point.x,
            y: point.y,
        };
        let result = notifs.event(&click, bounds, &mut cx);
        assert_eq!(result, EventResult::Ignored);
        assert_eq!(notifs.count(), 2);
    }
}
