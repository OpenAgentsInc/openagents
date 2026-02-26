use wgpui::components::hud::{PaneFrame, ResizeEdge};
use wgpui::{Bounds, Component, InputEvent, Modifiers, MouseButton, Point, Size};
use winit::window::CursorIcon;

use crate::app_state::{DesktopPane, PaneDragMode, PaneKind, RenderState};
use crate::hotbar::{HOTBAR_FLOAT_GAP, HOTBAR_HEIGHT};
use crate::render::logical_size;
use crate::spark_pane::{
    self, PAY_INVOICE_PANE_HEIGHT, PAY_INVOICE_PANE_WIDTH, PayInvoicePaneAction, SPARK_PANE_HEIGHT,
    SPARK_PANE_WIDTH, SparkPaneAction,
};

const PANE_DEFAULT_WIDTH: f32 = 420.0;
const PANE_DEFAULT_HEIGHT: f32 = 280.0;
const NOSTR_PANE_WIDTH: f32 = 760.0;
const NOSTR_PANE_HEIGHT: f32 = 380.0;
pub const PANE_TITLE_HEIGHT: f32 = 28.0;
pub const PANE_MIN_WIDTH: f32 = 220.0;
pub const PANE_MIN_HEIGHT: f32 = 140.0;
const PANE_MARGIN: f32 = 18.0;
const PANE_CASCADE_X: f32 = 26.0;
const PANE_CASCADE_Y: f32 = 22.0;
const PANE_BOTTOM_RESERVED: f32 = HOTBAR_HEIGHT + HOTBAR_FLOAT_GAP + PANE_MARGIN;

pub struct PaneController;

pub struct PaneInput;

#[derive(Clone, Copy)]
pub struct PaneDescriptor {
    pub kind: PaneKind,
    pub width: f32,
    pub height: f32,
    pub singleton: bool,
}

impl PaneDescriptor {
    pub const fn empty() -> Self {
        Self {
            kind: PaneKind::Empty,
            width: PANE_DEFAULT_WIDTH,
            height: PANE_DEFAULT_HEIGHT,
            singleton: false,
        }
    }

    pub const fn nostr_identity() -> Self {
        Self {
            kind: PaneKind::NostrIdentity,
            width: NOSTR_PANE_WIDTH,
            height: NOSTR_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn spark_wallet() -> Self {
        Self {
            kind: PaneKind::SparkWallet,
            width: SPARK_PANE_WIDTH,
            height: SPARK_PANE_HEIGHT,
            singleton: true,
        }
    }

    pub const fn pay_invoice() -> Self {
        Self {
            kind: PaneKind::SparkPayInvoice,
            width: PAY_INVOICE_PANE_WIDTH,
            height: PAY_INVOICE_PANE_HEIGHT,
            singleton: true,
        }
    }
}

pub fn create_pane(state: &mut RenderState, descriptor: PaneDescriptor) -> u64 {
    if descriptor.singleton
        && let Some(existing_id) = state
            .panes
            .iter()
            .find(|pane| pane.kind == descriptor.kind)
            .map(|pane| pane.id)
    {
        bring_pane_to_front(state, existing_id);
        return existing_id;
    }

    let id = state.next_pane_id;
    state.next_pane_id = state.next_pane_id.saturating_add(1);

    let logical = logical_size(&state.config, state.scale_factor);
    let tier = (id as usize - 1) % 10;
    let x = PANE_MARGIN + tier as f32 * PANE_CASCADE_X;
    let y = PANE_MARGIN + tier as f32 * PANE_CASCADE_Y;
    let bounds = clamp_bounds_to_window(
        Bounds::new(x, y, descriptor.width, descriptor.height),
        logical,
    );

    let title = pane_title(descriptor.kind, id);
    let pane = DesktopPane {
        id,
        title: title.clone(),
        kind: descriptor.kind,
        bounds,
        z_index: state.next_z_index,
        frame: PaneFrame::new()
            .title(title)
            .active(true)
            .dismissable(true)
            .title_height(PANE_TITLE_HEIGHT),
    };

    state.next_z_index = state.next_z_index.saturating_add(1);
    state.panes.push(pane);
    id
}

impl PaneController {
    pub fn create(state: &mut RenderState, descriptor: PaneDescriptor) -> u64 {
        create_pane(state, descriptor)
    }

    pub fn create_empty(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::empty());
    }

    pub fn create_nostr_identity(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::nostr_identity());
    }

    pub fn create_spark_wallet(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::spark_wallet());
    }

    pub fn create_pay_invoice(state: &mut RenderState) {
        let _ = Self::create(state, PaneDescriptor::pay_invoice());
    }

    pub fn close(state: &mut RenderState, pane_id: u64) {
        close_pane(state, pane_id);
    }

    pub fn active(state: &RenderState) -> Option<u64> {
        active_pane_id(state)
    }

    pub fn bring_to_front(state: &mut RenderState, pane_id: u64) {
        bring_pane_to_front_by_id(state, pane_id);
    }

    pub fn update_drag(state: &mut RenderState, current_mouse: Point) -> bool {
        self::update_drag(state, current_mouse)
    }
}

