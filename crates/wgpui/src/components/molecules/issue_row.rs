//! Issue row molecule for displaying issue list items with bounty.
//!
//! Shows issue title, labels, status, and optional bounty amount.

use crate::components::atoms::IssueStatus;
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Issue label
#[derive(Debug, Clone)]
pub struct IssueLabel {
    pub name: String,
    pub color: Hsla,
}

impl IssueLabel {
    pub fn new(name: impl Into<String>, color: Hsla) -> Self {
        Self {
            name: name.into(),
            color,
        }
    }

    pub fn bug() -> Self {
        Self::new("bug", Hsla::new(0.0, 0.7, 0.5, 1.0))
    }

    pub fn enhancement() -> Self {
        Self::new("enhancement", Hsla::new(200.0, 0.7, 0.5, 1.0))
    }

    pub fn good_first_issue() -> Self {
        Self::new("good first issue", Hsla::new(270.0, 0.6, 0.5, 1.0))
    }

    pub fn help_wanted() -> Self {
        Self::new("help wanted", Hsla::new(120.0, 0.6, 0.45, 1.0))
    }
}

/// Issue info
#[derive(Debug, Clone)]
pub struct IssueInfo {
    pub id: String,
    pub number: u32,
    pub title: String,
    pub status: IssueStatus,
    pub labels: Vec<IssueLabel>,
    pub author: String,
    pub bounty_sats: Option<u64>,
    pub comments: u32,
    pub created_at: String,
}

impl IssueInfo {
    pub fn new(id: impl Into<String>, number: u32, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            number,
            title: title.into(),
            status: IssueStatus::Open,
            labels: Vec::new(),
            author: "unknown".to_string(),
            bounty_sats: None,
            comments: 0,
            created_at: "Just now".to_string(),
        }
    }

    pub fn status(mut self, status: IssueStatus) -> Self {
        self.status = status;
        self
    }

    pub fn label(mut self, label: IssueLabel) -> Self {
        self.labels.push(label);
        self
    }

    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = author.into();
        self
    }

    pub fn bounty(mut self, sats: u64) -> Self {
        self.bounty_sats = Some(sats);
        self
    }

    pub fn comments(mut self, count: u32) -> Self {
        self.comments = count;
        self
    }

    pub fn created_at(mut self, time: impl Into<String>) -> Self {
        self.created_at = time.into();
        self
    }
}

/// Issue row component
pub struct IssueRow {
    id: Option<ComponentId>,
    issue: IssueInfo,
    hovered: bool,
    on_click: Option<Box<dyn FnMut(String)>>,
}

impl IssueRow {
    pub fn new(issue: IssueInfo) -> Self {
        Self {
            id: None,
            issue,
            hovered: false,
            on_click: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn on_click<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_click = Some(Box::new(f));
        self
    }

    fn format_bounty(&self) -> Option<String> {
        self.issue.bounty_sats.map(|sats| {
            if sats >= 1_000_000 {
                format!("{:.2}M sats", sats as f64 / 1_000_000.0)
            } else if sats >= 1_000 {
                format!("{:.1}K sats", sats as f64 / 1_000.0)
            } else {
                format!("{} sats", sats)
            }
        })
    }
}

impl Component for IssueRow {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;

