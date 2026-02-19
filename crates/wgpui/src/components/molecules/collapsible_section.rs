//! Collapsible section component for grouping related content with expand/collapse.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, MouseButton, Point, Quad, theme};

/// Status indicator for a collapsible section
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SectionStatus {
    #[default]
    Pending,
    InProgress,
    Success,
    Error,
}

impl SectionStatus {
    fn icon(&self) -> &'static str {
        match self {
            SectionStatus::Pending => "",
            SectionStatus::InProgress => "...",
            SectionStatus::Success => "[OK]",
            SectionStatus::Error => "[X]",
        }
    }

    fn color(&self) -> crate::Hsla {
        match self {
            SectionStatus::Pending => theme::text::MUTED,
            SectionStatus::InProgress => theme::status::WARNING,
            SectionStatus::Success => theme::status::SUCCESS,
            SectionStatus::Error => theme::status::ERROR,
        }
    }
}

/// A collapsible section that groups multiple lines of content.
/// Shows a summary line when collapsed, and all detail lines when expanded.
pub struct CollapsibleSection {
    id: Option<ComponentId>,
    expanded: bool,
    hovered: bool,
    summary: String,
    details: Vec<String>,
    status: SectionStatus,
    font_size: f32,
    on_toggle: Option<Box<dyn FnMut(bool)>>,
}

impl CollapsibleSection {
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            id: None,
            expanded: false,
            hovered: false,
            summary: summary.into(),
            details: Vec::new(),
            status: SectionStatus::default(),
            font_size: theme::font_size::SM,
            on_toggle: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn details(mut self, details: Vec<String>) -> Self {
        self.details = details;
        self
    }

    pub fn add_detail(mut self, detail: impl Into<String>) -> Self {
        self.details.push(detail.into());
        self
    }

    pub fn status(mut self, status: SectionStatus) -> Self {
        self.status = status;
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn on_toggle<F>(mut self, f: F) -> Self
    where
        F: FnMut(bool) + 'static,
    {
        self.on_toggle = Some(Box::new(f));
        self
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
        if let Some(on_toggle) = &mut self.on_toggle {
            on_toggle(self.expanded);
        }
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    pub fn detail_count(&self) -> usize {
        self.details.len()
    }

    fn header_height(&self) -> f32 {
        self.font_size * 1.6
    }

    fn line_height(&self) -> f32 {
        self.font_size * 1.4
    }
}

impl Default for CollapsibleSection {
    fn default() -> Self {
        Self::new("")
    }
}

impl Component for CollapsibleSection {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = theme::spacing::SM;
        let header_height = self.header_height();
        let line_height = self.line_height();
        let indent = theme::spacing::LG;

        // Header background on hover
        let header_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            header_height,
        );
        if self.hovered {
            cx.scene.draw_quad(
                Quad::new(header_bounds).with_background(theme::bg::MUTED.with_alpha(0.3)),
            );
        }

        // Arrow indicator
        let arrow = if self.expanded { "v" } else { ">" };
        let arrow_color = if self.hovered {
            theme::text::PRIMARY
        } else {
            theme::text::MUTED
        };
        let arrow_run = cx.text.layout_mono(
            arrow,
            Point::new(
                bounds.origin.x + padding,
                bounds.origin.y + header_height * 0.3,
            ),
            self.font_size,
            arrow_color,
        );
        cx.scene.draw_text(arrow_run);

        // Summary text
        let summary_x = bounds.origin.x + padding + self.font_size * 1.2;
        let summary_color = if self.hovered {
            theme::text::PRIMARY
        } else {
            theme::text::SECONDARY
        };
        let summary_run = cx.text.layout_mono(
            &self.summary,
            Point::new(summary_x, bounds.origin.y + header_height * 0.3),
            self.font_size,
            summary_color,
        );
        cx.scene.draw_text(summary_run);

        // Status icon on right
        let status_icon = self.status.icon();
        if !status_icon.is_empty() {
            let status_width = status_icon.len() as f32 * self.font_size * 0.6;
            let status_run = cx.text.layout_mono(
                status_icon,
                Point::new(
                    bounds.origin.x + bounds.size.width - status_width - padding,
                    bounds.origin.y + header_height * 0.3,
                ),
                self.font_size,
                self.status.color(),
            );
            cx.scene.draw_text(status_run);
        }

        // Detail lines (only when expanded)
        if self.expanded && !self.details.is_empty() {
            let mut y = bounds.origin.y + header_height + theme::spacing::XS;

            for detail in &self.details {
                if y + line_height > bounds.origin.y + bounds.size.height {
                    break;
                }

                let detail_run = cx.text.layout_mono(
                    detail,
                    Point::new(bounds.origin.x + padding + indent, y),
                    self.font_size,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(detail_run);

                y += line_height;
            }
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let header_height = self.header_height();
        let header_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            header_height,
        );

        match event {
            InputEvent::MouseMove { x, y } => {
                let was_hovered = self.hovered;
                self.hovered = header_bounds.contains(Point::new(*x, *y));
                if was_hovered != self.hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left && header_bounds.contains(Point::new(*x, *y)) {
                    self.toggle();
                    return EventResult::Handled;
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
        let header_height = self.header_height();

        let height = if self.expanded && !self.details.is_empty() {
            let line_height = self.line_height();
            header_height + theme::spacing::XS + (self.details.len() as f32 * line_height)
        } else {
            header_height
        };

        (None, Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collapsible_section_new() {
        let section = CollapsibleSection::new("Test Summary");
        assert_eq!(section.summary(), "Test Summary");
        assert!(!section.is_expanded());
        assert_eq!(section.detail_count(), 0);
    }

    #[test]
    fn test_collapsible_section_builder() {
        let section = CollapsibleSection::new("Auth ready")
            .with_id(1)
            .expanded(true)
            .status(SectionStatus::Success)
            .details(vec![
                "Checking auth...".to_string(),
                "Found credentials".to_string(),
            ]);

        assert_eq!(section.id, Some(1));
        assert!(section.is_expanded());
        assert_eq!(section.detail_count(), 2);
        assert_eq!(section.status, SectionStatus::Success);
    }

    #[test]
    fn test_toggle() {
        let mut section = CollapsibleSection::new("Test");
        assert!(!section.is_expanded());
        section.toggle();
        assert!(section.is_expanded());
        section.toggle();
        assert!(!section.is_expanded());
    }

    #[test]
    fn test_size_hint_collapsed() {
        let section = CollapsibleSection::new("Summary")
            .details(vec!["Detail 1".to_string(), "Detail 2".to_string()]);

        let (_, height) = section.size_hint();
        // Collapsed should only be header height
        assert!(height.is_some());
        let h = height.unwrap();
        assert!(h < 30.0); // Header only
    }

    #[test]
    fn test_size_hint_expanded() {
        let section = CollapsibleSection::new("Summary")
            .expanded(true)
            .details(vec!["Detail 1".to_string(), "Detail 2".to_string()]);

        let (_, height) = section.size_hint();
        // Expanded should include detail lines
        assert!(height.is_some());
        let h = height.unwrap();
        assert!(h > 30.0); // Header + details
    }
}
