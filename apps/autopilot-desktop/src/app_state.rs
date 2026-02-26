use std::sync::Arc;

use nostr::NostrIdentity;
use wgpui::components::hud::{Hotbar, PaneFrame, ResizablePane, ResizeEdge};
use wgpui::renderer::Renderer;
use wgpui::{Bounds, EventContext, Point, TextSystem};
use winit::window::Window;

pub const WINDOW_TITLE: &str = "Autopilot";
pub const WINDOW_WIDTH: f64 = 1280.0;
pub const WINDOW_HEIGHT: f64 = 800.0;

pub struct App {
    pub state: Option<RenderState>,
    pub cursor_position: Point,
}

impl Default for App {
    fn default() -> Self {
        Self {
            state: None,
            cursor_position: Point::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneKind {
    Empty,
    NostrIdentity,
}

#[derive(Clone, Copy)]
pub enum PaneDragMode {
    Moving {
        pane_id: u64,
        start_mouse: Point,
        start_bounds: Bounds,
    },
    Resizing {
        pane_id: u64,
        edge: ResizeEdge,
        start_mouse: Point,
        start_bounds: Bounds,
    },
}

pub struct DesktopPane {
    pub id: u64,
    pub title: String,
    pub kind: PaneKind,
    pub bounds: Bounds,
    pub z_index: i32,
    pub frame: PaneFrame,
}

pub struct RenderState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub renderer: Renderer,
    pub text_system: TextSystem,
    pub scale_factor: f32,
    pub hotbar: Hotbar,
    pub hotbar_bounds: Bounds,
    pub event_context: EventContext,
    pub panes: Vec<DesktopPane>,
    pub nostr_identity: Option<NostrIdentity>,
    pub nostr_identity_error: Option<String>,
    pub next_pane_id: u64,
    pub next_z_index: i32,
    pub pane_drag_mode: Option<PaneDragMode>,
    pub pane_resizer: ResizablePane,
    pub hotbar_flash_was_active: bool,
}