        // Background
        let bg = if self.hovered {
            theme::bg::HOVER
        } else {
            theme::bg::SURFACE
        };
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        // Status indicator bar
        let status_color = self.issue.status.color();
        let bar_w = 3.0;
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                bounds.origin.y,
                bar_w,
                bounds.size.height,
            ))
            .with_background(status_color),
        );

        let content_x = bounds.origin.x + padding + bar_w;
        let text_y = bounds.origin.y + 12.0;

        // Issue number
        let num_text = format!("#{}", self.issue.number);
        let num_run = cx.text.layout(
            &num_text,
            Point::new(content_x, text_y),
            theme::font_size::SM,
            theme::text::MUTED,
        );
        cx.scene.draw_text(num_run);

        // Title
        let title = if self.issue.title.len() > 50 {
            format!("{}...", &self.issue.title[..47])
        } else {
            self.issue.title.clone()
        };
        let title_run = cx.text.layout(
            &title,
            Point::new(content_x + 50.0, text_y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        // Labels row
        let labels_y = text_y + 22.0;
        let mut label_x = content_x;

        for label in &self.issue.labels {
            let label_w = (label.name.len() as f32 * 6.0) + 12.0;
            let label_bounds = Bounds::new(label_x, labels_y, label_w, 16.0);
            cx.scene.draw_quad(
                Quad::new(label_bounds)
                    .with_background(label.color.with_alpha(0.2))
                    .with_border(label.color, 1.0),
            );
            let label_text = cx.text.layout(
                &label.name,
                Point::new(label_x + 4.0, labels_y + 2.0),
                theme::font_size::XS,
                label.color,
            );
            cx.scene.draw_text(label_text);
            label_x += label_w + 6.0;
        }

        // Bounty badge (if present)
        if let Some(bounty_text) = self.format_bounty() {
            let bounty_x = bounds.origin.x + bounds.size.width - padding - 100.0;
            let bounty_bounds = Bounds::new(bounty_x, text_y - 2.0, 90.0, 20.0);
            let bounty_color = Hsla::new(35.0, 0.9, 0.5, 1.0); // Bitcoin orange
            cx.scene.draw_quad(
                Quad::new(bounty_bounds)
                    .with_background(bounty_color.with_alpha(0.2))
                    .with_border(bounty_color, 1.0),
            );
            let bounty_run = cx.text.layout(
                &format!("\u{20BF} {}", bounty_text),
                Point::new(bounty_x + 6.0, text_y),
                theme::font_size::XS,
                bounty_color,
            );
            cx.scene.draw_text(bounty_run);
        }

        // Meta info (author, comments, time)
        let meta_y = labels_y + 22.0;
        let author_text = format!("by {}", self.issue.author);
        let author_run = cx.text.layout(
            &author_text,
            Point::new(content_x, meta_y),
            theme::font_size::XS,
            theme::text::DISABLED,
        );
        cx.scene.draw_text(author_run);

        let comments_text = format!("\u{1F4AC} {}", self.issue.comments);
        let comments_run = cx.text.layout(
            &comments_text,
            Point::new(content_x + 120.0, meta_y),
            theme::font_size::XS,
            theme::text::DISABLED,
        );
        cx.scene.draw_text(comments_run);

        let time_run = cx.text.layout(
            &self.issue.created_at,
            Point::new(content_x + 180.0, meta_y),
            theme::font_size::XS,
            theme::text::DISABLED,
        );
        cx.scene.draw_text(time_run);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let was_hovered = self.hovered;
                self.hovered = bounds.contains(Point::new(*x, *y));
                if was_hovered != self.hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left && bounds.contains(Point::new(*x, *y)) {
                    if let Some(callback) = &mut self.on_click {
                        callback(self.issue.id.clone());
                    }
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
        (None, Some(80.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_info() {
        let issue = IssueInfo::new("1", 42, "Fix memory leak in parser")
            .status(IssueStatus::Open)
            .label(IssueLabel::bug())
            .bounty(50000)
            .author("alice");

        assert_eq!(issue.number, 42);
        assert_eq!(issue.bounty_sats, Some(50000));
    }

    #[test]
    fn test_format_bounty() {
        let issue = IssueInfo::new("1", 1, "Test").bounty(1500);
        let row = IssueRow::new(issue);
        assert_eq!(row.format_bounty(), Some("1.5K sats".to_string()));
    }

    #[test]
    fn test_labels() {
        let bug = IssueLabel::bug();
        assert_eq!(bug.name, "bug");

        let enhancement = IssueLabel::enhancement();
        assert_eq!(enhancement.name, "enhancement");
    }
}
