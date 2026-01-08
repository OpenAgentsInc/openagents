//! APM leaderboard organism for ranking sessions by APM.
//!
//! Shows a ranked list of sessions with their APM scores and tiers.

use crate::components::atoms::{ApmLevel, SessionStatus};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// Leaderboard entry data
#[derive(Debug, Clone)]
pub struct LeaderboardEntry {
    pub id: String,
    pub title: String,
    pub apm: f32,
    pub level: ApmLevel,
    pub status: SessionStatus,
    pub messages: u32,
    pub tool_calls: u32,
}

impl LeaderboardEntry {
    pub fn new(id: impl Into<String>, title: impl Into<String>, apm: f32) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            apm,
            level: ApmLevel::from_apm(apm),
            status: SessionStatus::Completed,
            messages: 0,
            tool_calls: 0,
        }
    }

    pub fn status(mut self, status: SessionStatus) -> Self {
        self.status = status;
        self
    }

    pub fn messages(mut self, count: u32) -> Self {
        self.messages = count;
        self
    }

    pub fn tool_calls(mut self, count: u32) -> Self {
        self.tool_calls = count;
        self
    }
}

/// APM leaderboard showing ranked sessions
pub struct ApmLeaderboard {
    id: Option<ComponentId>,
    entries: Vec<LeaderboardEntry>,
    title: String,
    selected_index: Option<usize>,
    hovered_index: Option<usize>,
    on_select: Option<Box<dyn FnMut(String)>>,
}

impl ApmLeaderboard {
    pub fn new() -> Self {
        Self {
            id: None,
            entries: Vec::new(),
            title: "APM Leaderboard".to_string(),
            selected_index: None,
            hovered_index: None,
            on_select: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn entries(mut self, entries: Vec<LeaderboardEntry>) -> Self {
        self.entries = entries;
        self
    }

    pub fn push_entry(mut self, entry: LeaderboardEntry) -> Self {
        self.entries.push(entry);
        self
    }

    pub fn on_select<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_select = Some(Box::new(f));
        self
    }

    fn rank_color(rank: usize) -> Hsla {
        match rank {
            0 => Hsla::new(45.0, 0.9, 0.5, 1.0),  // Gold
            1 => Hsla::new(0.0, 0.0, 0.75, 1.0),  // Silver
            2 => Hsla::new(30.0, 0.6, 0.45, 1.0), // Bronze
            _ => theme::text::MUTED,
        }
    }

    fn row_bounds(&self, bounds: &Bounds, index: usize) -> Bounds {
        let header_height = 36.0;
        let row_height = 36.0;
        let y = bounds.origin.y + header_height + index as f32 * row_height;
        Bounds::new(bounds.origin.x, y, bounds.size.width, row_height)
    }
}

impl Default for ApmLeaderboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ApmLeaderboard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let padding = 12.0;
        let header_height = 36.0;