impl PaneInput {
    pub fn handle_mouse_down(state: &mut RenderState, point: Point, button: MouseButton) -> bool {
        handle_pane_mouse_down(state, point, button)
    }

    pub fn handle_mouse_up(state: &mut RenderState, event: &InputEvent) -> bool {
        handle_pane_mouse_up(state, event)
    }

    pub fn dispatch_frame_event(state: &mut RenderState, event: &InputEvent) -> bool {
        dispatch_pane_frame_event(state, event)
    }

    pub fn cursor_icon(state: &RenderState, point: Point) -> CursorIcon {
        cursor_icon_for_pointer(state, point)
    }
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
                let next = state.pane_resizer.resize_bounds(
                    edge,
                    start_bounds,
                    start_mouse,
                    current_mouse,
                );
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

        if state.panes[pane_idx].kind == PaneKind::NostrIdentity {
            let content_bounds = pane_content_bounds(bounds);
            let regenerate_bounds = nostr_regenerate_button_bounds(content_bounds);
            let reveal_bounds = nostr_reveal_button_bounds(content_bounds);
            let copy_bounds = nostr_copy_secret_button_bounds(content_bounds);
            if regenerate_bounds.contains(point)
                || reveal_bounds.contains(point)
                || copy_bounds.contains(point)
            {
                return CursorIcon::Pointer;
            }
        }

        if state.panes[pane_idx].kind == PaneKind::SparkWallet {
            let content_bounds = pane_content_bounds(bounds);
            let layout = spark_pane::layout(content_bounds);
            if spark_pane::hit_action(layout, point).is_some() {
                return CursorIcon::Pointer;
            }
            if spark_pane::hits_input(layout, point) {
                return CursorIcon::Text;
            }
        }

        if state.panes[pane_idx].kind == PaneKind::SparkPayInvoice {
            let content_bounds = pane_content_bounds(bounds);
            let layout = spark_pane::pay_invoice_layout(content_bounds);
            if spark_pane::hit_pay_invoice_action(layout, point).is_some() {
                return CursorIcon::Pointer;
            }
            if spark_pane::hits_pay_invoice_input(layout, point) {
                return CursorIcon::Text;
            }
        }

        return CursorIcon::Default;
    }

    CursorIcon::Default
}

pub fn pane_content_bounds(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x,
        bounds.origin.y + PANE_TITLE_HEIGHT,
        bounds.size.width,
        (bounds.size.height - PANE_TITLE_HEIGHT).max(0.0),
    )
}

pub fn nostr_regenerate_button_bounds(content_bounds: Bounds) -> Bounds {
    let (regenerate_bounds, _, _) = nostr_button_bounds(content_bounds);
    regenerate_bounds
}

pub fn nostr_reveal_button_bounds(content_bounds: Bounds) -> Bounds {
    let (_, reveal_bounds, _) = nostr_button_bounds(content_bounds);
    reveal_bounds
}

pub fn nostr_copy_secret_button_bounds(content_bounds: Bounds) -> Bounds {
    let (_, _, copy_bounds) = nostr_button_bounds(content_bounds);
    copy_bounds
}

fn nostr_button_bounds(content_bounds: Bounds) -> (Bounds, Bounds, Bounds) {
    let gap = 8.0;
    let button_width = ((content_bounds.size.width - 24.0 - gap * 2.0) / 3.0).clamp(92.0, 156.0);
    let start_x = content_bounds.origin.x + 12.0;
    let y = content_bounds.origin.y + 12.0;

    let regenerate_bounds = Bounds::new(start_x, y, button_width, 30.0);
    let reveal_bounds = Bounds::new(
        regenerate_bounds.origin.x + button_width + gap,
        y,
        button_width,
        30.0,
    );
    let copy_bounds = Bounds::new(
        reveal_bounds.origin.x + button_width + gap,
        y,
        button_width,
        30.0,
    );

    (regenerate_bounds, reveal_bounds, copy_bounds)
}

pub fn topmost_nostr_regenerate_hit(state: &RenderState, point: Point) -> Option<u64> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::NostrIdentity {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        let regenerate_bounds = nostr_regenerate_button_bounds(content_bounds);
        if regenerate_bounds.contains(point) {
            return Some(pane.id);
        }
    }

    None
}

pub fn topmost_nostr_reveal_hit(state: &RenderState, point: Point) -> Option<u64> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::NostrIdentity {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        let reveal_bounds = nostr_reveal_button_bounds(content_bounds);
        if reveal_bounds.contains(point) {
            return Some(pane.id);
        }
    }

    None
}

