use std::borrow::Cow;

use crate::{Bounds, Hsla, PaintContext, Point, Quad, theme};

use super::panel;

#[derive(Clone)]
pub struct EventFeedRow<'a> {
    pub label: Cow<'a, str>,
    pub detail: Cow<'a, str>,
    pub color: Hsla,
}

pub fn paint_event_feed_body(
    bounds: Bounds,
    accent: Hsla,
    phase: f32,
    empty_state: &str,
    events: &[EventFeedRow<'_>],
    paint: &mut PaintContext,
) {
    panel::paint_texture(bounds, accent, phase, paint);
    if events.is_empty() {
        paint.scene.draw_text(paint.text.layout(
            empty_state,
            Point::new(bounds.origin.x + 14.0, bounds.origin.y + 36.0),
            11.0,
            theme::text::MUTED,
        ));
        return;
    }

    let rail_x = bounds.origin.x + 20.0;
    paint.scene.draw_quad(
        Quad::new(Bounds::new(
            rail_x,
            bounds.origin.y + 36.0,
            1.0,
            (bounds.size.height - 56.0).max(24.0),
        ))
        .with_background(accent.with_alpha(0.16)),
    );

    let mut y = bounds.origin.y + 38.0;
    let remaining_events = ((bounds.max_y() - y - 16.0) / 44.0).floor().max(1.0) as usize;
    for (index, event) in events.iter().take(remaining_events.min(6)).enumerate() {
        let pulse = if index == 0 { 0.12 + phase * 0.12 } else { 0.0 };
        paint.scene.draw_quad(
            Quad::new(Bounds::new(rail_x - 4.0, y + 3.0, 9.0, 9.0))
                .with_background(event.color.with_alpha(0.74 + pulse))
                .with_corner_radius(4.5),
        );
        paint.scene.draw_text(paint.text.layout_mono(
            event.label.as_ref(),
            Point::new(bounds.origin.x + 34.0, y),
            10.0,
            event.color.with_alpha(0.94),
        ));
        let detail_lines = split_text_for_display(event.detail.as_ref(), 46);
        let mut detail_y = y;
        for line in detail_lines.iter().take(2) {
            paint.scene.draw_text(paint.text.layout(
                line.as_str(),
                Point::new(bounds.origin.x + 76.0, detail_y),
                10.0,
                theme::text::PRIMARY,
            ));
            detail_y += 16.0;
        }
        y += 44.0;
    }
}

fn split_text_for_display(text: &str, chunk_len: usize) -> Vec<String> {
    if text.trim().is_empty() {
        return vec![String::new()];
    }

    let mut chunks = Vec::new();
    let chunk_len = chunk_len.max(1);
    for line in text.lines() {
        let line_chars = line.chars().collect::<Vec<_>>();
        if line_chars.is_empty() {
            chunks.push(String::new());
            continue;
        }
        chunks.extend(
            line_chars
                .chunks(chunk_len)
                .map(|chunk| chunk.iter().collect::<String>()),
        );
    }
    if text.ends_with('\n') {
        chunks.push(String::new());
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::split_text_for_display;

    #[test]
    fn split_text_for_display_preserves_empty_lines() {
        let lines = split_text_for_display("alpha\n\nbeta", 8);
        assert_eq!(lines, vec!["alpha", "", "beta"]);
    }

    #[test]
    fn split_text_for_display_wraps_long_lines() {
        let lines = split_text_for_display("abcdefgh", 3);
        assert_eq!(lines, vec!["abc", "def", "gh"]);
    }
}
