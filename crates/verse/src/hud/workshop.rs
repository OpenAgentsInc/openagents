//! The workshop agent's panel: a fixed sheet anchored to the bottom of the
//! window, standing on the hotbar (`docs/terminal/design-principles.md`). It does not float,
//! move, or animate; its rows are the status row, the transcript, a
//! pending proposal, the input line, and the key strip, all ASCII.

use super::{Rect, amber, field};
use crate::ui::{Atlas, UiBatch};
use coder_ui::theme::Intensity;

/// The rows the panel holds.
pub const WORKSHOP_ROWS: usize = 12;

/// How many characters fit across the panel in a window `size` wide.
#[must_use]
pub fn workshop_cols(atlas: &Atlas, size: [f32; 2], scale: f32) -> usize {
    let inner = size[0] - 2.0 * 12.0 * scale - 2.0 * 8.0 * scale;
    (inner / atlas.advance.max(1.0)).floor().max(20.0) as usize
}

/// Draws `rows` in the panel, `bottom` pixels over the window's bottom
/// edge, and returns where it is.
#[must_use]
pub fn workshop_panel(
    ui: &mut UiBatch,
    atlas: &Atlas,
    size: [f32; 2],
    scale: f32,
    bottom: f32,
    rows: &[(String, Intensity)],
) -> Rect {
    let s = scale;
    let margin = 12.0 * s;
    let pad = 8.0 * s;
    let height = rows.len() as f32 * atlas.line + 2.0 * pad;
    let rect = Rect {
        x: margin,
        y: (size[1] - bottom - margin - height).max(0.0),
        w: (size[0] - 2.0 * margin).max(1.0),
        h: height,
    };
    ui.rect(atlas, rect.x, rect.y, rect.w, rect.h, field(0.96));
    ui.frame(
        atlas,
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        s.max(1.0),
        amber(Intensity::Half, 1.0),
    );
    for (i, (text, tone)) in rows.iter().enumerate() {
        ui.text(
            atlas,
            rect.x + pad,
            rect.y + pad + i as f32 * atlas.line,
            text,
            amber(*tone, 1.0),
        );
    }
    rect
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_is_anchored_over_the_bottom_edge() {
        let atlas = Atlas::new(14.0);
        let size = [1280.0, 800.0];
        let rows: Vec<(String, Intensity)> = (0..WORKSHOP_ROWS)
            .map(|i| (format!("row {i}"), Intensity::Full))
            .collect();
        let mut ui = UiBatch::default();
        let rect = workshop_panel(&mut ui, &atlas, size, 1.0, 60.0, &rows);
        assert!((rect.y + rect.h - (size[1] - 72.0)).abs() < 0.5);
        assert!((rect.x - 12.0).abs() < 0.5 && (rect.x + rect.w - (size[0] - 12.0)).abs() < 0.5);
        assert!(!ui.vertices.is_empty());
        assert!(workshop_cols(&atlas, size, 1.0) * atlas.advance as usize <= 1280);
    }
}
