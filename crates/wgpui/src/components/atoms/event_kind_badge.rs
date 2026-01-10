//! Event kind badge for Nostr events.
//!
//! Displays the kind number and a human-readable label for common event types.

use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, Point, Quad, theme};

/// Common Nostr event kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventKind {
    /// kind:0 - User metadata
    Metadata,
    /// kind:1 - Short text note
    #[default]
    TextNote,
    /// kind:2 - Recommend relay
    RecommendRelay,
    /// kind:3 - Contacts (follow list)
    Contacts,
    /// kind:4 - Encrypted DM (legacy)
    EncryptedDm,
    /// kind:5 - Event deletion
    Deletion,
    /// kind:6 - Repost
    Repost,
    /// kind:7 - Reaction
    Reaction,
    /// kind:40 - Channel create
    ChannelCreate,
    /// kind:42 - Channel message
    ChannelMessage,
    /// kind:1063 - File metadata
    FileMetadata,
    /// kind:9735 - Zap receipt
    ZapReceipt,
    /// kind:10002 - Relay list
    RelayList,
    /// kind:30023 - Long-form content
    LongFormContent,
    /// kind:30617 - Repository announcement
    RepoAnnounce,
    /// kind:1621 - Issue
    Issue,
    /// kind:1617 - Patch
    Patch,
    /// kind:1618 - Pull request
    PullRequest,
    /// kind:39200 - Agent profile (NIP-SA)
    AgentProfile,
    /// kind:39230 - Trajectory session (NIP-SA)
    TrajectorySession,
    /// kind:5050 - DVM text request
    DvmTextRequest,
    /// kind:6050 - DVM text result
    DvmTextResult,
    /// Custom kind number
    Custom(u32),
}

impl EventKind {
    /// Create from a raw kind number
    pub fn from_kind(kind: u32) -> Self {
        match kind {
            0 => EventKind::Metadata,
            1 => EventKind::TextNote,
            2 => EventKind::RecommendRelay,
            3 => EventKind::Contacts,
            4 => EventKind::EncryptedDm,
            5 => EventKind::Deletion,
            6 => EventKind::Repost,
            7 => EventKind::Reaction,
            40 => EventKind::ChannelCreate,
            42 => EventKind::ChannelMessage,
            1063 => EventKind::FileMetadata,
            9735 => EventKind::ZapReceipt,
            10002 => EventKind::RelayList,
            30023 => EventKind::LongFormContent,
            30617 => EventKind::RepoAnnounce,
            1621 => EventKind::Issue,
            1617 => EventKind::Patch,
            1618 => EventKind::PullRequest,
            39200 => EventKind::AgentProfile,
            39230 => EventKind::TrajectorySession,
            5050 => EventKind::DvmTextRequest,
            6050 => EventKind::DvmTextResult,
            k => EventKind::Custom(k),
        }
    }

    /// Get the raw kind number
    pub fn kind(&self) -> u32 {
        match self {
            EventKind::Metadata => 0,
            EventKind::TextNote => 1,
            EventKind::RecommendRelay => 2,
            EventKind::Contacts => 3,
            EventKind::EncryptedDm => 4,
            EventKind::Deletion => 5,
            EventKind::Repost => 6,
            EventKind::Reaction => 7,
            EventKind::ChannelCreate => 40,
            EventKind::ChannelMessage => 42,
            EventKind::FileMetadata => 1063,
            EventKind::ZapReceipt => 9735,
            EventKind::RelayList => 10002,
            EventKind::LongFormContent => 30023,
            EventKind::RepoAnnounce => 30617,
            EventKind::Issue => 1621,
            EventKind::Patch => 1617,
            EventKind::PullRequest => 1618,
            EventKind::AgentProfile => 39200,
            EventKind::TrajectorySession => 39230,
            EventKind::DvmTextRequest => 5050,
            EventKind::DvmTextResult => 6050,
            EventKind::Custom(k) => *k,
        }
    }

