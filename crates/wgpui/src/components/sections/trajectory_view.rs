use crate::components::atoms::{TrajectoryStatus, TrajectoryStatusBadge};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

#[derive(Debug, Clone)]
pub struct TrajectoryEntry {
    pub title: String,
    pub detail: Option<String>,
    pub timestamp: Option<String>,
    pub status: TrajectoryStatus,
}

impl TrajectoryEntry {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: None,
            timestamp: None,
            status: TrajectoryStatus::Unknown,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = Some(timestamp.into());
        self
    }

    pub fn status(mut self, status: TrajectoryStatus) -> Self {
        self.status = status;
        self
    }
}

pub struct TrajectoryView {
    id: Option<ComponentId>,
    entries: Vec<TrajectoryEntry>,
    scroll_offset: f32,
    content_height: f32,
    auto_scroll: bool,
    item_spacing: f32,
    on_entry_click: Option<Box<dyn FnMut(usize)>>,
}

impl TrajectoryView {
    pub fn new() -> Self {
        Self {
            id: None,
            entries: Vec::new(),
            scroll_offset: 0.0,
            content_height: 0.0,
            auto_scroll: true,
            item_spacing: theme::spacing::SM,
            on_entry_click: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn entries(mut self, entries: Vec<TrajectoryEntry>) -> Self {
        self.entries = entries;
        self
    }

    pub fn auto_scroll(mut self, auto: bool) -> Self {
        self.auto_scroll = auto;
        self
    }

    pub fn item_spacing(mut self, spacing: f32) -> Self {
        self.item_spacing = spacing;
        self
    }

    pub fn on_entry_click<F>(mut self, f: F) -> Self
    where
        F: FnMut(usize) + 'static,
    {
        self.on_entry_click = Some(Box::new(f));
        self
    }

    pub fn push_entry(&mut self, entry: TrajectoryEntry) {
        self.entries.push(entry);
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.scroll_offset = 0.0;
        self.content_height = 0.0;
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.content_height;
    }

    fn entry_height(entry: &TrajectoryEntry) -> f32 {
        let title_height = theme::font_size::SM * 1.6;
        let detail_height = if entry.detail.is_some() || entry.timestamp.is_some() {
            theme::font_size::XS * 1.4 + theme::spacing::XS
        } else {
            0.0
        };
        title_height + detail_height + theme::spacing::XS
    }

    fn calculate_content_height(&self) -> f32 {
        let mut height = 0.0;
        for entry in &self.entries {
            height += Self::entry_height(entry) + self.item_spacing;
        }
        height
    }

    fn detail_line(entry: &TrajectoryEntry) -> Option<String> {
        match (&entry.detail, &entry.timestamp) {
            (Some(detail), Some(ts)) => Some(format!("{} • {}", detail, ts)),
            (Some(detail), None) => Some(detail.clone()),
            (None, Some(ts)) => Some(ts.clone()),
            (None, None) => None,
        }
    }

    fn fit_text(cx: &mut PaintContext, text: &str, font_size: f32, max_width: f32) -> String {
        if text.is_empty() || max_width <= 0.0 {
            return String::new();
        }

        let char_width = cx.text.measure("W", font_size).max(1.0);
        let max_chars = (max_width / char_width).floor() as usize;
        if max_chars == 0 {
            return String::new();
        }

        let text_len = text.chars().count();
        if text_len <= max_chars {
            return text.to_string();
        }

        let ellipsis = "...";
        if max_chars <= ellipsis.len() {
            return ellipsis.chars().take(max_chars).collect();
        }

        let truncated = text
            .chars()
            .take(max_chars - ellipsis.len())
            .collect::<String>();
        format!("{}{}", truncated, ellipsis)
    }
}

impl Default for TrajectoryView {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for TrajectoryView {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene
            .draw_quad(Quad::new(bounds).with_background(theme::bg::APP));

        self.content_height = self.calculate_content_height();
        let max_scroll = (self.content_height - bounds.size.height).max(0.0);
        self.scroll_offset = self.scroll_offset.clamp(0.0, max_scroll);

        cx.scene.push_clip(bounds);

        let mut y = bounds.origin.y - self.scroll_offset;
        let padding = theme::spacing::SM;

        for entry in &self.entries {
            let height = Self::entry_height(entry);

            if y + height >= bounds.origin.y && y <= bounds.origin.y + bounds.size.height {
                let badge_bounds = Bounds::new(bounds.origin.x + padding, y + 2.0, 24.0, 22.0);
                let mut badge = TrajectoryStatusBadge::new(entry.status).compact(true);
                badge.paint(badge_bounds, cx);

                let text_x = badge_bounds.origin.x + badge_bounds.size.width + 8.0;
                let text_width = (bounds.origin.x + bounds.size.width - text_x - padding).max(0.0);

                let title = Self::fit_text(cx, &entry.title, theme::font_size::SM, text_width);
                let title_run = cx.text.layout(
                    &title,
                    Point::new(text_x, y),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(title_run);

                if let Some(detail_line) = Self::detail_line(entry) {
                    let detail = Self::fit_text(cx, &detail_line, theme::font_size::XS, text_width);
                    let detail_run = cx.text.layout(
                        &detail,
                        Point::new(text_x, y + theme::font_size::SM * 1.2),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(detail_run);
                }
            }

            y += height + self.item_spacing;
        }

        cx.scene.pop_clip();

        if self.content_height > bounds.size.height {
            let scrollbar_width = 6.0;
            let scrollbar_height = bounds.size.height * (bounds.size.height / self.content_height);
            let scrollbar_y =
                bounds.origin.y + (self.scroll_offset / self.content_height) * bounds.size.height;

            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x + bounds.size.width - scrollbar_width - 2.0,
                    scrollbar_y,
                    scrollbar_width,
                    scrollbar_height,
                ))
                .with_background(theme::bg::MUTED),
            );
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::Scroll { dy, .. } => {
                let max_scroll = (self.content_height - bounds.size.height).max(0.0);
                self.scroll_offset = (self.scroll_offset - dy).clamp(0.0, max_scroll);
                return EventResult::Handled;
            }
            InputEvent::MouseUp { x, y, .. } => {
                let point = Point::new(*x, *y);
                if bounds.contains(point) {
                    let mut check_y = bounds.origin.y - self.scroll_offset;
                    for (i, entry) in self.entries.iter().enumerate() {
                        let height = Self::entry_height(entry);
                        let entry_bounds =
                            Bounds::new(bounds.origin.x, check_y, bounds.size.width, height);
                        if entry_bounds.contains(point) {
                            if let Some(callback) = &mut self.on_entry_click {
                                callback(i);
                            }
                            return EventResult::Handled;
                        }
                        check_y += height + self.item_spacing;
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

    #[test]
    fn test_trajectory_entry_builder() {
        let entry = TrajectoryEntry::new("Step 1")
            .detail("Fetch metadata")
            .timestamp("12:01")
            .status(TrajectoryStatus::Verified);

        assert_eq!(entry.title, "Step 1");
        assert_eq!(entry.detail.as_deref(), Some("Fetch metadata"));
        assert_eq!(entry.timestamp.as_deref(), Some("12:01"));
        assert_eq!(entry.status, TrajectoryStatus::Verified);
    }

    #[test]
    fn test_trajectory_view_counts() {
        let mut view = TrajectoryView::new();
        view.push_entry(TrajectoryEntry::new("Step 1"));
        view.push_entry(TrajectoryEntry::new("Step 2"));
        assert_eq!(view.entry_count(), 2);
        view.clear();
        assert_eq!(view.entry_count(), 0);
    }
}