pub fn topmost_nostr_copy_secret_hit(state: &RenderState, point: Point) -> Option<u64> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::NostrIdentity {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        let copy_bounds = nostr_copy_secret_button_bounds(content_bounds);
        if copy_bounds.contains(point) {
            return Some(pane.id);
        }
    }

    None
}

pub fn topmost_spark_action_hit(
    state: &RenderState,
    point: Point,
) -> Option<(u64, SparkPaneAction)> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::SparkWallet {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        let layout = spark_pane::layout(content_bounds);
        if let Some(action) = spark_pane::hit_action(layout, point) {
            return Some((pane.id, action));
        }
    }

    None
}

pub fn topmost_pay_invoice_action_hit(
    state: &RenderState,
    point: Point,
) -> Option<(u64, PayInvoicePaneAction)> {
    for pane_idx in pane_indices_by_z_desc(state) {
        let pane = &state.panes[pane_idx];
        if pane.kind != PaneKind::SparkPayInvoice {
            continue;
        }

        let content_bounds = pane_content_bounds(pane.bounds);
        let layout = spark_pane::pay_invoice_layout(content_bounds);
        if let Some(action) = spark_pane::hit_pay_invoice_action(layout, point) {
            return Some((pane.id, action));
        }
    }

    None
}

pub fn dispatch_spark_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_spark = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::SparkWallet)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_spark else {
        return false;
    };

    let content_bounds = pane_content_bounds(bounds);
    let layout = spark_pane::layout(content_bounds);
    let mut handled = false;

    handled |= state
        .spark_inputs
        .invoice_amount
        .event(event, layout.invoice_amount_input, &mut state.event_context)
        .is_handled();
    handled |= state
        .spark_inputs
        .send_request
        .event(event, layout.send_request_input, &mut state.event_context)
        .is_handled();
    handled |= state
        .spark_inputs
        .send_amount
        .event(event, layout.send_amount_input, &mut state.event_context)
        .is_handled();

    handled
}

pub fn dispatch_pay_invoice_input_event(state: &mut RenderState, event: &InputEvent) -> bool {
    let top_pay_invoice = state
        .panes
        .iter()
        .filter(|pane| pane.kind == PaneKind::SparkPayInvoice)
        .max_by_key(|pane| pane.z_index)
        .map(|pane| pane.bounds);
    let Some(bounds) = top_pay_invoice else {
        return false;
    };

    let content_bounds = pane_content_bounds(bounds);
    let layout = spark_pane::pay_invoice_layout(content_bounds);
    let mut handled = false;

    handled |= state
        .pay_invoice_inputs
        .payment_request
        .event(
            event,
            layout.payment_request_input,
            &mut state.event_context,
        )
        .is_handled();
    handled |= state
        .pay_invoice_inputs
        .amount_sats
        .event(event, layout.amount_input, &mut state.event_context)
        .is_handled();

    handled
}

pub fn bring_pane_to_front_by_id(state: &mut RenderState, pane_id: u64) {
    bring_pane_to_front(state, pane_id);
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

fn pane_title(kind: PaneKind, pane_id: u64) -> String {
    match kind {
        PaneKind::Empty => format!("Pane {pane_id}"),
        PaneKind::NostrIdentity => "Nostr Keys (NIP-06)".to_string(),
        PaneKind::SparkWallet => "Spark Lightning Wallet".to_string(),
        PaneKind::SparkPayInvoice => "Pay Lightning Invoice".to_string(),
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

#[cfg(test)]
mod tests {
    use super::{
        nostr_copy_secret_button_bounds, nostr_regenerate_button_bounds,
        nostr_reveal_button_bounds, pane_content_bounds,
    };
    use wgpui::Bounds;

    #[test]
    fn pane_content_bounds_reserve_title_space() {
        let pane = Bounds::new(10.0, 20.0, 400.0, 300.0);
        let content = pane_content_bounds(pane);

        assert!((content.origin.x - pane.origin.x).abs() <= f32::EPSILON);
        assert!(content.origin.y > pane.origin.y);
        assert!((content.size.width - pane.size.width).abs() <= f32::EPSILON);
        assert!(content.size.height < pane.size.height);
    }

    #[test]
    fn nostr_buttons_are_non_overlapping_and_ordered() {
        let content = Bounds::new(0.0, 0.0, 480.0, 220.0);
        let regenerate = nostr_regenerate_button_bounds(content);
        let reveal = nostr_reveal_button_bounds(content);
        let copy = nostr_copy_secret_button_bounds(content);

        assert!(regenerate.max_x() < reveal.min_x());
        assert!(reveal.max_x() < copy.min_x());
        assert!(regenerate.size.height > 0.0);
        assert!(reveal.size.height > 0.0);
        assert!(copy.size.height > 0.0);
    }
}
