use super::*;

pub(super) fn chat_thread_action_grid_bounds(content_bounds: Bounds, index: usize) -> Bounds {
    let source = chat_thread_filter_source_button_bounds(content_bounds);
    let row = index / 2;
    let col = index % 2;
    Bounds::new(
        source.origin.x
            + col as f32 * (CHAT_THREAD_ACTION_BUTTON_WIDTH + CHAT_THREAD_ACTION_BUTTON_GAP),
        source.max_y()
            + 8.0
            + row as f32 * (CHAT_THREAD_ACTION_BUTTON_HEIGHT + CHAT_THREAD_ACTION_BUTTON_GAP),
        CHAT_THREAD_ACTION_BUTTON_WIDTH,
        CHAT_THREAD_ACTION_BUTTON_HEIGHT,
    )
}

pub(super) fn chat_thread_rail_controls_bottom(
    content_bounds: Bounds,
    thread_tools_expanded: bool,
) -> f32 {
    if thread_tools_expanded {
        chat_thread_action_unsubscribe_button_bounds(content_bounds).max_y() + 10.0
    } else {
        chat_thread_filter_provider_button_bounds(content_bounds).max_y() + 10.0
    }
}

pub(super) fn codex_action_button_bounds(
    content_bounds: Bounds,
    row: usize,
    col: usize,
    columns: usize,
) -> Bounds {
    let columns = columns.max(1);
    let gap = JOB_INBOX_BUTTON_GAP;
    let usable_width =
        (content_bounds.size.width - CHAT_PAD * 2.0 - gap * (columns as f32 - 1.0)).max(220.0);
    let width = (usable_width / columns as f32).clamp(120.0, 220.0);
    let x = content_bounds.origin.x + CHAT_PAD + col as f32 * (width + gap);
    let y = content_bounds.origin.y + CHAT_PAD + row as f32 * (JOB_INBOX_BUTTON_HEIGHT + gap);
    Bounds::new(x, y, width, JOB_INBOX_BUTTON_HEIGHT)
}

pub(super) fn codex_labs_button_bounds(content_bounds: Bounds, row: usize, col: usize) -> Bounds {
    codex_action_button_bounds(content_bounds, row, col, 3)
}

pub(super) fn codex_diagnostics_button_bounds(content_bounds: Bounds, col: usize) -> Bounds {
    codex_action_button_bounds(content_bounds, 0, col, 3)
}

pub(super) fn credentials_button_bounds(content_bounds: Bounds, row: usize, col: usize) -> Bounds {
    let top = credentials_name_input_bounds(content_bounds).max_y()
        + 10.0
        + row as f32 * (CREDENTIALS_BUTTON_HEIGHT + CREDENTIALS_BUTTON_GAP);
    Bounds::new(
        content_bounds.origin.x
            + CHAT_PAD
            + col as f32 * (CREDENTIALS_BUTTON_WIDTH + CREDENTIALS_BUTTON_GAP),
        top,
        CREDENTIALS_BUTTON_WIDTH,
        CREDENTIALS_BUTTON_HEIGHT,
    )
}

pub(super) fn nostr_button_bounds(content_bounds: Bounds) -> (Bounds, Bounds, Bounds) {
    let gap = 8.0;
    let start_x = content_bounds.origin.x + 12.0;
    let y = content_bounds.origin.y + 12.0;
    let available_width = (content_bounds.size.width - 24.0).max(300.0);
    let utility_width = ((available_width * 0.34) - gap).clamp(110.0, 168.0);
    let regenerate_width = (available_width - utility_width * 2.0 - gap * 2.0).max(118.0);

    let regenerate_bounds = Bounds::new(start_x, y, regenerate_width, 30.0);
    let reveal_bounds = Bounds::new(
        regenerate_bounds.origin.x + regenerate_width + gap,
        y,
        utility_width,
        30.0,
    );
    let copy_bounds = Bounds::new(
        reveal_bounds.origin.x + utility_width + gap,
        y,
        utility_width,
        30.0,
    );

    (regenerate_bounds, reveal_bounds, copy_bounds)
}

pub(super) fn pane_title_bounds(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        PANE_TITLE_HEIGHT,
    )
}

pub(super) fn cursor_icon_for_resize_edge(edge: ResizeEdge) -> CursorIcon {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorIcon::NsResize,
        ResizeEdge::Left | ResizeEdge::Right => CursorIcon::EwResize,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorIcon::NwseResize,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorIcon::NeswResize,
        ResizeEdge::None => CursorIcon::Default,
    }
}

pub(super) fn clamp_bounds_to_window(
    bounds: Bounds,
    window_size: Size,
    sidebar_width: f32,
    min_size: Size,
) -> Bounds {
    // Keep panes within the main canvas area and out from under the right sidebar.
    let reserved_sidebar = sidebar_width.max(0.0);
    let available_width = window_size.width - reserved_sidebar - PANE_MARGIN * 2.0;
    let top_inset = PANE_TOP_SAFE_INSET;
    let min_width = min_size.width.max(PANE_MIN_WIDTH);
    let max_width = available_width.max(min_width);
    let width = bounds.size.width.clamp(min_width, max_width);

    let min_height = min_size.height.max(PANE_MIN_HEIGHT);
    let max_height = (window_size.height - top_inset - PANE_BOTTOM_RESERVED).max(min_height);
    let height = bounds.size.height.clamp(min_height, max_height);

    let max_x = (window_size.width - reserved_sidebar - width - PANE_MARGIN).max(PANE_MARGIN);
    let max_y = (window_size.height - height - PANE_BOTTOM_RESERVED).max(top_inset);

    let x = bounds.origin.x.clamp(PANE_MARGIN, max_x);
    let y = bounds.origin.y.clamp(top_inset, max_y);

    Bounds::new(x, y, width, height)
}
