use wgpui::components::hud::{PaneFrame, ResizeEdge};
use wgpui::{Bounds, Component, InputEvent, Modifiers, MouseButton, Point, Size};
use winit::window::CursorIcon;

use crate::app_state::{DesktopPane, PaneDragMode, RenderState};
use crate::hotbar::{HOTBAR_FLOAT_GAP, HOTBAR_HEIGHT};
use crate::render::logical_size;

const PANE_DEFAULT_WIDTH: f32 = 420.0;
const PANE_DEFAULT_HEIGHT: f32 = 280.0;
pub const PANE_TITLE_HEIGHT: f32 = 28.0;
pub const PANE_MIN_WIDTH: f32 = 220.0;
pub const PANE_MIN_HEIGHT: f32 = 140.0;
const PANE_MARGIN: f32 = 18.0;
const PANE_CASCADE_X: f32 = 26.0;
const PANE_CASCADE_Y: f32 = 22.0;
const PANE_BOTTOM_RESERVED: f32 = HOTBAR_HEIGHT + HOTBAR_FLOAT_GAP + PANE_MARGIN;

pub fn create_empty_pane(state: &mut RenderState) {
    let id = state.next_pane_id;
    state.next_pane_id = state.next_pane_id.saturating_add(1);

    let logical = logical_size(&state.config, state.scale_factor);
    let tier = (id as usize - 1) % 10;
    let x = PANE_MARGIN + tier as f32 * PANE_CASCADE_X;
    let y = PANE_MARGIN + tier as f32 * PANE_CASCADE_Y;

    let bounds = clamp_bounds_to_window(
        Bounds::new(x, y, PANE_DEFAULT_WIDTH, PANE_DEFAULT_HEIGHT),
        logical,
    );

    let pane = DesktopPane {
        id,
        title: format!("Pane {id}"),
        bounds,
        z_index: state.next_z_index,
        frame: PaneFrame::new()
            .title(format!("Pane {id}"))
            .active(true)
            .dismissable(true)
            .title_height(PANE_TITLE_HEIGHT),
    };

    state.next_z_index = state.next_z_index.saturating_add(1);
    state.panes.push(pane);
}

pub fn handle_pane_mouse_down(state: &mut RenderState, point: Point, button: MouseButton) -> bool {
    if button != MouseButton::Left {
        return false;
    }

    for pane_idx in pane_indices_by_z_desc(state) {
        let pane_id = state.panes[pane_idx].id;
        let bounds = state.panes[pane_idx].bounds;

        let down_event = InputEvent::MouseDown {
            button,
            x: point.x,
            y: point.y,
            modifiers: Modifiers::default(),
        };
        if state.panes[pane_idx]
            .frame
            .event(&down_event, bounds, &mut state.event_context)
            .is_handled()
        {
            bring_pane_to_front(state, pane_id);
            return true;
        }

        let resize_edge = state.pane_resizer.edge_at(bounds, point);
        let title_bounds = pane_title_bounds(bounds);

        if resize_edge != ResizeEdge::None || bounds.contains(point) {
            bring_pane_to_front(state, pane_id);

            if resize_edge != ResizeEdge::None {
                state.pane_drag_mode = Some(PaneDragMode::Resizing {
                    pane_id,
                    edge: resize_edge,
                    start_mouse: point,
                    start_bounds: bounds,
                });
                return true;
            }

            if title_bounds.contains(point) {
                state.pane_drag_mode = Some(PaneDragMode::Moving {
                    pane_id,
                    start_mouse: point,
                    start_bounds: bounds,
                });
            }

            return true;
        }
    }

    false
}

pub fn handle_pane_mouse_up(state: &mut RenderState, event: &InputEvent) -> bool {
    let mut handled = false;
    let mut close_target: Option<u64> = None;

    for pane_idx in pane_indices_by_z_desc(state) {
        let bounds = state.panes[pane_idx].bounds;
        if state.panes[pane_idx]
            .frame
            .event(event, bounds, &mut state.event_context)
            .is_handled()
        {
            handled = true;
        }

        if state.panes[pane_idx].frame.take_close_clicked() {
            close_target = Some(state.panes[pane_idx].id);
            break;
        }
    }

    if let Some(pane_id) = close_target {
        close_pane(state, pane_id);
        handled = true;
    }

    if state.pane_drag_mode.take().is_some() {
        handled = true;
    }

    handled
}

pub fn dispatch_pane_frame_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let mut handled = false;
    for pane_idx in pane_indices_by_z_desc(state) {
        let bounds = state.panes[pane_idx].bounds;
        if state.panes[pane_idx]
            .frame
            .event(event, bounds, &mut state.event_context)
            .is_handled()
        {
            handled = true;
        }
    }
    handled
}

