//! Ported from grok-build (Apache-2.0, Copyright 2023-2026 SpaceXAI).
//!
//! Grok Build's working spinner, as its pager draws it beside "Starting
//! session…" and a running turn's activity:
//!
//! - the frames are `braille_spinner_frames` in
//!   `crates/codegen/xai-grok-pager-render/src/glyphs.rs`, with its four
//!   ASCII positions for a console whose font has no braille;
//! - each frame holds `SPINNER_DIVISOR` (4) ticks of the 30 fps animation
//!   clock (`crates/codegen/xai-grok-pager/src/views/turn_status.rs`,
//!   `appearance.animation.fps` = 30), so 4 × 33 ms;
//! - the spinner, its words, and the timer share one style, adapted to
//!   Coder Noir's tertiary content role;
//! - the timer is `format_duration` in
//!   `crates/codegen/xai-grok-pager-render/src/util.rs`.
//!
//! Changes: the frames are a `const` table rather than a function that
//! asks the console, the caller says whether braille can be drawn, and
//! the color goes through this crate's [`Ladder`] depths.

use std::time::Duration;

use ratatui::style::{Modifier, Style};

use crate::ladder::{Colors, Ladder, rgb};

/// The braille frames, in order.
pub const FRAMES: [&str; 8] = [
    "\u{280b}", "\u{2819}", "\u{2839}", "\u{2838}", "\u{283c}", "\u{2834}", "\u{2826}", "\u{2827}",
];

/// The positions a console without braille draws instead.
pub const FALLBACK: [&str; 4] = ["|", "/", "-", "\\"];

/// One tick of the animation clock: 30 a second.
pub const TICK: Duration = Duration::from_millis(1000 / 30);

/// How many ticks each frame holds.
pub const DIVISOR: u64 = 4;

/// The shared tertiary content role for the spinner, its words, and its timer.
pub const GRAY_DIM: u32 = coder_ui::coder_noir::CONTENT_TERTIARY;

/// The frame at animation tick `tick`.
#[must_use]
pub fn frame(tick: u64) -> &'static str {
    FRAMES[usize::try_from((tick / DIVISOR) % FRAMES.len() as u64).unwrap_or(0)]
}

/// The frame at animation tick `tick` on a console without braille.
#[must_use]
pub fn fallback(tick: u64) -> &'static str {
    FALLBACK[usize::try_from((tick / DIVISOR) % FALLBACK.len() as u64).unwrap_or(0)]
}

/// Whether tick `tick` shows a new frame: a screen that draws only for
/// the spinner draws then.
#[must_use]
pub fn turns(tick: u64) -> bool {
    tick % DIVISOR == 0
}

/// The time `ticks` animation ticks take.
#[must_use]
pub fn elapsed(ticks: u64) -> Duration {
    TICK * u32::try_from(ticks).unwrap_or(u32::MAX)
}

/// The timer beside the spinner: "0.5s", "5.2s", "10s", "1m20s", "1h2m".
#[must_use]
pub fn timer(elapsed: Duration) -> String {
    let total = elapsed.as_secs();
    if total < 10 {
        return format!("{:.1}s", elapsed.as_secs_f64());
    }
    if total < 60 {
        return format!("{total}s");
    }
    let minutes = total / 60;
    let seconds = total % 60;
    if minutes < 60 {
        return format!("{minutes}m{seconds}s");
    }
    format!("{}h{}m", minutes / 60, minutes % 60)
}

/// The spinner's style at this terminal: `gray_dim`, its nearest 256-color
/// entry, or dim when there is no color.
#[must_use]
pub fn style(ladder: Ladder) -> Style {
    match ladder.colors() {
        Colors::True => Style::new().fg(rgb(GRAY_DIM)),
        Colors::Indexed => Style::new().fg(code_highlight::grok::color::quantize_color(
            rgb(GRAY_DIM),
            code_highlight::grok::ColorLevel::Ansi256,
        )),
        Colors::None => Style::new().add_modifier(Modifier::DIM),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn each_frame_holds_four_ticks_of_the_30_fps_clock() {
        let walked: Vec<&str> = (0..36).step_by(4).map(frame).collect();
        assert_eq!(walked, vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠋"]);
        assert_eq!(frame(3), "⠋");
        assert!(turns(8) && !turns(9));
        assert_eq!(TICK, Duration::from_millis(33));
        assert_eq!(fallback(4), "/");
    }

    #[test]
    fn the_timer_reads_as_grok_builds_does() {
        assert_eq!(timer(Duration::from_millis(500)), "0.5s");
        assert_eq!(timer(Duration::from_secs_f64(5.2)), "5.2s");
        assert_eq!(timer(Duration::from_secs_f64(9.9)), "9.9s");
        assert_eq!(timer(Duration::from_secs(10)), "10s");
        assert_eq!(timer(Duration::from_secs(80)), "1m20s");
        assert_eq!(timer(Duration::from_secs(3725)), "1h2m");
        assert_eq!(elapsed(30), Duration::from_millis(990));
    }

    #[test]
    fn the_spinner_uses_the_shared_tertiary_role_at_each_depth() {
        assert_eq!(style(Ladder::new(Colors::True)).fg, Some(rgb(GRAY_DIM)));
        assert_eq!(
            style(Ladder::new(Colors::Indexed)).fg,
            Some(Color::Indexed(241))
        );
        assert_eq!(
            style(Ladder::new(Colors::None)),
            Style::new().add_modifier(Modifier::DIM)
        );
    }
}