    /// Human-readable label
    pub fn label(&self) -> &'static str {
        match self {
            EventKind::Metadata => "Profile",
            EventKind::TextNote => "Note",
            EventKind::RecommendRelay => "Relay",
            EventKind::Contacts => "Contacts",
            EventKind::EncryptedDm => "DM",
            EventKind::Deletion => "Delete",
            EventKind::Repost => "Repost",
            EventKind::Reaction => "React",
            EventKind::ChannelCreate => "Channel",
            EventKind::ChannelMessage => "Msg",
            EventKind::FileMetadata => "File",
            EventKind::ZapReceipt => "Zap",
            EventKind::RelayList => "Relays",
            EventKind::LongFormContent => "Article",
            EventKind::RepoAnnounce => "Repo",
            EventKind::Issue => "Issue",
            EventKind::Patch => "Patch",
            EventKind::PullRequest => "PR",
            EventKind::AgentProfile => "Agent",
            EventKind::TrajectorySession => "Traj",
            EventKind::DvmTextRequest => "DVM Req",
            EventKind::DvmTextResult => "DVM Res",
            EventKind::Custom(_) => "Event",
        }
    }

    /// Category color
    pub fn color(&self) -> Hsla {
        match self {
            // Social (cyan)
            EventKind::TextNote | EventKind::Repost | EventKind::Reaction => {
                Hsla::new(180.0, 0.8, 0.5, 1.0)
            }
            // Identity (purple)
            EventKind::Metadata | EventKind::Contacts | EventKind::RelayList => {
                Hsla::new(280.0, 0.7, 0.6, 1.0)
            }
            // Messaging (blue)
            EventKind::EncryptedDm | EventKind::ChannelCreate | EventKind::ChannelMessage => {
                Hsla::new(210.0, 0.8, 0.55, 1.0)
            }
            // Git (orange)
            EventKind::RepoAnnounce
            | EventKind::Issue
            | EventKind::Patch
            | EventKind::PullRequest => Hsla::new(30.0, 0.9, 0.55, 1.0),
            // Agent (green)
            EventKind::AgentProfile | EventKind::TrajectorySession => {
                Hsla::new(140.0, 0.7, 0.5, 1.0)
            }
            // DVM (yellow)
            EventKind::DvmTextRequest | EventKind::DvmTextResult => Hsla::new(50.0, 0.9, 0.5, 1.0),
            // Payments (gold)
            EventKind::ZapReceipt => Hsla::new(45.0, 0.95, 0.55, 1.0),
            // Content (teal)
            EventKind::LongFormContent | EventKind::FileMetadata => Hsla::new(170.0, 0.6, 0.5, 1.0),
            // System (gray)
            EventKind::RecommendRelay | EventKind::Deletion => Hsla::new(0.0, 0.0, 0.6, 1.0),
            // Custom (muted)
            EventKind::Custom(_) => Hsla::new(0.0, 0.0, 0.5, 1.0),
        }
    }

    /// Icon for the event kind
    pub fn icon(&self) -> &'static str {
        match self {
            EventKind::TextNote => "📝",
            EventKind::Metadata => "👤",
            EventKind::Contacts => "👥",
            EventKind::EncryptedDm => "🔒",
            EventKind::Reaction => "❤",
            EventKind::Repost => "🔄",
            EventKind::ZapReceipt => "⚡",
            EventKind::Issue => "🐛",
            EventKind::Patch => "📋",
            EventKind::PullRequest => "↗",
            EventKind::RepoAnnounce => "📦",
            EventKind::AgentProfile => "🤖",
            EventKind::TrajectorySession => "📊",
            EventKind::DvmTextRequest => "⇢",
            EventKind::DvmTextResult => "⇠",
            EventKind::LongFormContent => "📄",
            EventKind::FileMetadata => "📎",
            _ => "○",
        }
    }
}

/// Badge displaying event kind
pub struct EventKindBadge {
    id: Option<ComponentId>,
    kind: EventKind,
    show_number: bool,
    compact: bool,
}

impl EventKindBadge {
    pub fn new(kind: EventKind) -> Self {
        Self {
            id: None,
            kind,
            show_number: true,
            compact: false,
        }
    }

    pub fn from_raw(kind: u32) -> Self {
        Self::new(EventKind::from_kind(kind))
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn show_number(mut self, show: bool) -> Self {
        self.show_number = show;
        self
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    pub fn kind(&self) -> &EventKind {
        &self.kind
    }
}

impl Component for EventKindBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let color = self.kind.color();
        let bg = Hsla::new(color.h, color.s * 0.2, 0.12, 0.95);

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(bg)
                .with_border(color, 1.0),
        );

        let padding = 6.0;

        if self.compact {
            // Just show icon
            let icon = self.kind.icon();
            let run = cx.text.layout_mono(
                icon,
                Point::new(bounds.origin.x + padding, bounds.origin.y + 4.0),
                theme::font_size::SM,
                color,
            );
            cx.scene.draw_text(run);
        } else {
            // Show label and optionally kind number
            let label = self.kind.label();
            let text_y = bounds.origin.y + (bounds.size.height - theme::font_size::XS) / 2.0;

            let run = cx.text.layout_mono(
                label,
                Point::new(bounds.origin.x + padding, text_y),
                theme::font_size::XS,
                color,
            );
            cx.scene.draw_text(run);

            if self.show_number {
                let kind_text = format!(":{}", self.kind.kind());
                let kind_x =
                    bounds.origin.x + bounds.size.width - padding - kind_text.len() as f32 * 5.5;
                let kind_run = cx.text.layout_mono(
                    &kind_text,
                    Point::new(kind_x, text_y),
                    theme::font_size::XS,
                    Hsla::new(color.h, color.s * 0.5, 0.5, 0.8),
                );
                cx.scene.draw_text(kind_run);
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
            (Some(28.0), Some(24.0))
        } else if self.show_number {
            (Some(90.0), Some(22.0))
        } else {
            (Some(60.0), Some(22.0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_kind_from_raw() {
        assert_eq!(EventKind::from_kind(1), EventKind::TextNote);
        assert_eq!(EventKind::from_kind(0), EventKind::Metadata);
        assert_eq!(EventKind::from_kind(9735), EventKind::ZapReceipt);
        assert_eq!(EventKind::from_kind(99999), EventKind::Custom(99999));
    }

    #[test]
    fn test_event_kind_round_trip() {
        let kind = EventKind::PullRequest;
        assert_eq!(EventKind::from_kind(kind.kind()), kind);
    }
}
