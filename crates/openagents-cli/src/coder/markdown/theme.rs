//! Coder's markdown palette.
//!
//! The engine under `crate::coder::markdown` is xAI's; the palette is ours and does
//! not move. Coder paints one amber (`#FFB000`) on one near-black
//! (`#080600`) and separates markdown elements by *effect* — bold, dim,
//! italic, underline — never by hue. Every style below therefore carries the
//! same foreground colour; only the effects differ.
//!
//! Keep it that way. A port that introduces a second hue has changed what
//! Coder looks like, which is the one thing this port must not do.

use anstyle::{Color as AnsiStyleColor, RgbColor, Style};
use ratatui::style::Color;

use super::style::MarkdownStyle;
use super::syntax::Syntect;

/// Coder's single foreground amber.
pub const TEXT_COLOR: Color = Color::Rgb(255, 176, 0);

/// A dimmer amber for notices, tokens, and other secondary text.
///
/// It is the halfway point between `TEXT_COLOR` and the background, so it
/// reads at half the intensity of the main text.
pub const DIM_TEXT_COLOR: Color = Color::Rgb(131, 91, 0);

/// Amber composited at 75% opacity over [`BACKGROUND_COLOR`] for user turns.
pub const USER_TEXT_COLOR: Color = Color::Rgb(193, 134, 0);

/// Coder's single background.
pub const BACKGROUND_COLOR: Color = Color::Rgb(8, 6, 0);

const AMBER: AnsiStyleColor = AnsiStyleColor::Rgb(RgbColor(255, 176, 0));
const NEAR_BLACK: AnsiStyleColor = AnsiStyleColor::Rgb(RgbColor(8, 6, 0));

/// Amber foreground with no effects.
const fn amber() -> Style {
    Style::new().fg_color(Some(AMBER))
}

/// The `MarkdownStyle` Coder renders with.
///
/// `_outer` styles are `hidden()`, which is how the engine suppresses
/// syntax markers (`**`, `` ` ``, `#`) in pretty mode. `hidden()` is a
/// semantic marker consumed by the renderer, not an escape hatch that drops
/// content: the *inner* text always survives.
pub fn coder_markdown_style() -> MarkdownStyle {
    coder_markdown_style_base().adapt()
}

fn coder_markdown_style_base() -> MarkdownStyle {
    let hidden_marker = amber().dimmed().hidden();
    MarkdownStyle {
        // Headings step down by weight, not by colour.
        heading_inner: [
            amber().bold().underline(),
            amber().bold(),
            amber().bold(),
            amber().bold(),
            amber(),
            amber().dimmed(),
        ],
        heading_outer: [hidden_marker; 6],
        strong_inner: amber().bold(),
        strong_outer: hidden_marker,
        emphasis_inner: amber().italic(),
        emphasis_outer: hidden_marker,
        strikethrough_inner: amber().strikethrough(),
        strikethrough_outer: hidden_marker,
        inline_code_inner: amber().bold(),
        inline_code_outer: hidden_marker,
        blockquote_outer: amber().dimmed(),
        task_checked: amber(),
        task_unchecked: amber().dimmed(),
        list_item: amber().dimmed(),
        rule: amber().dimmed(),
        // Visible, unlike the other `_outer` markers: pretty mode prints the
        // destination after the link text, and the parentheses around it are
        // what keep `the forge (https://…)` from reading as one run-on word.
        link_outer: amber().dimmed(),
        link_text: amber().underline(),
        link_url: amber().dimmed(),
        link_title: amber().dimmed(),
        code_outer: hidden_marker,
        // Hidden, matching grok-build's own theme. The info string is markup
        // (like `**`), and its meaning reaches the screen as syntax
        // highlighting. The block body is never hidden — see
        // `an_unknown_language_fence_still_shows_its_body`.
        code_language: hidden_marker,
        code_untagged: amber(),
        code_background: Style::new().bg_color(Some(NEAR_BLACK)),
        table_outer: amber().dimmed(),
        text: amber(),
        math: amber().italic(),
    }
}

