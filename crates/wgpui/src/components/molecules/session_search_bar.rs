//! Session search bar for filtering and searching sessions.
//!
//! Provides search input and status filter chips for session browsing.

use crate::components::atoms::SessionStatus;
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult, TextInput};
use crate::{Bounds, InputEvent, MouseButton, Point, Quad, theme};

/// Filter chip for session status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusFilter {
    pub status: SessionStatus,
    pub active: bool,
}

/// Session search bar with filters
pub struct SessionSearchBar {
    id: Option<ComponentId>,
    input: TextInput,
    status_filters: Vec<StatusFilter>,
    hovered_filter: Option<usize>,
    on_search: Option<Box<dyn FnMut(String)>>,
    on_filter_change: Option<Box<dyn FnMut(SessionStatus, bool)>>,
}

impl SessionSearchBar {
    pub fn new() -> Self {
        let filters = vec![
            StatusFilter {
                status: SessionStatus::Running,
                active: false,
            },
            StatusFilter {
                status: SessionStatus::Completed,
                active: false,
            },
            StatusFilter {
                status: SessionStatus::Failed,
                active: false,
            },
            StatusFilter {
                status: SessionStatus::Paused,
                active: false,
            },
        ];

        Self {
            id: None,
            input: TextInput::new()
                .placeholder("Search sessions...")
                .background(theme::bg::SURFACE),
            status_filters: filters,
            hovered_filter: None,
            on_search: None,
            on_filter_change: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.input = self.input.placeholder(placeholder);
        self
    }

    pub fn on_search<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_search = Some(Box::new(f));
        self
    }

    pub fn on_filter_change<F>(mut self, f: F) -> Self
    where
        F: FnMut(SessionStatus, bool) + 'static,
    {
        self.on_filter_change = Some(Box::new(f));
        self
    }

    pub fn search_value(&self) -> &str {
        self.input.get_value()
    }

    pub fn active_filters(&self) -> Vec<SessionStatus> {
        self.status_filters
            .iter()
            .filter(|f| f.active)
            .map(|f| f.status)
            .collect()
    }

    pub fn set_filter(&mut self, status: SessionStatus, active: bool) {
        for filter in &mut self.status_filters {
            if filter.status == status {
                filter.active = active;
                break;
            }
        }
    }

    pub fn clear_filters(&mut self) {
        for filter in &mut self.status_filters {
            filter.active = false;
        }
    }

    fn filter_bounds(&self, bounds: &Bounds) -> Vec<Bounds> {
        let chip_height = 24.0;
        let chip_gap = 8.0;
        let input_width = 200.0;
        let start_x = bounds.origin.x + input_width + 20.0;
        let y = bounds.origin.y + (bounds.size.height - chip_height) / 2.0;

        let mut result = Vec::new();
        let mut x = start_x;

        for filter in &self.status_filters {
            let label = filter.status.label();
            let width = label.len() as f32 * 7.0 + 16.0;
            result.push(Bounds::new(x, y, width, chip_height));
            x += width + chip_gap;
        }

        result
    }
}

impl Default for SessionSearchBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SessionSearchBar {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let padding = 12.0;

        // Search icon
        let icon_run = cx.text.layout(
            "⌕",
            Point::new(
                bounds.origin.x + padding,
                bounds.origin.y + (bounds.size.height - theme::font_size::SM) / 2.0,
            ),
            theme::font_size::SM,
            theme::text::MUTED,
        );
        cx.scene.draw_text(icon_run);

        // Search input
        let input_x = bounds.origin.x + padding + 20.0;
        let input_bounds = Bounds::new(
            input_x,
            bounds.origin.y + (bounds.size.height - 28.0) / 2.0,
            180.0,
            28.0,
        );
        self.input.paint(input_bounds, cx);

        // Filter chips
        let filter_bounds = self.filter_bounds(&bounds);
        for (idx, (filter, chip_bounds)) in self
            .status_filters
            .iter()
            .zip(filter_bounds.iter())
            .enumerate()
        {
            let is_hovered = self.hovered_filter == Some(idx);
            let status_color = filter.status.color();

            let bg = if filter.active {
                status_color.with_alpha(0.3)
            } else if is_hovered {
                theme::bg::HOVER
            } else {
                theme::bg::MUTED
            };

            let border = if filter.active {
                status_color
            } else {
                theme::border::DEFAULT
            };

            cx.scene.draw_quad(
                Quad::new(*chip_bounds)
                    .with_background(bg)
                    .with_border(border, 1.0),
            );

            let text_color = if filter.active {
                status_color
            } else if is_hovered {
                theme::text::PRIMARY
            } else {
                theme::text::MUTED
            };

            let label = filter.status.label();
            let label_run = cx.text.layout(
                label,
                Point::new(
                    chip_bounds.origin.x + 8.0,
                    chip_bounds.origin.y + (chip_bounds.size.height - theme::font_size::XS) / 2.0,
                ),
                theme::font_size::XS,
                text_color,
            );
            cx.scene.draw_text(label_run);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let filter_bounds = self.filter_bounds(&bounds);

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_hovered = self.hovered_filter;
                self.hovered_filter = None;

                for (idx, chip_bounds) in filter_bounds.iter().enumerate() {
                    if chip_bounds.contains(point) {
                        self.hovered_filter = Some(idx);
                        break;
                    }
                }

                if was_hovered != self.hovered_filter {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    for (idx, chip_bounds) in filter_bounds.iter().enumerate() {
                        if chip_bounds.contains(point) {
                            let filter = &mut self.status_filters[idx];
                            filter.active = !filter.active;
                            let status = filter.status;
                            let active = filter.active;

                            if let Some(callback) = &mut self.on_filter_change {
                                callback(status, active);
                            }
                            return EventResult::Handled;
                        }
                    }
                }
            }
            _ => {}
        }

        // Forward to input
        let padding = 12.0;
        let input_x = bounds.origin.x + padding + 20.0;
        let input_bounds = Bounds::new(
            input_x,
            bounds.origin.y + (bounds.size.height - 28.0) / 2.0,
            180.0,
            28.0,
        );

        let result = self.input.event(event, input_bounds, cx);

        // Trigger search on input change
        if result == EventResult::Handled
            && let Some(callback) = &mut self.on_search
        {
            callback(self.input.get_value().to_string());
        }

        result
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, Some(44.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_search_bar_new() {
        let bar = SessionSearchBar::new();
        assert_eq!(bar.search_value(), "");
        assert!(bar.active_filters().is_empty());
    }

    #[test]
    fn test_session_search_bar_filters() {
        let mut bar = SessionSearchBar::new();
        bar.set_filter(SessionStatus::Running, true);
        bar.set_filter(SessionStatus::Failed, true);

        let active = bar.active_filters();
        assert_eq!(active.len(), 2);
        assert!(active.contains(&SessionStatus::Running));
        assert!(active.contains(&SessionStatus::Failed));

        bar.clear_filters();
        assert!(bar.active_filters().is_empty());
    }
}
