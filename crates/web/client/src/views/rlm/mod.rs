mod demo;
mod detail;
mod list;

pub(crate) use demo::{
    build_rlm_demo_page, handle_rlm_demo_click, handle_rlm_demo_keydown,
    handle_rlm_demo_mouse_move, handle_rlm_demo_scroll,
};
pub(crate) use detail::{
    build_rlm_detail_page, handle_rlm_detail_click, handle_rlm_detail_mouse_move,
    handle_rlm_detail_scroll,
};
pub(crate) use list::{
    build_rlm_list_page, handle_rlm_list_click, handle_rlm_list_mouse_move,
    handle_rlm_list_scroll,
};

use js_sys::Date;
use wgpui::TextSystem;
use wgpui::Hsla;

// ============================================================================
// V2 Color Palette (from spec)
// ============================================================================

// Background colors
pub(crate) fn bg_dark() -> Hsla {
    Hsla::from_hex(0x08090a)
}

pub(crate) fn bg_panel() -> Hsla {
    Hsla::from_hex(0x0d0f11)
}

pub(crate) fn border_color() -> Hsla {
    Hsla::from_hex(0x1d2328)
}

// Text colors
pub(crate) fn text_primary() -> Hsla {
    Hsla::from_hex(0xf7f8f8)
}

pub(crate) fn text_muted() -> Hsla {
    Hsla::from_hex(0x9aa4ad)
}

// State colors
pub(crate) fn state_pending() -> Hsla {
    Hsla::from_hex(0x3a424a)
}

pub(crate) fn state_active() -> Hsla {
    Hsla::from_hex(0xe6b450)
}

pub(crate) fn state_complete() -> Hsla {
    Hsla::from_hex(0x23d18b)
}

pub(crate) fn state_error() -> Hsla {
    Hsla::from_hex(0xf44747)
}

// ============================================================================
// V2 Typography (from spec)
// ============================================================================

pub(crate) const FONT_TITLE: f32 = 20.0;
pub(crate) const FONT_HEADER: f32 = 14.0;
pub(crate) const FONT_BODY: f32 = 13.0;
pub(crate) const FONT_TABLE: f32 = 13.0;
pub(crate) const FONT_SMALL: f32 = 12.0;

// ============================================================================
// Shared helpers
// ============================================================================

pub(crate) fn wrap_text(
    text_system: &mut TextSystem,
    text: &str,
    max_width: f32,
    font_size: f32,
) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let words: Vec<&str> = paragraph.split_whitespace().collect();
        if words.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current_line = String::new();
        for word in words {
            let test = if current_line.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", current_line, word)
            };
            let width = text_system.measure(&test, font_size);
            if width > max_width && !current_line.is_empty() {
                lines.push(current_line);
                current_line = word.to_string();
            } else {
                current_line = test;
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    lines
}

pub(crate) fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

pub(crate) fn format_duration_ms(duration_ms: i64) -> String {
    let duration_ms = duration_ms.max(0);
    if duration_ms < 1000 {
        return format!("{}ms", duration_ms);
    }
    let total_seconds = duration_ms / 1000;
    if total_seconds < 60 {
        return format!("{}s", total_seconds);
    }
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    if minutes < 60 {
        return format!("{}m {}s", minutes, seconds);
    }
    let hours = minutes / 60;
    let minutes = minutes % 60;
    format!("{}h {}m", hours, minutes)
}

pub(crate) fn format_time_ago(epoch_seconds: i64) -> String {
    let now_ms = Date::now() as i64;
    let now_seconds = now_ms / 1000;
    let delta = (now_seconds - epoch_seconds).max(0);
    if delta < 60 {
        return format!("{}s ago", delta);
    }
    if delta < 3600 {
        return format!("{}m ago", delta / 60);
    }
    if delta < 86400 {
        return format!("{}h ago", delta / 3600);
    }
    format!("{}d ago", delta / 86400)
}
