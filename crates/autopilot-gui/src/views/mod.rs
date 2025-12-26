pub mod chat;
pub mod context;
pub mod dashboard;
pub mod diff_view;
pub mod parallel;
pub mod receipts;
pub mod replay;
pub mod timeline;

pub use chat::ChatView;
pub use context::ContextView;
pub use dashboard::DashboardView;
pub use diff_view::DiffView;
pub use parallel::ParallelView;
pub use receipts::ReceiptsPanel;
pub use replay::ReplayView;
pub use timeline::{TimelineScrubber, TimelineState};

use wgpui::PaintContext;

pub(crate) fn fit_text(
    cx: &mut PaintContext,
    text: &str,
    font_size: f32,
    max_width: f32,
) -> String {
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

    let truncated = text.chars().take(max_chars - ellipsis.len()).collect::<String>();
    format!("{}{}", truncated, ellipsis)
}