/// The syntect theme used for fenced code blocks.
///
/// `tokyo-night` is the theme grok-build ships; its hues are then flattened to
/// Coder's amber by [`amberize`] before the spans reach the screen, so
/// highlighting shows up as weight and dimming rather than colour.
pub fn syntect() -> &'static Syntect {
    use std::sync::OnceLock;
    static SYNTECT: OnceLock<Syntect> = OnceLock::new();
    SYNTECT.get_or_init(|| Syntect::new(include_bytes!("assets/tokyo-night.tmTheme")))
}

/// Collapse every foreground to Coder's amber and every background to
/// Coder's near-black, preserving the effect bits.
///
/// Syntect emits real RGB colours for code spans and the engine emits themed
/// colours for prose. Both are run through this before display so a fenced
/// `rust` block still reads as amber-on-black, exactly as it did before this
/// port. Relative luminance of the incoming foreground is mapped onto amber's
/// `DIM` bit so highlighting remains legible as *contrast* without ever
/// introducing a second hue.
pub fn amberize(lines: &mut [ratatui::text::Line<'static>]) {
    use ratatui::style::Modifier;
    for line in lines.iter_mut() {
        line.style = line.style.fg(TEXT_COLOR).bg(BACKGROUND_COLOR);
        for span in &mut line.spans {
            let dim_from_luma = span
                .style
                .fg
                .and_then(luminance)
                .is_some_and(|luma| luma < 0.35);
            span.style = span.style.fg(TEXT_COLOR).bg(BACKGROUND_COLOR);
            if dim_from_luma {
                span.style = span.style.add_modifier(Modifier::DIM);
            }
        }
    }
}

/// Relative luminance in `0.0..=1.0` for the colours we can measure.
///
/// Indexed and named ANSI colours have no fixed RGB value, so they return
/// `None` rather than a guess.
fn luminance(color: Color) -> Option<f32> {
    let (r, g, b) = match color {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => return None,
    };
    Some((0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Modifier;
    use ratatui::text::{Line, Span};

    #[test]
    fn every_style_uses_the_one_amber_foreground() {
        let s = coder_markdown_style_base();
        let expected = Some(AMBER);
        let mut checked = 0;
        for style in s.heading_inner.iter().chain(s.heading_outer.iter()).chain([
            &s.strong_inner,
            &s.strong_outer,
            &s.emphasis_inner,
            &s.emphasis_outer,
            &s.strikethrough_inner,
            &s.strikethrough_outer,
            &s.inline_code_inner,
            &s.inline_code_outer,
            &s.blockquote_outer,
            &s.task_checked,
            &s.task_unchecked,
            &s.list_item,
            &s.rule,
            &s.link_outer,
            &s.link_text,
            &s.link_url,
            &s.link_title,
            &s.code_outer,
            &s.code_language,
            &s.code_untagged,
            &s.table_outer,
            &s.text,
            &s.math,
        ]) {
            assert_eq!(
                style.get_fg_color(),
                expected,
                "a markdown style drifted off Coder's amber"
            );
            checked += 1;
        }
        assert_eq!(checked, 35, "style coverage changed; update this assertion");
    }

    #[test]
    fn code_background_is_the_one_background() {
        assert_eq!(
            coder_markdown_style_base().code_background.get_bg_color(),
            Some(NEAR_BLACK)
        );
    }

    #[test]
    fn amberize_flattens_foreign_hues_but_keeps_effects() {
        let mut lines = vec![Line::from(vec![Span::styled(
            "fn",
            ratatui::style::Style::default()
                .fg(Color::Rgb(0x7a, 0xa2, 0xf7))
                .add_modifier(Modifier::BOLD),
        )])];
        amberize(&mut lines);
        let span = &lines[0].spans[0];
        assert_eq!(span.style.fg, Some(TEXT_COLOR));
        assert_eq!(span.style.bg, Some(BACKGROUND_COLOR));
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn amberize_maps_dark_source_colors_onto_dim() {
        let mut lines = vec![Line::from(vec![
            Span::styled(
                "// comment",
                ratatui::style::Style::default().fg(Color::Rgb(0x40, 0x40, 0x40)),
            ),
            Span::styled(
                "bright",
                ratatui::style::Style::default().fg(Color::Rgb(0xe0, 0xe0, 0xe0)),
            ),
        ])];
        amberize(&mut lines);
        assert!(lines[0].spans[0].style.add_modifier.contains(Modifier::DIM));
        assert!(!lines[0].spans[1].style.add_modifier.contains(Modifier::DIM));
    }
}
