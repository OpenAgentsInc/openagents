//! PR timeline item molecule for displaying PR activity.
//!
//! Shows PR events like commits, reviews, comments, and status changes.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// PR timeline event type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrEventType {
    Commit,
    Review,
    Comment,
    StatusChange,
    Merge,
    Close,
    Reopen,
}

impl PrEventType {
    pub fn icon(&self) -> &'static str {
        match self {
            PrEventType::Commit => "\u{2022}",
            PrEventType::Review => "\u{2713}",
            PrEventType::Comment => "\u{1F4AC}",
            PrEventType::StatusChange => "\u{2699}",
            PrEventType::Merge => "\u{2714}",
            PrEventType::Close => "\u{2716}",
            PrEventType::Reopen => "\u{21BB}",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            PrEventType::Commit => Hsla::new(200.0, 0.6, 0.5, 1.0), // Blue
            PrEventType::Review => Hsla::new(270.0, 0.6, 0.55, 1.0), // Purple
            PrEventType::Comment => theme::text::MUTED,
            PrEventType::StatusChange => Hsla::new(45.0, 0.7, 0.5, 1.0), // Yellow
            PrEventType::Merge => Hsla::new(120.0, 0.7, 0.45, 1.0),      // Green
            PrEventType::Close => Hsla::new(0.0, 0.7, 0.5, 1.0),         // Red
            PrEventType::Reopen => Hsla::new(120.0, 0.5, 0.5, 1.0),      // Light green
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            PrEventType::Commit => "committed",
            PrEventType::Review => "reviewed",
            PrEventType::Comment => "commented",
            PrEventType::StatusChange => "status changed",
            PrEventType::Merge => "merged",
            PrEventType::Close => "closed",
            PrEventType::Reopen => "reopened",
        }
    }
}

/// Review state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReviewState {
    Approved,
    RequestChanges,
    Commented,
    Pending,
}

impl ReviewState {
    pub fn label(&self) -> &'static str {
        match self {
            ReviewState::Approved => "Approved",
            ReviewState::RequestChanges => "Requested changes",
            ReviewState::Commented => "Commented",
            ReviewState::Pending => "Pending",
        }
    }

    pub fn color(&self) -> Hsla {
        match self {
            ReviewState::Approved => Hsla::new(120.0, 0.7, 0.45, 1.0), // Green
            ReviewState::RequestChanges => Hsla::new(0.0, 0.7, 0.5, 1.0), // Red
            ReviewState::Commented => theme::text::MUTED,
            ReviewState::Pending => Hsla::new(45.0, 0.7, 0.5, 1.0), // Yellow
        }
    }
}

/// PR timeline event
#[derive(Debug, Clone)]
pub struct PrEvent {
    pub id: String,
    pub event_type: PrEventType,
    pub actor: String,
    pub message: String,
    pub timestamp: String,
    pub commit_sha: Option<String>,
    pub review_state: Option<ReviewState>,
}

impl PrEvent {
    pub fn new(id: impl Into<String>, event_type: PrEventType, actor: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            event_type,
            actor: actor.into(),
            message: String::new(),
            timestamp: "Just now".to_string(),
            commit_sha: None,
            review_state: None,
        }
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    pub fn timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = timestamp.into();
        self
    }

    pub fn commit_sha(mut self, sha: impl Into<String>) -> Self {
        self.commit_sha = Some(sha.into());
        self
    }

    pub fn review_state(mut self, state: ReviewState) -> Self {
        self.review_state = Some(state);
        self
    }
}

/// PR timeline item component
pub struct PrTimelineItem {
    id: Option<ComponentId>,
    event: PrEvent,
    is_last: bool,
}