        // Title
        let title_run = cx.text.layout(
            &self.title,
            Point::new(
                bounds.origin.x + padding,
                bounds.origin.y + (header_height - theme::font_size::SM) / 2.0,
            ),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        // Column headers
        let col_rank_x = bounds.origin.x + padding;
        let col_apm_x = bounds.origin.x + 50.0;
        let col_title_x = bounds.origin.x + 130.0;
        let col_stats_x = bounds.origin.x + bounds.size.width - padding - 100.0;

        let header_y = bounds.origin.y + header_height - 4.0;

        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                header_y + 4.0,
                bounds.size.width,
                1.0,
            ))
            .with_background(theme::border::DEFAULT),
        );

        // Entries
        for (idx, entry) in self.entries.iter().enumerate() {
            let row = self.row_bounds(&bounds, idx);

            // Highlight selected/hovered
            if self.selected_index == Some(idx) {
                cx.scene.draw_quad(
                    Quad::new(row).with_background(theme::accent::PRIMARY.with_alpha(0.2)),
                );
            } else if self.hovered_index == Some(idx) {
                cx.scene
                    .draw_quad(Quad::new(row).with_background(theme::bg::HOVER));
            }

            let text_y = row.origin.y + (row.size.height - theme::font_size::SM) / 2.0;

            // Rank
            let rank_text = format!("#{}", idx + 1);
            let rank_run = cx.text.layout(
                &rank_text,
                Point::new(col_rank_x, text_y),
                theme::font_size::SM,
                Self::rank_color(idx),
            );
            cx.scene.draw_text(rank_run);

            // APM with tier color
            let apm_text = format!("{:.0}", entry.apm);
            let apm_run = cx.text.layout(
                &apm_text,
                Point::new(col_apm_x, text_y),
                theme::font_size::SM,
                entry.level.color(),
            );
            cx.scene.draw_text(apm_run);

            // Tier badge
            let tier_text = entry.level.label();
            let tier_run = cx.text.layout(
                tier_text,
                Point::new(col_apm_x + 40.0, text_y),
                theme::font_size::XS,
                entry.level.color(),
            );
            cx.scene.draw_text(tier_run);

            // Title
            let title = if entry.title.len() > 25 {
                format!("{}...", &entry.title[..22])
            } else {
                entry.title.clone()
            };
            let title_run = cx.text.layout(
                &title,
                Point::new(col_title_x, text_y),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(title_run);

            // Stats
            let stats = format!("{} msg / {} tools", entry.messages, entry.tool_calls);
            let stats_run = cx.text.layout(
                &stats,
                Point::new(col_stats_x, text_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(stats_run);
        }

        // Empty state
        if self.entries.is_empty() {
            let empty_y = bounds.origin.y + header_height + 24.0;
            let empty_run = cx.text.layout(
                "No sessions yet",
                Point::new(bounds.origin.x + bounds.size.width / 2.0 - 50.0, empty_y),
                theme::font_size::SM,
                theme::text::DISABLED,
            );
            cx.scene.draw_text(empty_run);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_hovered = self.hovered_index;
                self.hovered_index = None;

                for idx in 0..self.entries.len() {
                    let row = self.row_bounds(&bounds, idx);
                    if row.contains(point) {
                        self.hovered_index = Some(idx);
                        break;
                    }
                }

                if was_hovered != self.hovered_index {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);
                    for idx in 0..self.entries.len() {
                        let row = self.row_bounds(&bounds, idx);
                        if row.contains(point) {
                            self.selected_index = Some(idx);
                            if let Some(callback) = &mut self.on_select {
                                callback(self.entries[idx].id.clone());
                            }
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
        let header_height = 36.0;
        let row_height = 36.0;
        let min_rows = 3.0;
        let height =
            header_height + (self.entries.len().max(min_rows as usize) as f32) * row_height;
        (None, Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leaderboard_entry() {
        let entry = LeaderboardEntry::new("sess-1", "Build feature", 85.0)
            .messages(100)
            .tool_calls(50)
            .status(SessionStatus::Completed);

        assert_eq!(entry.id, "sess-1");
        assert_eq!(entry.apm, 85.0);
        assert_eq!(entry.messages, 100);
    }

    #[test]
    fn test_apm_leaderboard() {
        let leaderboard = ApmLeaderboard::new()
            .title("Today's Top Sessions")
            .push_entry(LeaderboardEntry::new("1", "Session A", 95.0))
            .push_entry(LeaderboardEntry::new("2", "Session B", 80.0));

        assert_eq!(leaderboard.entries.len(), 2);
        assert_eq!(leaderboard.title, "Today's Top Sessions");
    }

    #[test]
    fn test_rank_colors() {
        let gold = ApmLeaderboard::rank_color(0);
        assert!(gold.h > 40.0 && gold.h < 50.0); // Gold hue

        let silver = ApmLeaderboard::rank_color(1);
        assert!(silver.s < 0.1); // Desaturated for silver

        let muted = ApmLeaderboard::rank_color(10);
        assert!(muted.s < 0.5); // Muted for other ranks
    }
}