pub fn update_drag(state: &mut RenderState, current_mouse: Point) -> bool {
    let Some(mode) = state.pane_drag_mode else {
        return false;
    };

    let logical = logical_size(&state.config, state.scale_factor);

    match mode {
        PaneDragMode::Moving {
            pane_id,
            start_mouse,
            start_bounds,
        } => {
            let dx = current_mouse.x - start_mouse.x;
            let dy = current_mouse.y - start_mouse.y;

            if let Some(pane) = state.panes.iter_mut().find(|pane| pane.id == pane_id) {
                let next = Bounds::new(
                    start_bounds.origin.x + dx,
                    start_bounds.origin.y + dy,
                    start_bounds.size.width,
                    start_bounds.size.height,
                );
                pane.bounds = clamp_bounds_to_window(next, logical);
                return true;
            }
        }
        PaneDragMode::Resizing {
            pane_id,
            edge,
            start_mouse,
            start_bounds,
        } => {
            if let Some(pane) = state.panes.iter_mut().find(|pane| pane.id == pane_id) {
                let next = state
                    .pane_resizer
                    .resize_bounds(edge, start_bounds, start_mouse, current_mouse);
                pane.bounds = clamp_bounds_to_window(next, logical);
                return true;
            }
        }
    }

    false
}

pub fn close_pane(state: &mut RenderState, pane_id: u64) {
    state.panes.retain(|pane| pane.id != pane_id);
}

pub fn active_pane_id(state: &RenderState) -> Option<u64> {
    state
        .panes
        .iter()
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.id)
}

pub fn cursor_icon_for_pointer(state: &RenderState, point: Point) -> CursorIcon {
    if let Some(mode) = state.pane_drag_mode {
        return match mode {
            PaneDragMode::Moving { .. } => CursorIcon::Move,
            PaneDragMode::Resizing { edge, .. } => cursor_icon_for_resize_edge(edge),
        };
    }

    if state.hotbar_bounds.contains(point) {
        return CursorIcon::Pointer;
    }

    for pane_idx in pane_indices_by_z_desc(state) {
        let bounds = state.panes[pane_idx].bounds;
        if !bounds.contains(point) {
            continue;
        }

        let edge = state.pane_resizer.edge_at(bounds, point);
        if edge != ResizeEdge::None {
            return cursor_icon_for_resize_edge(edge);
        }

        if pane_title_bounds(bounds).contains(point) {
            return CursorIcon::Move;
        }

        return CursorIcon::Default;
    }

    CursorIcon::Default
}

fn pane_indices_by_z_desc(state: &RenderState) -> Vec<usize> {
    let mut ordered: Vec<usize> = (0..state.panes.len()).collect();
    ordered.sort_by(|lhs, rhs| state.panes[*rhs].z_index.cmp(&state.panes[*lhs].z_index));
    ordered
}

fn bring_pane_to_front(state: &mut RenderState, pane_id: u64) {
    if let Some(pane) = state.panes.iter_mut().find(|pane| pane.id == pane_id) {
        pane.z_index = state.next_z_index;
        state.next_z_index = state.next_z_index.saturating_add(1);
    }
}

fn pane_title_bounds(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        PANE_TITLE_HEIGHT,
    )
}

fn cursor_icon_for_resize_edge(edge: ResizeEdge) -> CursorIcon {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorIcon::NsResize,
        ResizeEdge::Left | ResizeEdge::Right => CursorIcon::EwResize,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorIcon::NwseResize,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorIcon::NeswResize,
        ResizeEdge::None => CursorIcon::Default,
    }
}

fn clamp_bounds_to_window(bounds: Bounds, window_size: Size) -> Bounds {
    let max_width = (window_size.width - PANE_MARGIN * 2.0).max(PANE_MIN_WIDTH);
    let width = bounds.size.width.clamp(PANE_MIN_WIDTH, max_width);

    let max_height = (window_size.height - PANE_MARGIN - PANE_BOTTOM_RESERVED).max(PANE_MIN_HEIGHT);
    let height = bounds.size.height.clamp(PANE_MIN_HEIGHT, max_height);

    let max_x = (window_size.width - width - PANE_MARGIN).max(PANE_MARGIN);
    let max_y = (window_size.height - height - PANE_BOTTOM_RESERVED).max(PANE_MARGIN);

    let x = bounds.origin.x.clamp(PANE_MARGIN, max_x);
    let y = bounds.origin.y.clamp(PANE_MARGIN, max_y);

    Bounds::new(x, y, width, height)
}