impl PrTimelineItem {
    pub fn new(event: PrEvent) -> Self {
        Self {
            id: None,
            event,
            is_last: false,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn is_last(mut self, is_last: bool) -> Self {
        self.is_last = is_last;
        self
    }
}

impl Component for PrTimelineItem {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;
        let timeline_x = bounds.origin.x + padding + 12.0;

        // Timeline dot
        let dot_size = 10.0;
        let dot_y = bounds.origin.y + 14.0;
        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                timeline_x - dot_size / 2.0,
                dot_y,
                dot_size,
                dot_size,
            ))
            .with_background(self.event.event_type.color())
            .with_border(self.event.event_type.color(), 1.0),
        );

        // Timeline line (unless last item)
        if !self.is_last {
            let line_height = bounds.size.height - dot_y - dot_size + bounds.origin.y;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    timeline_x - 1.0,
                    dot_y + dot_size + 2.0,
                    2.0,
                    line_height,
                ))
                .with_background(theme::border::DEFAULT),
            );
        }

        // Content area
        let content_x = timeline_x + 20.0;
        let content_y = bounds.origin.y + 8.0;

        // Event icon
        let icon_run = cx.text.layout_mono(
            self.event.event_type.icon(),
            Point::new(content_x, content_y),
            theme::font_size::SM,
            self.event.event_type.color(),
        );
        cx.scene.draw_text(icon_run);

        // Actor + action
        let action_text = format!("{} {}", self.event.actor, self.event.event_type.label());
        let action_run = cx.text.layout_mono(
            &action_text,
            Point::new(content_x + 20.0, content_y),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(action_run);

        // Timestamp
        let time_run = cx.text.layout_mono(
            &self.event.timestamp,
            Point::new(
                bounds.origin.x + bounds.size.width - padding - 80.0,
                content_y,
            ),
            theme::font_size::XS,
            theme::text::DISABLED,
        );
        cx.scene.draw_text(time_run);

        // Message or details
        let details_y = content_y + 22.0;

        if let Some(sha) = &self.event.commit_sha {
            let sha_short = &sha[..7.min(sha.len())];
            let sha_run = cx.text.layout_mono(
                sha_short,
                Point::new(content_x + 20.0, details_y),
                theme::font_size::XS,
                theme::accent::PRIMARY,
            );
            cx.scene.draw_text(sha_run);

            if !self.event.message.is_empty() {
                let msg = if self.event.message.len() > 50 {
                    format!("{}...", &self.event.message[..47])
                } else {
                    self.event.message.clone()
                };
                let msg_run = cx.text.layout_mono(
                    &msg,
                    Point::new(content_x + 80.0, details_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(msg_run);
            }
        } else if let Some(review_state) = &self.event.review_state {
            let state_bounds = Bounds::new(content_x + 20.0, details_y - 2.0, 100.0, 18.0);
            cx.scene.draw_quad(
                Quad::new(state_bounds)
                    .with_background(review_state.color().with_alpha(0.2))
                    .with_border(review_state.color(), 1.0),
            );
            let state_run = cx.text.layout_mono(
                review_state.label(),
                Point::new(content_x + 26.0, details_y),
                theme::font_size::XS,
                review_state.color(),
            );
            cx.scene.draw_text(state_run);
        } else if !self.event.message.is_empty() {
            let msg = if self.event.message.len() > 60 {
                format!("{}...", &self.event.message[..57])
            } else {
                self.event.message.clone()
            };
            let msg_run = cx.text.layout_mono(
                &msg,
                Point::new(content_x + 20.0, details_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(msg_run);
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
        (None, Some(60.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pr_event() {
        let event = PrEvent::new("1", PrEventType::Commit, "alice")
            .message("Fix typo in README")
            .commit_sha("abc1234");

        assert_eq!(event.actor, "alice");
        assert_eq!(event.commit_sha, Some("abc1234".to_string()));
    }

    #[test]
    fn test_event_types() {
        assert_eq!(PrEventType::Merge.label(), "merged");
        assert_eq!(PrEventType::Close.label(), "closed");
    }

    #[test]
    fn test_review_states() {
        assert_eq!(ReviewState::Approved.label(), "Approved");
        assert_eq!(ReviewState::RequestChanges.label(), "Requested changes");
    }
}
