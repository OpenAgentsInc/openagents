use super::*;

pub(super) fn normalize_pane_rect(rect: PaneRect) -> PaneRect {
    let mut width = rect.width.max(PANE_MIN_WIDTH);
    let mut height = rect.height.max(PANE_MIN_HEIGHT);
    if width.is_nan() || width <= 0.0 {
        width = PANE_MIN_WIDTH;
    }
    if height.is_nan() || height <= 0.0 {
        height = PANE_MIN_HEIGHT;
    }
    PaneRect {
        x: rect.x,
        y: rect.y,
        width,
        height,
    }
}

pub(super) fn calculate_new_pane_position(
    last: Option<PaneRect>,
    screen: Size,
    width: f32,
    height: f32,
) -> PaneRect {
    if let Some(last) = last {
        let mut x = last.x + PANE_OFFSET;
        let mut y = last.y + PANE_OFFSET;
        if x + width > screen.width - PANE_MARGIN {
            x = PANE_MARGIN;
        }
        if y + height > screen.height - PANE_MARGIN {
            y = PANE_MARGIN;
        }
        PaneRect {
            x,
            y,
            width,
            height,
        }
    } else {
        PaneRect {
            x: (screen.width - width) * 0.5,
            y: (screen.height - height) * 0.3,
            width,
            height,
        }
    }
}
