use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use wgpui::{
    Animation, AnimatorState, Bounds, Component, Easing, EventContext, EventResult, Hsla,
    Illuminator, InputEvent, Key, Modifiers, MouseButton, NamedKey, PaintContext, Point, Quad,
    Scene, Size, SpringAnimation, Text, TextDecipher, TextEffectTiming, TextSequence, TextSystem,
    theme,
};
use wgpui::components::atoms::{
    AmountDirection, BitcoinAmount, BitcoinNetwork, BitcoinUnit, CheckpointBadge, ContentType,
    ContentTypeIcon, EntryMarker, EntryType, FeedbackButton, KeybindingHint, Mode, ModeBadge,
    Model, ModelBadge, NetworkBadge, PaymentMethod, PaymentMethodIcon, PaymentStatus,
    PaymentStatusBadge, PermissionAction, PermissionButton, Status, StatusDot, StreamingIndicator,
    ThinkingToggle, ToolIcon, ToolStatus, ToolStatusBadge, ToolType,
};
use wgpui::components::hud::{
    Command, CommandPalette, ContextMenu, CornerConfig, DotsGrid, DotsOrigin, DotShape, DrawDirection,
    Frame, FrameAnimation, FrameStyle, GridLinesBackground, LineDirection, MenuItem,
    MovingLinesBackground, Notification, NotificationLevel, NotificationPosition, Notifications,
    PuffsBackground, Reticle, Scanlines, SignalMeter, StatusBar, StatusBarPosition, StatusItem,
    StatusItemAlignment, Tooltip, TooltipPosition,
};
use wgpui::components::molecules::{
    BalanceCard, CheckpointRestore, DiffHeader, DiffType, InvoiceDisplay, InvoiceInfo, InvoiceType,
    MessageHeader, ModeSelector, ModelSelector, PaymentDirection, PaymentInfo, PaymentRow,
    PermissionBar, ThinkingBlock, ToolHeader, WalletBalance,
};
use wgpui::components::organisms::{
    AssistantMessage, DiffToolCall, DiffLine, DiffLineKind, PermissionDialog, SearchMatch,
    SearchToolCall, TerminalToolCall, ThreadControls, ThreadEntry, ThreadEntryType, ToolCallCard,
    UserMessage,
};
use wgpui::renderer::Renderer;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key as WinitKey, NamedKey as WinitNamedKey, ModifiersState};
use winit::window::{Window, WindowId};

const MARGIN: f32 = 24.0;
const HEADER_HEIGHT: f32 = 48.0;
const NAV_WIDTH: f32 = 220.0;
const NAV_ITEM_HEIGHT: f32 = 36.0;
const GAP: f32 = 20.0;
const PANEL_PADDING: f32 = 12.0;
const SECTION_GAP: f32 = 24.0;
const FRAME_TILE_W: f32 = 170.0;
const FRAME_TILE_H: f32 = 110.0;
const FRAME_TILE_GAP: f32 = 12.0;
const FRAME_VARIANT_W: f32 = 160.0;
const FRAME_VARIANT_H: f32 = 100.0;
const BG_TILE_W: f32 = 300.0;
const BG_TILE_H: f32 = 200.0;
const BG_TILE_GAP: f32 = 16.0;
const TEXT_TILE_W: f32 = 240.0;
const TEXT_TILE_H: f32 = 80.0;
const TEXT_TILE_GAP: f32 = 12.0;
const ILLUMINATOR_TILE_W: f32 = 200.0;
const ILLUMINATOR_TILE_H: f32 = 140.0;
const ILLUMINATOR_TILE_GAP: f32 = 12.0;
const LIGHT_DEMO_FRAMES_INNER_H: f32 = 320.0;
const LIGHT_DEMO_HERO_INNER_H: f32 = 280.0;
const TOOLCALL_DEMO_INNER_H: f32 = 520.0;
const SECTION_OVERVIEW: usize = 0;
const SECTION_ATOMS: usize = 1;
const SECTION_MOLECULES: usize = 2;
const SECTION_ORGANISMS: usize = 3;
const SECTION_INTERACTIONS: usize = 4;
const SECTION_ARWES_FRAMES: usize = 5;
const SECTION_ARWES_BACKGROUNDS: usize = 6;
const SECTION_ARWES_TEXT: usize = 7;
const SECTION_ARWES_ILLUMINATOR: usize = 8;
const SECTION_HUD_WIDGETS: usize = 9;
const SECTION_LIGHT_DEMO: usize = 10;
const SECTION_TOOLCALL_DEMO: usize = 11;
const SECTION_SYSTEM_UI: usize = 12;
const SECTION_CHAT_THREADS: usize = 13;
const SECTION_BITCOIN_WALLET: usize = 14;

#[derive(Clone, Copy)]
struct GlowPreset {
    short: &'static str,
    color: Hsla,
}

const GLOW_PRESETS: [GlowPreset; 8] = [
    GlowPreset { short: "Wht", color: Hsla::new(0.0, 0.0, 1.0, 0.6) },
    GlowPreset { short: "Cyn", color: Hsla::new(180.0, 1.0, 0.7, 0.5) },
    GlowPreset { short: "Pur", color: Hsla::new(280.0, 1.0, 0.7, 0.5) },
    GlowPreset { short: "Grn", color: Hsla::new(120.0, 1.0, 0.6, 0.5) },
    GlowPreset { short: "C2", color: Hsla::new(0.5, 1.0, 0.6, 0.8) },
    GlowPreset { short: "Org", color: Hsla::new(0.125, 1.0, 0.5, 0.9) },
    GlowPreset { short: "Red", color: Hsla::new(0.0, 1.0, 0.5, 1.0) },
    GlowPreset { short: "G2", color: Hsla::new(0.389, 1.0, 0.5, 0.8) },
];

const FRAME_STYLES: [FrameStyle; 9] = [
    FrameStyle::Corners,
    FrameStyle::Lines,
    FrameStyle::Octagon,
    FrameStyle::Underline,
    FrameStyle::Nefrex,
    FrameStyle::Kranox,
    FrameStyle::Nero,
    FrameStyle::Header,
    FrameStyle::Circle,
];
const FRAME_ANIMATIONS: [FrameAnimation; 4] = [
    FrameAnimation::Fade,
    FrameAnimation::Draw,
    FrameAnimation::Flicker,
    FrameAnimation::Assemble,
];
const FRAME_DIRECTIONS: [DrawDirection; 6] = [
    DrawDirection::LeftToRight,
    DrawDirection::RightToLeft,
    DrawDirection::TopToBottom,
    DrawDirection::BottomToTop,
    DrawDirection::CenterOut,
    DrawDirection::EdgesIn,
];
const DOT_SHAPES: [DotShape; 3] = [DotShape::Box, DotShape::Circle, DotShape::Cross];
const DOT_ORIGINS: [DotsOrigin; 6] = [
    DotsOrigin::Left,
    DotsOrigin::Right,
    DotsOrigin::Top,
    DotsOrigin::Bottom,
    DotsOrigin::Center,
    DotsOrigin::Point(0.25, 0.75),
];
const LINE_DIRECTIONS: [LineDirection; 4] = [
    LineDirection::Right,
    LineDirection::Left,
    LineDirection::Down,
    LineDirection::Up,
];

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("Event loop failed");
}

#[derive(Default)]
struct App {
    state: Option<RenderState>,
    cursor_position: Point,
    modifiers: ModifiersState,
}

struct RenderState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    text_system: TextSystem,
    scale_factor: f32,
    story: Storybook,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("ACP Storybook")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 900));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        let state = pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            let surface = instance
                .create_surface(window.clone())
                .expect("Failed to create surface");

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .expect("Failed to find adapter");

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("Failed to create device");

            let size = window.inner_size();
            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .unwrap_or(surface_caps.formats[0]);

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: surface_caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            let renderer = Renderer::new(&device, surface_format);
            let scale_factor = window.scale_factor() as f32;
            let text_system = TextSystem::new(scale_factor);

            RenderState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                scale_factor,
                story: Storybook::new(),
            }
        });

        self.state = Some(state);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                state.config.width = new_size.width.max(1);
                state.config.height = new_size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                state.scale_factor = scale_factor as f32;
                state.text_system.set_scale_factor(scale_factor as f32);
                state.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = Point::new(position.x as f32, position.y as f32);
                let input_event = InputEvent::MouseMove {
                    x: self.cursor_position.x,
                    y: self.cursor_position.y,
                };
                let bounds = window_bounds(&state.config);
                if state.story.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
            }
            WindowEvent::MouseInput { state: mouse_state, button, .. } => {
                let button = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    _ => return,
                };

                let input_event = match mouse_state {
                    ElementState::Pressed => InputEvent::MouseDown {
                        button,
                        x: self.cursor_position.x,
                        y: self.cursor_position.y,
                    },
                    ElementState::Released => InputEvent::MouseUp {
                        button,
                        x: self.cursor_position.x,
                        y: self.cursor_position.y,
                    },
                };

                let bounds = window_bounds(&state.config);
                if state.story.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (-x * 24.0, -y * 24.0),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        (-pos.x as f32, -pos.y as f32)
                    }
                };
                let input_event = InputEvent::Scroll { dx, dy };
                let bounds = window_bounds(&state.config);
                if state.story.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let Some(key) = map_key(&event.logical_key) else {
                    return;
                };
                let modifiers = to_modifiers(self.modifiers);
                let input_event = match event.state {
                    ElementState::Pressed => InputEvent::KeyDown { key, modifiers },
                    ElementState::Released => InputEvent::KeyUp { key, modifiers },
                };
                let bounds = window_bounds(&state.config);
                if state.story.handle_input(&input_event, bounds) {
                    state.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let bounds = window_bounds(&state.config);
                let mut scene = Scene::new();
                state.story.tick();
                state
                    .story
                    .paint(bounds, &mut PaintContext::new(&mut scene, &mut state.text_system, state.scale_factor));

                state.renderer.resize(
                    &state.queue,
                    Size::new(bounds.size.width, bounds.size.height),
                    state.scale_factor,
                );

                if state.text_system.is_dirty() {
                    state.renderer.update_atlas(
                        &state.queue,
                        state.text_system.atlas_data(),
                        state.text_system.atlas_size(),
                    );
                    state.text_system.mark_clean();
                }

                let output = match state.surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(wgpu::SurfaceError::Lost) => {
                        state.surface.configure(&state.device, &state.config);
                        return;
                    }
                    Err(_) => return,
                };

                let view = output
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let mut encoder =
                    state
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Render Encoder"),
                        });

                state.renderer.prepare(&state.device, &scene);
                state.renderer.render(&mut encoder, &view);
                state.queue.submit(std::iter::once(encoder.finish()));
                output.present();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &self.state {
            state.window.request_redraw();
        }
    }
}

fn map_key(key: &WinitKey) -> Option<Key> {
    match key {
        WinitKey::Named(named) => match named {
            WinitNamedKey::Enter => Some(Key::Named(NamedKey::Enter)),
            WinitNamedKey::Escape => Some(Key::Named(NamedKey::Escape)),
            WinitNamedKey::Backspace => Some(Key::Named(NamedKey::Backspace)),
            WinitNamedKey::Delete => Some(Key::Named(NamedKey::Delete)),
            WinitNamedKey::Tab => Some(Key::Named(NamedKey::Tab)),
            WinitNamedKey::Home => Some(Key::Named(NamedKey::Home)),
            WinitNamedKey::End => Some(Key::Named(NamedKey::End)),
            WinitNamedKey::ArrowUp => Some(Key::Named(NamedKey::ArrowUp)),
            WinitNamedKey::ArrowDown => Some(Key::Named(NamedKey::ArrowDown)),
            WinitNamedKey::ArrowLeft => Some(Key::Named(NamedKey::ArrowLeft)),
            WinitNamedKey::ArrowRight => Some(Key::Named(NamedKey::ArrowRight)),
            _ => None,
        },
        WinitKey::Character(ch) => Some(Key::Character(ch.to_string())),
        _ => None,
    }
}

fn to_modifiers(modifiers: ModifiersState) -> Modifiers {
    Modifiers {
        shift: modifiers.shift_key(),
        ctrl: modifiers.control_key(),
        alt: modifiers.alt_key(),
        meta: modifiers.super_key(),
    }
}

fn window_bounds(config: &wgpu::SurfaceConfiguration) -> Bounds {
    Bounds::new(0.0, 0.0, config.width as f32, config.height as f32)
}

struct StoryLayout {
    header: Bounds,
    nav: Bounds,
    content: Bounds,
}

struct Storybook {
    nav_items: Vec<&'static str>,
    active_section: usize,
    hover_nav: Option<usize>,
    scroll_offsets: Vec<f32>,
    cursor_position: Point,
    event_context: EventContext,
    last_tick: Instant,
    light_frame_anim: Animation<f32>,
    glow_pulse_anim: Animation<f32>,
    mode_selector: ModeSelector,
    model_selector: ModelSelector,
    permission_bar: PermissionBar,
    thinking_block: ThinkingBlock,
    checkpoint_restore: CheckpointRestore,
    thread_controls: ThreadControls,
    streaming_indicator: StreamingIndicator,
    assistant_message: AssistantMessage,
    permission_dialog: PermissionDialog,
    focus_demo: FocusDemo,
    show_permission_dialog: bool,
    toolcall_demo: ToolcallDemo,
}

impl Storybook {
    fn new() -> Self {
        let mut checkpoint_restore = CheckpointRestore::new();
        checkpoint_restore.add_checkpoint("v1.0");
        checkpoint_restore.add_checkpoint("v1.1");
        checkpoint_restore.add_checkpoint("v1.2");

        let nav_items = vec![
            "Overview",
            "Atoms",
            "Molecules",
            "Organisms",
            "Interactions",
            "Arwes Frames",
            "Arwes Backgrounds",
            "Arwes Text Effects",
            "Arwes Illuminator",
            "HUD Widgets",
            "Light Demo",
            "Toolcall Demo",
            "System UI",
            "Chat Threads",
            "Bitcoin Wallet",
        ];
        let nav_len = nav_items.len();

        let mut light_frame_anim = Animation::new(0.0_f32, 1.0, Duration::from_millis(2400))
            .easing(Easing::EaseInOutCubic)
            .iterations(0)
            .alternate();
        light_frame_anim.start();

        let mut glow_pulse_anim = Animation::new(0.4_f32, 1.0, Duration::from_millis(1800))
            .easing(Easing::EaseInOutSine)
            .iterations(0)
            .alternate();
        glow_pulse_anim.start();

        Self {
            nav_items,
            active_section: 0,
            hover_nav: None,
            scroll_offsets: vec![0.0; nav_len],
            cursor_position: Point::new(0.0, 0.0),
            event_context: EventContext::new(),
            last_tick: Instant::now(),
            light_frame_anim,
            glow_pulse_anim,
            mode_selector: ModeSelector::new(Mode::Normal),
            model_selector: ModelSelector::new(Model::ClaudeSonnet),
            permission_bar: PermissionBar::new("Permission: read repository?"),
            thinking_block: ThinkingBlock::new(
                "Chain of thought preview\nLine 2 of reasoning\nLine 3 of reasoning\nLine 4 of reasoning",
            ),
            checkpoint_restore,
            thread_controls: ThreadControls::new().running(true),
            streaming_indicator: StreamingIndicator::new(),
            assistant_message: AssistantMessage::new("Streaming response from ACP components")
                .model(Model::ClaudeSonnet)
                .streaming(true),
            permission_dialog: PermissionDialog::default(),
            focus_demo: FocusDemo::new(),
            show_permission_dialog: true,
            toolcall_demo: ToolcallDemo::new(),
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let delta = now.saturating_duration_since(self.last_tick);
        self.last_tick = now;
        self.streaming_indicator.tick();
        self.assistant_message.tick();
        self.light_frame_anim.tick(delta);
        self.glow_pulse_anim.tick(delta);
        self.toolcall_demo.tick(delta);
    }

    fn layout(&self, bounds: Bounds) -> StoryLayout {
        let header = Bounds::new(
            bounds.origin.x + MARGIN,
            bounds.origin.y + MARGIN,
            (bounds.size.width - MARGIN * 2.0).max(0.0),
            HEADER_HEIGHT,
        );
        let content_height = (bounds.size.height - MARGIN * 2.0 - HEADER_HEIGHT - GAP).max(0.0);
        let nav = Bounds::new(
            bounds.origin.x + MARGIN,
            header.origin.y + header.size.height + GAP,
            NAV_WIDTH.min((bounds.size.width - MARGIN * 2.0).max(0.0)),
            content_height,
        );
        let content_width = (bounds.size.width - MARGIN * 2.0 - NAV_WIDTH - GAP).max(0.0);
        let content = Bounds::new(
            nav.origin.x + nav.size.width + GAP,
            nav.origin.y,
            content_width,
            content_height,
        );
        StoryLayout {
            header,
            nav,
            content,
        }
    }

    fn max_scroll_for_section(&self, section: usize, bounds: Bounds) -> f32 {
        let content_height = self.section_content_height(section, bounds);
        (content_height - bounds.size.height).max(0.0)
    }

    fn section_content_height(&self, section: usize, bounds: Bounds) -> f32 {
        match section {
            SECTION_ARWES_FRAMES => arwes_frames_height(bounds),
            SECTION_ARWES_BACKGROUNDS => arwes_backgrounds_height(bounds),
            SECTION_ARWES_TEXT => arwes_text_effects_height(bounds),
            SECTION_ARWES_ILLUMINATOR => arwes_illuminator_height(bounds),
            SECTION_HUD_WIDGETS => hud_widgets_height(bounds),
            SECTION_LIGHT_DEMO => light_demo_height(bounds),
            SECTION_TOOLCALL_DEMO => toolcall_demo_height(bounds),
            SECTION_SYSTEM_UI => system_ui_height(bounds),
            SECTION_CHAT_THREADS => chat_threads_height(bounds),
            SECTION_BITCOIN_WALLET => bitcoin_wallet_height(bounds),
            _ => bounds.size.height,
        }
    }

    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene
            .draw_quad(Quad::new(bounds).with_background(theme::bg::APP));

        let layout = self.layout(bounds);
        self.paint_header(layout.header, cx);
        self.paint_nav(layout.nav, cx);

        let max_scroll = self.max_scroll_for_section(self.active_section, layout.content);
        if let Some(offset) = self.scroll_offsets.get_mut(self.active_section) {
            *offset = offset.clamp(0.0, max_scroll);
        }
        let scroll = self.scroll_offsets.get(self.active_section).copied().unwrap_or(0.0);
        let content_bounds = Bounds::new(
            layout.content.origin.x,
            layout.content.origin.y - scroll,
            layout.content.size.width,
            layout.content.size.height,
        );

        cx.scene.push_clip(layout.content);
        match self.active_section {
            SECTION_OVERVIEW => self.paint_overview(content_bounds, cx),
            SECTION_ATOMS => self.paint_atoms(content_bounds, cx),
            SECTION_MOLECULES => self.paint_molecules(content_bounds, cx),
            SECTION_ORGANISMS => self.paint_organisms(content_bounds, cx),
            SECTION_INTERACTIONS => self.paint_interactions(content_bounds, cx),
            SECTION_ARWES_FRAMES => self.paint_arwes_frames(content_bounds, cx),
            SECTION_ARWES_BACKGROUNDS => self.paint_arwes_backgrounds(content_bounds, cx),
            SECTION_ARWES_TEXT => self.paint_arwes_text_effects(content_bounds, cx),
            SECTION_ARWES_ILLUMINATOR => self.paint_arwes_illuminator(content_bounds, cx),
            SECTION_HUD_WIDGETS => self.paint_hud_widgets(content_bounds, cx),
            SECTION_LIGHT_DEMO => self.paint_light_demo(content_bounds, cx),
            SECTION_TOOLCALL_DEMO => self.paint_toolcall_demo(content_bounds, cx),
            SECTION_SYSTEM_UI => self.paint_system_ui(content_bounds, cx),
            SECTION_CHAT_THREADS => self.paint_chat_threads(content_bounds, cx),
            SECTION_BITCOIN_WALLET => self.paint_bitcoin_wallet(content_bounds, cx),
            _ => {}
        }
        cx.scene.pop_clip();
    }

    fn handle_input(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        let layout = self.layout(bounds);
        let mut handled = self.handle_nav_event(event, layout.nav);

        if let InputEvent::MouseMove { x, y } = event {
            self.cursor_position = Point::new(*x, *y);
        }

        if let InputEvent::Scroll { dy, .. } = event {
            if layout.content.contains(self.cursor_position) {
                let max_scroll = self.max_scroll_for_section(self.active_section, layout.content);
                if let Some(offset) = self.scroll_offsets.get_mut(self.active_section) {
                    let next = (*offset + *dy).clamp(0.0, max_scroll);
                    if (next - *offset).abs() > 0.1 {
                        *offset = next;
                        handled = true;
                    }
                }
            }
            return handled;
        }

        let scroll = self.scroll_offsets.get(self.active_section).copied().unwrap_or(0.0);
        let content_bounds = Bounds::new(
            layout.content.origin.x,
            layout.content.origin.y - scroll,
            layout.content.size.width,
            layout.content.size.height,
        );
        let in_content = layout.content.contains(self.cursor_position);
        let translated = match event {
            InputEvent::MouseMove { .. }
            | InputEvent::MouseDown { .. }
            | InputEvent::MouseUp { .. } => {
                if in_content {
                    Some(event.clone())
                } else {
                    None
                }
            }
            _ => Some(event.clone()),
        };

        let Some(event) = translated else {
            return handled;
        };

        match self.active_section {
            SECTION_MOLECULES => handled |= self.handle_molecules_event(&event, content_bounds),
            SECTION_ORGANISMS => handled |= self.handle_organisms_event(&event, content_bounds),
            SECTION_INTERACTIONS => handled |= self.handle_interactions_event(&event, content_bounds),
            _ => {}
        }

        handled
    }

    fn paint_header(&self, bounds: Bounds, cx: &mut PaintContext) {
        let title = "ACP Storybook";
        let subtitle = "ACP components rendered with WGPUI";
        let mut title_text = Text::new(title)
            .font_size(theme::font_size::LG)
            .color(theme::text::PRIMARY);
        title_text.paint(
            Bounds::new(
                bounds.origin.x,
                bounds.origin.y + 4.0,
                bounds.size.width,
                24.0,
            ),
            cx,
        );

        let mut subtitle_text = Text::new(subtitle)
            .font_size(theme::font_size::SM)
            .color(theme::text::MUTED);
        subtitle_text.paint(
            Bounds::new(
                bounds.origin.x,
                bounds.origin.y + 28.0,
                bounds.size.width,
                20.0,
            ),
            cx,
        );

        cx.scene.draw_quad(
            Quad::new(Bounds::new(
                bounds.origin.x,
                bounds.origin.y + bounds.size.height - 2.0,
                bounds.size.width,
                2.0,
            ))
            .with_background(theme::accent::PRIMARY),
        );
    }

    fn paint_nav(&self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let pad = PANEL_PADDING;
        for (i, label) in self.nav_items.iter().enumerate() {
            let item_bounds = Bounds::new(
                bounds.origin.x + pad,
                bounds.origin.y + pad + i as f32 * NAV_ITEM_HEIGHT,
                bounds.size.width - pad * 2.0,
                NAV_ITEM_HEIGHT,
            );
            let is_active = self.active_section == i;
            let is_hover = self.hover_nav == Some(i);
            let bg = if is_active {
                theme::accent::PRIMARY.with_alpha(0.2)
            } else if is_hover {
                theme::bg::HOVER
            } else {
                Hsla::transparent()
            };

            if bg.a > 0.0 {
                cx.scene.draw_quad(Quad::new(item_bounds).with_background(bg));
            }

            let mut text = Text::new(*label)
                .font_size(theme::font_size::SM)
                .color(if is_active {
                    theme::text::PRIMARY
                } else {
                    theme::text::MUTED
                });
            text.paint(
                Bounds::new(
                    item_bounds.origin.x + 8.0,
                    item_bounds.origin.y + 6.0,
                    item_bounds.size.width - 16.0,
                    item_bounds.size.height,
                ),
                cx,
            );
        }
    }

    fn handle_nav_event(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        let pad = PANEL_PADDING;
        match event {
            InputEvent::MouseMove { x, y } => {
                let mut hover = None;
                if bounds.contains(Point::new(*x, *y)) {
                    let rel_y = *y - bounds.origin.y - pad;
                    if rel_y >= 0.0 {
                        let idx = (rel_y / NAV_ITEM_HEIGHT) as usize;
                        if idx < self.nav_items.len() {
                            hover = Some(idx);
                        }
                    }
                }
                if hover != self.hover_nav {
                    self.hover_nav = hover;
                    return true;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left && bounds.contains(Point::new(*x, *y)) {
                    let rel_y = *y - bounds.origin.y - pad;
                    if rel_y >= 0.0 {
                        let idx = (rel_y / NAV_ITEM_HEIGHT) as usize;
                        if idx < self.nav_items.len() {
                            self.active_section = idx;
                            return true;
                        }
                    }
                }
            }
            _ => {}
        }
        false
    }

    fn paint_overview(&self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let mut y = bounds.origin.y + PANEL_PADDING;
        let mut title = Text::new("Overview")
            .font_size(theme::font_size::BASE)
            .color(theme::text::PRIMARY);
        title.paint(
            Bounds::new(
                bounds.origin.x + PANEL_PADDING,
                y,
                bounds.size.width - PANEL_PADDING * 2.0,
                24.0,
            ),
            cx,
        );
        y += 32.0;

        let body = [
            "This gallery covers ACP components plus Arwes parity components.",
            "Use the navigation to explore each layer and Arwes variants.",
            "Scroll inside the content pane to see full permutations.",
        ];

        for line in body {
            let mut text = Text::new(line)
                .font_size(theme::font_size::SM)
                .color(theme::text::MUTED);
            text.paint(
                Bounds::new(
                    bounds.origin.x + PANEL_PADDING,
                    y,
                    bounds.size.width - PANEL_PADDING * 2.0,
                    20.0,
                ),
                cx,
            );
            y += 22.0;
        }
    }

    fn paint_atoms(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let cols = 3;
        let gap = 16.0;
        let tile_w = ((bounds.size.width - gap * (cols as f32 - 1.0)) / cols as f32).max(0.0);
        let tile_h = 96.0;

        let tiles = [
            "Tool icon",
            "Tool status",
            "Streaming",
            "Mode badge",
            "Model badge",
            "Status dot",
            "Permission button",
            "Thinking toggle",
            "Feedback buttons",
            "Entry marker",
            "Content type",
            "Checkpoint badge",
            "Keybinding hint",
        ];

        for (index, label) in tiles.iter().enumerate() {
            let col = (index % cols) as f32;
            let row = (index / cols) as f32;
            let x = bounds.origin.x + col * (tile_w + gap);
            let y = bounds.origin.y + row * (tile_h + gap);
            let tile_bounds = Bounds::new(x, y, tile_w, tile_h);

            draw_tile(tile_bounds, label, cx, |inner, cx| {
                match *label {
                    "Tool icon" => {
                        let mut icon = ToolIcon::new(ToolType::Bash);
                        paint_centered(&mut icon, inner, cx);
                    }
                    "Tool status" => {
                        let mut badge = ToolStatusBadge::new(ToolStatus::Running);
                        paint_centered(&mut badge, inner, cx);
                    }
                    "Streaming" => {
                        self.streaming_indicator
                            .paint(center_bounds(inner, 80.0, 20.0), cx);
                    }
                    "Mode badge" => {
                        let mut badge = ModeBadge::new(Mode::Act);
                        paint_centered(&mut badge, inner, cx);
                    }
                    "Model badge" => {
                        let mut badge = ModelBadge::new(Model::ClaudeSonnet);
                        paint_centered(&mut badge, inner, cx);
                    }
                    "Status dot" => {
                        let mut dot = StatusDot::new(Status::Busy).size(10.0);
                        paint_centered(&mut dot, inner, cx);
                    }
                    "Permission button" => {
                        let mut btn = PermissionButton::new(PermissionAction::AllowOnce);
                        paint_centered(&mut btn, inner, cx);
                    }
                    "Thinking toggle" => {
                        let mut toggle = ThinkingToggle::new().expanded(true);
                        paint_centered(&mut toggle, inner, cx);
                    }
                    "Feedback buttons" => {
                        let mut up = FeedbackButton::thumbs_up().selected(true);
                        let mut down = FeedbackButton::thumbs_down();
                        let left = Bounds::new(inner.origin.x, inner.origin.y, inner.size.width / 2.0, inner.size.height);
                        let right = Bounds::new(
                            inner.origin.x + inner.size.width / 2.0,
                            inner.origin.y,
                            inner.size.width / 2.0,
                            inner.size.height,
                        );
                        paint_centered(&mut up, left, cx);
                        paint_centered(&mut down, right, cx);
                    }
                    "Entry marker" => {
                        let mut marker = EntryMarker::new(EntryType::Tool);
                        paint_centered(&mut marker, inner, cx);
                    }
                    "Content type" => {
                        let mut icon = ContentTypeIcon::new(ContentType::Markdown);
                        paint_centered(&mut icon, inner, cx);
                    }
                    "Checkpoint badge" => {
                        let mut badge = CheckpointBadge::new("v1.2").active(true);
                        paint_centered(&mut badge, inner, cx);
                    }
                    _ => {
                        let mut hint = KeybindingHint::combo(&["Ctrl", "K"]);
                        paint_centered(&mut hint, inner, cx);
                    }
                }
            });
        }
    }

    fn paint_molecules(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let col_gap = GAP;
        let col_width = ((bounds.size.width - col_gap) / 2.0).max(0.0);
        let left_x = bounds.origin.x;
        let right_x = bounds.origin.x + col_width + col_gap;

        let selectors = Bounds::new(left_x, bounds.origin.y, col_width, 90.0);
        let permission = Bounds::new(left_x, bounds.origin.y + 110.0, col_width, 70.0);
        let checkpoints = Bounds::new(left_x, bounds.origin.y + 200.0, col_width, 90.0);

        let thinking = Bounds::new(right_x, bounds.origin.y, col_width, 170.0);
        let headers = Bounds::new(right_x, bounds.origin.y + 190.0, col_width, 120.0);

        draw_panel("Selectors", selectors, cx, |inner, cx| {
            let selector_h = 28.0;
            self.mode_selector.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, selector_h),
                cx,
            );
            self.model_selector.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + selector_h + 12.0,
                    inner.size.width,
                    selector_h,
                ),
                cx,
            );
        });

        draw_panel("Permission bar", permission, cx, |inner, cx| {
            self.permission_bar.paint(inner, cx);
        });

        draw_panel("Checkpoint restore", checkpoints, cx, |inner, cx| {
            self.checkpoint_restore.paint(inner, cx);
        });

        draw_panel("Thinking block", thinking, cx, |inner, cx| {
            self.thinking_block.paint(inner, cx);
        });

        draw_panel("Headers", headers, cx, |inner, cx| {
            let row_height = 28.0;
            let mut header = MessageHeader::assistant(Model::ClaudeHaiku).timestamp("12:42");
            header.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, row_height),
                cx,
            );
            let mut tool_header = ToolHeader::new(ToolType::Read, "read_file").status(ToolStatus::Success);
            tool_header.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + row_height + 8.0,
                    inner.size.width,
                    row_height,
                ),
                cx,
            );
            let mut diff_header = DiffHeader::new("src/main.rs")
                .additions(3)
                .deletions(1)
                .diff_type(DiffType::Unified);
            diff_header.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + (row_height + 8.0) * 2.0,
                    inner.size.width,
                    row_height,
                ),
                cx,
            );
        });
    }

    fn handle_molecules_event(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        let col_gap = GAP;
        let col_width = ((bounds.size.width - col_gap) / 2.0).max(0.0);
        let left_x = bounds.origin.x;
        let right_x = bounds.origin.x + col_width + col_gap;

        let selectors = Bounds::new(left_x, bounds.origin.y, col_width, 90.0);
        let permission = Bounds::new(left_x, bounds.origin.y + 110.0, col_width, 70.0);
        let checkpoints = Bounds::new(left_x, bounds.origin.y + 200.0, col_width, 90.0);
        let thinking = Bounds::new(right_x, bounds.origin.y, col_width, 170.0);

        let selectors_inner = panel_inner(selectors);
        let permission_inner = panel_inner(permission);
        let checkpoints_inner = panel_inner(checkpoints);
        let thinking_inner = panel_inner(thinking);

        let mut handled = false;
        handled |= component_event(
            &mut self.mode_selector,
            event,
            Bounds::new(
                selectors_inner.origin.x,
                selectors_inner.origin.y,
                selectors_inner.size.width,
                28.0,
            ),
            &mut self.event_context,
        );
        handled |= component_event(
            &mut self.model_selector,
            event,
            Bounds::new(
                selectors_inner.origin.x,
                selectors_inner.origin.y + 40.0,
                selectors_inner.size.width,
                28.0,
            ),
            &mut self.event_context,
        );

        handled |= component_event(
            &mut self.permission_bar,
            event,
            permission_inner,
            &mut self.event_context,
        );

        handled |= component_event(
            &mut self.checkpoint_restore,
            event,
            checkpoints_inner,
            &mut self.event_context,
        );

        handled |= component_event(
            &mut self.thinking_block,
            event,
            thinking_inner,
            &mut self.event_context,
        );

        handled
    }

    fn paint_organisms(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let col_gap = GAP;
        let col_width = ((bounds.size.width - col_gap) / 2.0).max(0.0);
        let left_x = bounds.origin.x;
        let right_x = bounds.origin.x + col_width + col_gap;

        let user_msg = Bounds::new(left_x, bounds.origin.y, col_width, 140.0);
        let asst_msg = Bounds::new(left_x, bounds.origin.y + 160.0, col_width, 180.0);
        let thread_entry = Bounds::new(left_x, bounds.origin.y + 360.0, col_width, 120.0);

        let controls = Bounds::new(right_x, bounds.origin.y, col_width, 80.0);
        let tool_card = Bounds::new(right_x, bounds.origin.y + 100.0, col_width, 160.0);
        let terminal = Bounds::new(right_x, bounds.origin.y + 280.0, col_width, 150.0);
        let permission = Bounds::new(right_x, bounds.origin.y + 450.0, col_width, 160.0);

        draw_panel("User message", user_msg, cx, |inner, cx| {
            let mut msg = UserMessage::new("User says hello from ACP.");
            msg.paint(inner, cx);
        });

        draw_panel("Assistant message", asst_msg, cx, |inner, cx| {
            self.assistant_message.paint(inner, cx);
        });

        draw_panel("Thread entry", thread_entry, cx, |inner, cx| {
            let mut entry = ThreadEntry::new(ThreadEntryType::System, Text::new("System note"));
            entry.paint(inner, cx);
        });

        draw_panel("Thread controls", controls, cx, |inner, cx| {
            self.thread_controls.paint(inner, cx);
        });

        draw_panel("Tool call card", tool_card, cx, |inner, cx| {
            let mut card = ToolCallCard::new(ToolType::Read, "read_file")
                .status(ToolStatus::Success)
                .input("path: /etc/hosts")
                .output("read 12 lines");
            card.paint(inner, cx);
        });

        draw_panel("Terminal tool", terminal, cx, |inner, cx| {
            let mut tool = TerminalToolCall::new("ls -la")
                .output("src\nCargo.toml\ntarget")
                .status(ToolStatus::Running);
            tool.paint(inner, cx);
        });

        draw_panel("Permission dialog", permission, cx, |inner, cx| {
            if self.show_permission_dialog {
                self.permission_dialog.show();
            } else {
                self.permission_dialog.hide();
            }
            self.permission_dialog.paint(inner, cx);
        });
    }

    fn handle_organisms_event(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        let col_gap = GAP;
        let col_width = ((bounds.size.width - col_gap) / 2.0).max(0.0);
        let right_x = bounds.origin.x + col_width + col_gap;

        let controls = Bounds::new(right_x, bounds.origin.y, col_width, 80.0);
        let permission = Bounds::new(right_x, bounds.origin.y + 450.0, col_width, 160.0);
        let controls_inner = panel_inner(controls);
        let permission_inner = panel_inner(permission);

        let mut handled = false;
        handled |= component_event(
            &mut self.thread_controls,
            event,
            controls_inner,
            &mut self.event_context,
        );

        if let InputEvent::KeyDown { key, .. } = event {
            if let Key::Character(ch) = key {
                if ch.eq_ignore_ascii_case("p") {
                    self.show_permission_dialog = !self.show_permission_dialog;
                    handled = true;
                }
            }
        }

        handled |= component_event(
            &mut self.permission_dialog,
            event,
            permission_inner,
            &mut self.event_context,
        );

        if let InputEvent::MouseDown { button, x, y } = event {
            if *button == MouseButton::Left
                && permission.contains(Point::new(*x, *y))
                && !self.permission_dialog.is_open()
            {
                self.permission_dialog.show();
                handled = true;
            }
        }

        handled
    }

    fn paint_interactions(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let focus_panel = Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, 150.0);
        let tool_panel = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 170.0,
            bounds.size.width,
            260.0,
        );
        let stream_panel = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 450.0,
            bounds.size.width,
            140.0,
        );

        draw_panel("Focus and keyboard", focus_panel, cx, |inner, cx| {
            self.focus_demo.paint(inner, cx);
        });

        draw_panel("Tool cards", tool_panel, cx, |inner, cx| {
            let col_gap = 16.0;
            let col_width = ((inner.size.width - col_gap) / 2.0).max(0.0);
            let left = Bounds::new(inner.origin.x, inner.origin.y, col_width, inner.size.height);
            let right = Bounds::new(inner.origin.x + col_width + col_gap, inner.origin.y, col_width, inner.size.height);

            let mut tool = ToolCallCard::new(ToolType::Search, "grep")
                .status(ToolStatus::Success)
                .input("query: todo")
                .output("6 matches");
            tool.paint(center_bounds(left, left.size.width, 140.0), cx);

            let mut search = SearchToolCall::new("todo").status(ToolStatus::Success);
            search.paint(center_bounds(right, right.size.width, 180.0), cx);
        });

        draw_panel("Streaming indicator", stream_panel, cx, |inner, cx| {
            let mut title = Text::new("Press S to toggle streaming")
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            title.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 16.0),
                cx,
            );
            self.streaming_indicator
                .paint(center_bounds(inner, 120.0, 24.0), cx);
        });
    }

    fn handle_interactions_event(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        let focus_panel = Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, 150.0);
        let stream_panel = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + 450.0,
            bounds.size.width,
            140.0,
        );
        let focus_inner = panel_inner(focus_panel);
        let stream_inner = panel_inner(stream_panel);

        let mut handled = self.focus_demo.handle_event(event, focus_inner);

        if let InputEvent::KeyDown { key, .. } = event {
            if let Key::Character(ch) = key {
                if ch.eq_ignore_ascii_case("s") {
                    let active = self.streaming_indicator.is_active();
                    self.streaming_indicator.set_active(!active);
                    handled = true;
                }
            }
        }

        if let InputEvent::MouseDown { button, x, y } = event {
            if *button == MouseButton::Left && stream_inner.contains(Point::new(*x, *y)) {
                let active = self.streaming_indicator.is_active();
                self.streaming_indicator.set_active(!active);
                handled = true;
            }
        }

        handled
    }

    fn paint_arwes_frames(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;
        let available = (width - PANEL_PADDING * 2.0).max(0.0);

        let permutations = FRAME_STYLES.len() * FRAME_ANIMATIONS.len() * FRAME_DIRECTIONS.len();
        let grid = grid_metrics(available, permutations, FRAME_TILE_W, FRAME_TILE_H, FRAME_TILE_GAP);
        let permutation_height = panel_height(grid.height);
        let panel_bounds = Bounds::new(bounds.origin.x, y, width, permutation_height);
        draw_panel(
            "Frame permutations (style x animation x direction)",
            panel_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    permutations,
                    FRAME_TILE_W,
                    FRAME_TILE_H,
                    FRAME_TILE_GAP,
                );
                let mut idx = 0;
                for style in FRAME_STYLES.iter().copied() {
                    for animation in FRAME_ANIMATIONS.iter().copied() {
                        for direction in FRAME_DIRECTIONS.iter().copied() {
                            let row = idx / grid.cols;
                            let col = idx % grid.cols;
                            let tile_bounds = Bounds::new(
                                inner.origin.x + col as f32 * (FRAME_TILE_W + FRAME_TILE_GAP),
                                inner.origin.y + row as f32 * (FRAME_TILE_H + FRAME_TILE_GAP),
                                FRAME_TILE_W,
                                FRAME_TILE_H,
                            );
                            let label = format!(
                                "{} {} {}",
                                frame_style_label(style),
                                frame_animation_label(animation),
                                draw_direction_label(direction)
                            );
                            draw_tile(tile_bounds, &label, cx, |inner, cx| {
                                let progress = match animation {
                                    FrameAnimation::Fade => 1.0,
                                    FrameAnimation::Flicker => 0.6,
                                    FrameAnimation::Draw | FrameAnimation::Assemble => 0.65,
                                };
                                let mut frame = Frame::new()
                                    .style(style)
                                    .animation_mode(animation)
                                    .draw_direction(direction)
                                    .animation_progress(progress);
                                if animation == FrameAnimation::Flicker {
                                    frame = frame.is_exiting(false);
                                }
                                frame.paint(inset_bounds(inner, 4.0), cx);
                            });
                            idx += 1;
                        }
                    }
                }
            },
        );
        y += permutation_height + SECTION_GAP;

        let flicker_count = FRAME_STYLES.len() * 2;
        let flicker_grid = grid_metrics(available, flicker_count, FRAME_VARIANT_W, FRAME_VARIANT_H, FRAME_TILE_GAP);
        let flicker_height = panel_height(flicker_grid.height);
        let flicker_bounds = Bounds::new(bounds.origin.x, y, width, flicker_height);
        draw_panel(
            "Flicker state (enter vs exit)",
            flicker_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    flicker_count,
                    FRAME_VARIANT_W,
                    FRAME_VARIANT_H,
                    FRAME_TILE_GAP,
                );
                let mut idx = 0;
                for style in FRAME_STYLES.iter().copied() {
                    for exiting in [false, true] {
                        let row = idx / grid.cols;
                        let col = idx % grid.cols;
                        let tile_bounds = Bounds::new(
                            inner.origin.x + col as f32 * (FRAME_VARIANT_W + FRAME_TILE_GAP),
                            inner.origin.y + row as f32 * (FRAME_VARIANT_H + FRAME_TILE_GAP),
                            FRAME_VARIANT_W,
                            FRAME_VARIANT_H,
                        );
                        let label = format!(
                            "{} {}",
                            frame_style_label(style),
                            if exiting { "Exit" } else { "Enter" }
                        );
                        draw_tile(tile_bounds, &label, cx, |inner, cx| {
                            let mut frame = Frame::new()
                                .style(style)
                                .animation_mode(FrameAnimation::Flicker)
                                .animation_progress(0.6)
                                .is_exiting(exiting);
                            frame.paint(inset_bounds(inner, 4.0), cx);
                        });
                        idx += 1;
                    }
                }
            },
        );
        y += flicker_height + SECTION_GAP;

        let glow_count = FRAME_STYLES.len() * 2;
        let glow_grid = grid_metrics(available, glow_count, FRAME_VARIANT_W, FRAME_VARIANT_H, FRAME_TILE_GAP);
        let glow_height = panel_height(glow_grid.height);
        let glow_bounds = Bounds::new(bounds.origin.x, y, width, glow_height);
        draw_panel("Glow toggle (off/on)", glow_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                glow_count,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            );
            let mut idx = 0;
            for style in FRAME_STYLES.iter().copied() {
                for glow in [false, true] {
                    let row = idx / grid.cols;
                    let col = idx % grid.cols;
                    let tile_bounds = Bounds::new(
                        inner.origin.x + col as f32 * (FRAME_VARIANT_W + FRAME_TILE_GAP),
                        inner.origin.y + row as f32 * (FRAME_VARIANT_H + FRAME_TILE_GAP),
                        FRAME_VARIANT_W,
                        FRAME_VARIANT_H,
                    );
                    let label = format!(
                        "{} {}",
                        frame_style_label(style),
                        if glow { "Glow" } else { "NoGlow" }
                    );
                    draw_tile(tile_bounds, &label, cx, |inner, cx| {
                        let mut frame = Frame::new()
                            .style(style)
                            .animation_mode(FrameAnimation::Fade)
                            .animation_progress(1.0);
                        if glow {
                            frame = frame.glow_color(theme::accent::PRIMARY);
                        }
                        frame.paint(inset_bounds(inner, 4.0), cx);
                    });
                    idx += 1;
                }
            }
        });
        y += glow_height + SECTION_GAP;

        let glow_palette_count = FRAME_STYLES.len() * FRAME_ANIMATIONS.len() * GLOW_PRESETS.len();
        let glow_palette_grid =
            grid_metrics(available, glow_palette_count, FRAME_VARIANT_W, FRAME_VARIANT_H, FRAME_TILE_GAP);
        let glow_palette_height = panel_height(glow_palette_grid.height);
        let glow_palette_bounds = Bounds::new(bounds.origin.x, y, width, glow_palette_height);
        draw_panel("Glow palette x animation", glow_palette_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                glow_palette_count,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            );
            let progress = self.light_frame_anim.current_value();
            let glow_pulse = self.glow_pulse_anim.current_value();
            let flicker_exit = progress > 0.5;
            let white = Hsla::new(0.0, 0.0, 1.0, 1.0);
            let dark_bg = Hsla::new(0.0, 0.0, 0.08, 0.85);
            let mut idx = 0;

            for style in FRAME_STYLES.iter().copied() {
                for animation in FRAME_ANIMATIONS.iter().copied() {
                    for preset in GLOW_PRESETS.iter().copied() {
                        let row = idx / grid.cols;
                        let col = idx % grid.cols;
                        let tile_bounds = Bounds::new(
                            inner.origin.x + col as f32 * (FRAME_VARIANT_W + FRAME_TILE_GAP),
                            inner.origin.y + row as f32 * (FRAME_VARIANT_H + FRAME_TILE_GAP),
                            FRAME_VARIANT_W,
                            FRAME_VARIANT_H,
                        );
                        let label = format!(
                            "{} {} {}",
                            frame_style_short(style),
                            frame_animation_label(animation),
                            preset.short
                        );
                        draw_tile(tile_bounds, &label, cx, |inner, cx| {
                            let mut frame = demo_frame(style)
                                .line_color(white)
                                .bg_color(dark_bg)
                                .stroke_width(2.0)
                                .animation_mode(animation)
                                .draw_direction(DrawDirection::CenterOut)
                                .animation_progress(progress);
                            if animation == FrameAnimation::Flicker {
                                frame = frame.is_exiting(flicker_exit);
                            }
                            let glow = preset.color.with_alpha(preset.color.a * glow_pulse);
                            frame = frame.glow_color(glow);
                            frame.paint(inset_bounds(inner, 4.0), cx);
                        });
                        idx += 1;
                    }
                }
            }
        });
        y += glow_palette_height + SECTION_GAP;

        let nefrex_count = 16;
        let nefrex_grid = grid_metrics(available, nefrex_count, FRAME_VARIANT_W, FRAME_VARIANT_H, FRAME_TILE_GAP);
        let nefrex_height = panel_height(nefrex_grid.height);
        let nefrex_bounds = Bounds::new(bounds.origin.x, y, width, nefrex_height);
        draw_panel(
            "Nefrex corners (LT LB RT RB order)",
            nefrex_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    nefrex_count,
                    FRAME_VARIANT_W,
                    FRAME_VARIANT_H,
                    FRAME_TILE_GAP,
                );
                for mask in 0..16 {
                    let row = mask / grid.cols;
                    let col = mask % grid.cols;
                    let tile_bounds = Bounds::new(
                        inner.origin.x + col as f32 * (FRAME_VARIANT_W + FRAME_TILE_GAP),
                        inner.origin.y + row as f32 * (FRAME_VARIANT_H + FRAME_TILE_GAP),
                        FRAME_VARIANT_W,
                        FRAME_VARIANT_H,
                    );
                    let config = CornerConfig {
                        left_top: mask & 1 != 0,
                        left_bottom: mask & 2 != 0,
                        right_top: mask & 4 != 0,
                        right_bottom: mask & 8 != 0,
                    };
                    let label = format!("{:04b}", mask);
                    draw_tile(tile_bounds, &label, cx, |inner, cx| {
                        let mut frame = Frame::new()
                            .style(FrameStyle::Nefrex)
                            .corner_config(config)
                            .animation_progress(1.0);
                        frame.paint(inset_bounds(inner, 4.0), cx);
                    });
                }
            },
        );
        y += nefrex_height + SECTION_GAP;

        let header_count = 2;
        let header_grid = grid_metrics(available, header_count, FRAME_VARIANT_W, FRAME_VARIANT_H, FRAME_TILE_GAP);
        let header_height = panel_height(header_grid.height);
        let header_bounds = Bounds::new(bounds.origin.x, y, width, header_height);
        draw_panel("Header bottom toggle", header_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                header_count,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            );
            for (idx, bottom) in [false, true].iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (FRAME_VARIANT_W + FRAME_TILE_GAP),
                    inner.origin.y + row as f32 * (FRAME_VARIANT_H + FRAME_TILE_GAP),
                    FRAME_VARIANT_W,
                    FRAME_VARIANT_H,
                );
                let label = if *bottom { "Bottom" } else { "Top" };
                draw_tile(tile_bounds, label, cx, |inner, cx| {
                    let mut frame = Frame::new()
                        .style(FrameStyle::Header)
                        .header_bottom(*bottom)
                        .animation_progress(1.0);
                    frame.paint(inset_bounds(inner, 4.0), cx);
                });
            }
        });
        y += header_height + SECTION_GAP;

        let circle_segments = [8u32, 16, 32, 64];
        let circle_count = circle_segments.len();
        let circle_grid = grid_metrics(available, circle_count, FRAME_VARIANT_W, FRAME_VARIANT_H, FRAME_TILE_GAP);
        let circle_height = panel_height(circle_grid.height);
        let circle_bounds = Bounds::new(bounds.origin.x, y, width, circle_height);
        draw_panel("Circle segments", circle_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                circle_count,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            );
            for (idx, segments) in circle_segments.iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (FRAME_VARIANT_W + FRAME_TILE_GAP),
                    inner.origin.y + row as f32 * (FRAME_VARIANT_H + FRAME_TILE_GAP),
                    FRAME_VARIANT_W,
                    FRAME_VARIANT_H,
                );
                let label = format!("{segments} seg");
                draw_tile(tile_bounds, &label, cx, |inner, cx| {
                    let mut frame = Frame::new()
                        .style(FrameStyle::Circle)
                        .circle_segments(*segments)
                        .animation_progress(1.0);
                    frame.paint(inset_bounds(inner, 4.0), cx);
                });
            }
        });
    }

    fn paint_arwes_backgrounds(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;
        let available = (width - PANEL_PADDING * 2.0).max(0.0);

        let dots_count = DOT_SHAPES.len() * DOT_ORIGINS.len() * 2;
        let dots_grid = grid_metrics(available, dots_count, BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
        let dots_height = panel_height(dots_grid.height);
        let dots_bounds = Bounds::new(bounds.origin.x, y, width, dots_height);
        draw_panel(
            "DotsGrid permutations (shape x origin x invert)",
            dots_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(inner.size.width, dots_count, BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
                let mut idx = 0;
                for shape in DOT_SHAPES.iter().copied() {
                    for origin in DOT_ORIGINS.iter().copied() {
                        for inverted in [false, true] {
                            let row = idx / grid.cols;
                            let col = idx % grid.cols;
                            let tile_bounds = Bounds::new(
                                inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                                inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                                BG_TILE_W,
                                BG_TILE_H,
                            );
                            let label = format!(
                                "{} {} {}",
                                dot_shape_label(shape),
                                dots_origin_label(origin),
                                if inverted { "Inv" } else { "Norm" }
                            );
                            draw_tile(tile_bounds, &label, cx, |inner, cx| {
                                let mut grid = DotsGrid::new()
                                    .shape(shape)
                                    .origin(origin)
                                    .origin_inverted(inverted)
                                    .distance(20.0)
                                    .size(4.0)
                                    .opacity(1.0)
                                    .color(Hsla::new(180.0, 0.8, 0.5, 0.9));
                                grid.paint(inner, cx);
                            });
                            idx += 1;
                        }
                    }
                }
            },
        );
        y += dots_height + SECTION_GAP;

        let dots_states = [0.0f32, 0.35, 0.7, 1.0];
        let dots_state_count = dots_states.len();
        let dots_state_grid = grid_metrics(available, dots_state_count, BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
        let dots_state_height = panel_height(dots_state_grid.height);
        let dots_state_bounds = Bounds::new(bounds.origin.x, y, width, dots_state_height);
        draw_panel("DotsGrid progress states", dots_state_bounds, cx, |inner, cx| {
            let grid = grid_metrics(inner.size.width, dots_state_count, BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
            for (idx, progress) in dots_states.iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                    inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                    BG_TILE_W,
                    BG_TILE_H,
                );
                let label = format!("{}%", (progress * 100.0) as i32);
                draw_tile(tile_bounds, &label, cx, |inner, cx| {
                    let mut grid = DotsGrid::new()
                        .shape(DotShape::Box)
                        .origin(DotsOrigin::Center)
                        .distance(18.0)
                        .size(5.0)
                        .opacity(1.0)
                        .animation_progress(*progress)
                        .color(Hsla::new(280.0, 0.9, 0.6, 0.95));
                    grid.paint(inner, cx);
                });
            }
        });
        y += dots_state_height + SECTION_GAP;

        let grid_lines_count = 8;
        let grid_lines_grid = grid_metrics(available, grid_lines_count, BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
        let grid_lines_height = panel_height(grid_lines_grid.height);
        let grid_lines_bounds = Bounds::new(bounds.origin.x, y, width, grid_lines_height);
        draw_panel("GridLines permutations (orientation x dash)", grid_lines_bounds, cx, |inner, cx| {
            let grid = grid_metrics(inner.size.width, grid_lines_count, BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
            let orientations = [
                (true, true, "HV"),
                (true, false, "H"),
                (false, true, "V"),
                (false, false, "None"),
            ];
            let dashes = [(Vec::new(), "Solid"), (vec![6.0, 4.0], "Dash")];
            let mut idx = 0;
            for (h, v, label) in orientations {
                for (dash, dash_label) in dashes.iter() {
                    let row = idx / grid.cols;
                    let col = idx % grid.cols;
                    let tile_bounds = Bounds::new(
                        inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                        inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                        BG_TILE_W,
                        BG_TILE_H,
                    );
                    let label = format!("{label} {dash_label}");
                    draw_tile(tile_bounds, &label, cx, |inner, cx| {
                        let mut grid = GridLinesBackground::new()
                            .horizontal(h)
                            .vertical(v)
                            .spacing(24.0)
                            .line_width(2.0)
                            .color(Hsla::new(120.0, 0.7, 0.5, 0.8))
                            .horizontal_dash(dash.clone())
                            .vertical_dash(dash.clone());
                        grid.set_state(AnimatorState::Entered);
                        grid.paint(inner, cx);
                    });
                    idx += 1;
                }
            }
        });
        y += grid_lines_height + SECTION_GAP;

        let moving_count = LINE_DIRECTIONS.len() * 2;
        let moving_grid = grid_metrics(available, moving_count, BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
        let moving_height = panel_height(moving_grid.height);
        let moving_bounds = Bounds::new(bounds.origin.x, y, width, moving_height);
        draw_panel("MovingLines permutations (direction x spacing)", moving_bounds, cx, |inner, cx| {
            let grid = grid_metrics(inner.size.width, moving_count, BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
            let spacings = [30.0, 70.0];
            let mut idx = 0;
            for direction in LINE_DIRECTIONS.iter().copied() {
                for spacing in spacings.iter().copied() {
                    let row = idx / grid.cols;
                    let col = idx % grid.cols;
                    let tile_bounds = Bounds::new(
                        inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                        inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                        BG_TILE_W,
                        BG_TILE_H,
                    );
                    let label = format!("{} {}", line_direction_label(direction), spacing as i32);
                    draw_tile(tile_bounds, &label, cx, |inner, cx| {
                        let mut lines = MovingLinesBackground::new()
                            .direction(direction)
                            .spacing(spacing)
                            .line_width(2.5)
                            .color(Hsla::new(45.0, 0.9, 0.6, 0.85))
                            .sets(5)
                            .cycle_duration(Duration::from_secs(4));
                        lines.update_with_delta(AnimatorState::Entered, Duration::from_millis(600));
                        lines.paint(inner, cx);
                    });
                    idx += 1;
                }
            }
        });
        y += moving_height + SECTION_GAP;

        let puff_presets = 6;
        let puff_grid = grid_metrics(available, puff_presets, BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
        let puff_height = panel_height(puff_grid.height);
        let puff_bounds = Bounds::new(bounds.origin.x, y, width, puff_height);
        draw_panel("Puffs permutations", puff_bounds, cx, |inner, cx| {
            let grid = grid_metrics(inner.size.width, puff_presets, BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
            let presets: Vec<(&str, PuffsBackground)> = vec![
                ("Cyan", PuffsBackground::new()
                    .color(Hsla::new(180.0, 0.9, 0.5, 0.4))
                    .quantity(12)
                    .layers(8)),
                (
                    "Dense Magenta",
                    PuffsBackground::new()
                        .color(Hsla::new(300.0, 0.85, 0.5, 0.35))
                        .quantity(20)
                        .layers(14)
                        .radius_offset((8.0, 70.0)),
                ),
                (
                    "Sparse Blue",
                    PuffsBackground::new()
                        .color(Hsla::new(220.0, 0.8, 0.6, 0.45))
                        .quantity(6)
                        .layers(6)
                        .radius_offset((4.0, 30.0)),
                ),
                (
                    "Warm Orange",
                    PuffsBackground::new()
                        .color(Hsla::new(32.0, 0.95, 0.55, 0.4))
                        .quantity(14)
                        .layers(12),
                ),
                (
                    "Wide Green",
                    PuffsBackground::new()
                        .color(Hsla::new(140.0, 0.8, 0.45, 0.4))
                        .quantity(10)
                        .padding(60.0)
                        .radius_offset((8.0, 55.0)),
                ),
                (
                    "Offset Purple",
                    PuffsBackground::new()
                        .color(Hsla::new(270.0, 0.85, 0.55, 0.4))
                        .quantity(12)
                        .y_offset((-30.0, -100.0))
                        .x_offset((15.0, 50.0)),
                ),
            ];

            for (idx, (label, mut puffs)) in presets.into_iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                    inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                    BG_TILE_W,
                    BG_TILE_H,
                );
                draw_tile(tile_bounds, label, cx, |inner, cx| {
                    puffs.update_with_delta(AnimatorState::Entered, Duration::from_millis(500));
                    puffs.paint(inner, cx);
                });
            }
        });
    }

    fn paint_arwes_text_effects(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;
        let available = (width - PANEL_PADDING * 2.0).max(0.0);

        let sequence_presets = 8;
        let sequence_grid = grid_metrics(available, sequence_presets, TEXT_TILE_W, TEXT_TILE_H, TEXT_TILE_GAP);
        let sequence_height = panel_height(sequence_grid.height);
        let sequence_bounds = Bounds::new(bounds.origin.x, y, width, sequence_height);
        draw_panel("TextSequence permutations", sequence_bounds, cx, |inner, cx| {
            let grid = grid_metrics(inner.size.width, sequence_presets, TEXT_TILE_W, TEXT_TILE_H, TEXT_TILE_GAP);
            let mut items: Vec<(String, TextSequence)> = Vec::new();
            items.push((
                "Normal cursor".to_string(),
                TextSequence::new("Sequence reveal"),
            ));
            items.push((
                "Cursor off".to_string(),
                TextSequence::new("Sequence reveal").show_cursor(false),
            ));
            items.push((
                "Bold".to_string(),
                TextSequence::new("Sequence reveal").bold(),
            ));
            items.push((
                "Italic".to_string(),
                TextSequence::new("Sequence reveal").italic(),
            ));
            items.push((
                "Bold Italic".to_string(),
                TextSequence::new("Sequence reveal").bold_italic(),
            ));
            items.push((
                "Cursor _".to_string(),
                TextSequence::new("Sequence reveal").cursor_char('_'),
            ));
            let mut entering = TextSequence::new("Sequence reveal")
                .timing(TextEffectTiming::new(Duration::from_millis(900), Duration::from_millis(50)));
            entering.update_with_delta(AnimatorState::Entering, Duration::from_millis(350));
            items.push(("Entering".to_string(), entering));
            let mut exiting = TextSequence::new("Sequence reveal");
            exiting.update_with_delta(AnimatorState::Exiting, Duration::from_millis(350));
            items.push(("Exiting".to_string(), exiting));

            for (idx, (label, mut seq)) in items.into_iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (TEXT_TILE_W + TEXT_TILE_GAP),
                    inner.origin.y + row as f32 * (TEXT_TILE_H + TEXT_TILE_GAP),
                    TEXT_TILE_W,
                    TEXT_TILE_H,
                );
                draw_tile(tile_bounds, &label, cx, |inner, cx| {
                    let text_bounds = Bounds::new(
                        inner.origin.x,
                        inner.origin.y + 8.0,
                        inner.size.width,
                        inner.size.height - 8.0,
                    );
                    seq.paint(text_bounds, cx);
                });
            }
        });
        y += sequence_height + SECTION_GAP;

        let decipher_presets = 6;
        let decipher_grid = grid_metrics(available, decipher_presets, TEXT_TILE_W, TEXT_TILE_H, TEXT_TILE_GAP);
        let decipher_height = panel_height(decipher_grid.height);
        let decipher_bounds = Bounds::new(bounds.origin.x, y, width, decipher_height);
        draw_panel("TextDecipher permutations", decipher_bounds, cx, |inner, cx| {
            let grid = grid_metrics(inner.size.width, decipher_presets, TEXT_TILE_W, TEXT_TILE_H, TEXT_TILE_GAP);
            let mut items: Vec<(String, TextDecipher)> = Vec::new();
            let mut default = TextDecipher::new("Decrypting payload");
            default.update_with_delta(AnimatorState::Entering, Duration::from_millis(350));
            items.push(("Default".to_string(), default));

            let mut digits = TextDecipher::new("Decrypting payload")
                .characters("0123456789")
                .scramble_interval(Duration::from_millis(40));
            digits.update_with_delta(AnimatorState::Entering, Duration::from_millis(350));
            items.push(("Digits".to_string(), digits));

            let mut binary = TextDecipher::new("Decrypting payload")
                .characters("01")
                .scramble_interval(Duration::from_millis(25));
            binary.update_with_delta(AnimatorState::Entering, Duration::from_millis(350));
            items.push(("Binary".to_string(), binary));

            let mut slow = TextDecipher::new("Decrypting payload")
                .scramble_interval(Duration::from_millis(120));
            slow.update_with_delta(AnimatorState::Entering, Duration::from_millis(350));
            items.push(("Slow".to_string(), slow));

            let mut bold = TextDecipher::new("Decrypting payload").bold();
            bold.update_with_delta(AnimatorState::Entering, Duration::from_millis(350));
            items.push(("Bold".to_string(), bold));

            let mut exit = TextDecipher::new("Decrypting payload");
            exit.update_with_delta(AnimatorState::Exiting, Duration::from_millis(350));
            items.push(("Exiting".to_string(), exit));

            for (idx, (label, mut decipher)) in items.into_iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (TEXT_TILE_W + TEXT_TILE_GAP),
                    inner.origin.y + row as f32 * (TEXT_TILE_H + TEXT_TILE_GAP),
                    TEXT_TILE_W,
                    TEXT_TILE_H,
                );
                draw_tile(tile_bounds, &label, cx, |inner, cx| {
                    let text_bounds = Bounds::new(
                        inner.origin.x,
                        inner.origin.y + 8.0,
                        inner.size.width,
                        inner.size.height - 8.0,
                    );
                    decipher.paint(text_bounds, cx);
                });
            }
        });
    }

    fn paint_arwes_illuminator(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;
        let available = (width - PANEL_PADDING * 2.0).max(0.0);

        let preset_count = 8;
        let preset_grid = grid_metrics(
            available,
            preset_count,
            ILLUMINATOR_TILE_W,
            ILLUMINATOR_TILE_H,
            ILLUMINATOR_TILE_GAP,
        );
        let preset_height = panel_height(preset_grid.height);
        let preset_bounds = Bounds::new(bounds.origin.x, y, width, preset_height);
        draw_panel("Illuminator presets", preset_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                preset_count,
                ILLUMINATOR_TILE_W,
                ILLUMINATOR_TILE_H,
                ILLUMINATOR_TILE_GAP,
            );
            let presets: Vec<(&str, Illuminator)> = vec![
                ("Default", Illuminator::new()),
                (
                    "Small",
                    Illuminator::new()
                        .radius(40.0)
                        .intensity(0.7)
                        .color(Hsla::new(180.0, 0.6, 0.7, 0.25)),
                ),
                (
                    "Large",
                    Illuminator::new()
                        .radius(90.0)
                        .intensity(1.0)
                        .color(Hsla::new(200.0, 0.7, 0.7, 0.2)),
                ),
                (
                    "Warm",
                    Illuminator::new()
                        .radius(70.0)
                        .intensity(0.9)
                        .color(Hsla::new(25.0, 0.8, 0.6, 0.22)),
                ),
                (
                    "High Rings",
                    Illuminator::new()
                        .radius(70.0)
                        .rings(16)
                        .segments(64)
                        .intensity(0.8),
                ),
                (
                    "Low Rings",
                    Illuminator::new()
                        .radius(70.0)
                        .rings(6)
                        .segments(24)
                        .intensity(0.9),
                ),
                (
                    "Green",
                    Illuminator::new()
                        .radius(60.0)
                        .intensity(0.8)
                        .color(Hsla::new(120.0, 0.7, 0.6, 0.22)),
                ),
                (
                    "Blue",
                    Illuminator::new()
                        .radius(60.0)
                        .intensity(0.8)
                        .color(Hsla::new(210.0, 0.7, 0.6, 0.22)),
                ),
            ];

            for (idx, (label, mut illuminator)) in presets.into_iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (ILLUMINATOR_TILE_W + ILLUMINATOR_TILE_GAP),
                    inner.origin.y + row as f32 * (ILLUMINATOR_TILE_H + ILLUMINATOR_TILE_GAP),
                    ILLUMINATOR_TILE_W,
                    ILLUMINATOR_TILE_H,
                );
                draw_tile(tile_bounds, label, cx, |inner, cx| {
                    let center = Point::new(
                        inner.origin.x + inner.size.width / 2.0,
                        inner.origin.y + inner.size.height / 2.0,
                    );
                    illuminator.snap_to_position(center.x, center.y);
                    illuminator.update_with_delta(AnimatorState::Entered, Duration::from_millis(1));
                    illuminator.paint(inner, cx);
                });
            }
        });
        y += preset_height + SECTION_GAP;

        let state_count = 4;
        let state_grid = grid_metrics(
            available,
            state_count,
            ILLUMINATOR_TILE_W,
            ILLUMINATOR_TILE_H,
            ILLUMINATOR_TILE_GAP,
        );
        let state_height = panel_height(state_grid.height);
        let state_bounds = Bounds::new(bounds.origin.x, y, width, state_height);
        draw_panel("Illuminator states", state_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                state_count,
                ILLUMINATOR_TILE_W,
                ILLUMINATOR_TILE_H,
                ILLUMINATOR_TILE_GAP,
            );
            let states = [
                (AnimatorState::Entering, "Entering"),
                (AnimatorState::Entered, "Entered"),
                (AnimatorState::Exiting, "Exiting"),
                (AnimatorState::Exited, "Exited"),
            ];
            for (idx, (state, label)) in states.iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (ILLUMINATOR_TILE_W + ILLUMINATOR_TILE_GAP),
                    inner.origin.y + row as f32 * (ILLUMINATOR_TILE_H + ILLUMINATOR_TILE_GAP),
                    ILLUMINATOR_TILE_W,
                    ILLUMINATOR_TILE_H,
                );
                draw_tile(tile_bounds, label, cx, |inner, cx| {
                    let center = Point::new(
                        inner.origin.x + inner.size.width / 2.0,
                        inner.origin.y + inner.size.height / 2.0,
                    );
                    let mut illuminator = Illuminator::new().radius(70.0).intensity(0.8);
                    illuminator.snap_to_position(center.x, center.y);
                    illuminator.update_with_delta(*state, Duration::from_millis(350));
                    illuminator.paint(inner, cx);
                });
            }
        });
    }

    fn paint_hud_widgets(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;
        let available = (width - PANEL_PADDING * 2.0).max(0.0);
        let pulse = self.glow_pulse_anim.current_value();

        let scan_presets = [
            ("Tight", 8.0, 18.0, 0.8, 190.0, 0.0),
            ("Wide", 20.0, 24.0, 0.6, 190.0, 0.2),
            ("Soft", 14.0, 30.0, 0.5, 210.0, 0.4),
            ("Amber", 12.0, 22.0, 0.7, 35.0, 0.1),
            ("Emerald", 10.0, 28.0, 0.75, 120.0, 0.3),
            ("Deep", 16.0, 34.0, 0.55, 200.0, 0.55),
        ];

        let scan_grid = grid_metrics(available, scan_presets.len(), BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
        let scan_height = panel_height(scan_grid.height);
        let scan_bounds = Bounds::new(bounds.origin.x, y, width, scan_height);
        draw_panel("Scanline sweeps", scan_bounds, cx, |inner, cx| {
            let grid = grid_metrics(inner.size.width, scan_presets.len(), BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
            for (idx, (label, spacing, scan_width, opacity, hue, offset)) in scan_presets.iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                    inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                    BG_TILE_W,
                    BG_TILE_H,
                );
                let progress = (pulse + *offset).fract();
                draw_tile(tile_bounds, label, cx, |inner, cx| {
                    let mut scanlines = Scanlines::new()
                        .spacing(*spacing)
                        .scan_width(*scan_width)
                        .scan_progress(progress)
                        .opacity(*opacity)
                        .line_color(Hsla::new(*hue, 0.35, 0.6, 0.25))
                        .scan_color(Hsla::new(*hue, 0.8, 0.7, 0.35));
                    scanlines.paint(inner, cx);
                });
            }
        });
        y += scan_height + SECTION_GAP;

        let meter_presets = [
            ("Low 4", 4, 0.2, 190.0),
            ("Med 5", 5, 0.45, 190.0),
            ("High 6", 6, 0.75, 190.0),
            ("Full 8", 8, 1.0, 150.0),
            ("Amber", 6, 0.6, 35.0),
            ("Green", 5, 0.8, 120.0),
        ];

        let meter_grid = grid_metrics(available, meter_presets.len(), BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
        let meter_height = panel_height(meter_grid.height);
        let meter_bounds = Bounds::new(bounds.origin.x, y, width, meter_height);
        draw_panel("Signal meters", meter_bounds, cx, |inner, cx| {
            let grid = grid_metrics(inner.size.width, meter_presets.len(), BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
            for (idx, (label, bars, level, hue)) in meter_presets.iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                    inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                    BG_TILE_W,
                    BG_TILE_H,
                );
                draw_tile(tile_bounds, label, cx, |inner, cx| {
                    let active = Hsla::new(*hue, 0.8, 0.6, 0.9);
                    let inactive = Hsla::new(*hue, 0.25, 0.3, 0.35);
                    let mut meter = SignalMeter::new()
                        .bars(*bars)
                        .level(*level)
                        .gap(3.0)
                        .active_color(active)
                        .inactive_color(inactive);
                    meter.paint(inset_bounds(inner, 8.0), cx);
                });
            }
        });
        y += meter_height + SECTION_GAP;

        let reticle_presets = [
            ("Compact", 18.0, 4.0, 4.0, 8.0, 190.0),
            ("Wide", 32.0, 6.0, 6.0, 12.0, 190.0),
            ("Long", 40.0, 8.0, 4.0, 14.0, 200.0),
            ("Amber", 28.0, 5.0, 8.0, 10.0, 35.0),
            ("Green", 26.0, 6.0, 6.0, 12.0, 120.0),
            ("Offset", 24.0, 10.0, 10.0, 8.0, 160.0),
        ];

        let reticle_grid = grid_metrics(available, reticle_presets.len(), BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
        let reticle_height = panel_height(reticle_grid.height);
        let reticle_bounds = Bounds::new(bounds.origin.x, y, width, reticle_height);
        draw_panel("Reticle variants", reticle_bounds, cx, |inner, cx| {
            let grid = grid_metrics(inner.size.width, reticle_presets.len(), BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
            for (idx, (label, line_length, gap, center, tick, hue)) in reticle_presets.iter().enumerate() {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                    inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                    BG_TILE_W,
                    BG_TILE_H,
                );
                draw_tile(tile_bounds, label, cx, |inner, cx| {
                    let mut reticle = Reticle::new()
                        .line_length(*line_length)
                        .gap(*gap)
                        .center_size(*center)
                        .tick_length(*tick)
                        .color(Hsla::new(*hue, 0.6, 0.6, 0.85));
                    reticle.paint(inset_bounds(inner, 6.0), cx);
                });
            }
        });
    }

    fn paint_light_demo(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;
        let progress = self.light_frame_anim.current_value();
        let glow_pulse = self.glow_pulse_anim.current_value();

        let frames_height = panel_height(LIGHT_DEMO_FRAMES_INNER_H);
        let frames_bounds = Bounds::new(bounds.origin.x, y, width, frames_height);
        draw_panel("Light demo frames", frames_bounds, cx, |inner, cx| {
            let frame_w = ((inner.size.width - 16.0).max(0.0) / 2.0).max(0.0);
            let frame_h = 60.0;
            let left_x = inner.origin.x;
            let right_x = inner.origin.x + frame_w + 8.0;
            let mut row_y = inner.origin.y;

            let white = Hsla::new(0.0, 0.0, 1.0, 1.0);
            let dark_bg = Hsla::new(0.0, 0.0, 0.08, 0.8);
            let muted = Hsla::new(0.0, 0.0, 0.7, 1.0);
            let white_glow = Hsla::new(0.0, 0.0, 1.0, 0.6 * glow_pulse);
            let cyan_glow = Hsla::new(180.0, 1.0, 0.7, 0.5 * glow_pulse);
            let purple_glow = Hsla::new(280.0, 1.0, 0.7, 0.5 * glow_pulse);
            let green_glow = Hsla::new(120.0, 1.0, 0.6, 0.5 * glow_pulse);

            Frame::corners()
                .line_color(white)
                .bg_color(dark_bg)
                .glow_color(white_glow)
                .stroke_width(2.0)
                .corner_length(18.0)
                .animation_mode(FrameAnimation::Fade)
                .animation_progress(progress)
                .paint(Bounds::new(left_x, row_y, frame_w, frame_h), cx);
            let lbl = cx.text.layout(
                "Fade",
                Point::new(left_x + 10.0, row_y + frame_h / 2.0),
                11.0,
                white.with_alpha(progress),
            );
            cx.scene.draw_text(lbl);

            Frame::lines()
                .line_color(white)
                .bg_color(dark_bg)
                .glow_color(cyan_glow)
                .stroke_width(2.0)
                .animation_mode(FrameAnimation::Draw)
                .draw_direction(DrawDirection::CenterOut)
                .animation_progress(progress)
                .paint(Bounds::new(right_x, row_y, frame_w, frame_h), cx);
            let lbl = cx.text.layout(
                "Draw (CenterOut)",
                Point::new(right_x + 10.0, row_y + frame_h / 2.0),
                11.0,
                white,
            );
            cx.scene.draw_text(lbl);

            row_y += frame_h + 10.0;

            Frame::octagon()
                .line_color(white)
                .bg_color(dark_bg)
                .glow_color(purple_glow)
                .stroke_width(2.0)
                .corner_length(14.0)
                .animation_mode(FrameAnimation::Flicker)
                .animation_progress(progress)
                .paint(Bounds::new(left_x, row_y, frame_w, frame_h), cx);
            let lbl = cx.text.layout(
                "Flicker",
                Point::new(left_x + 10.0, row_y + frame_h / 2.0),
                11.0,
                white,
            );
            cx.scene.draw_text(lbl);

            Frame::nefrex()
                .line_color(white)
                .bg_color(dark_bg)
                .glow_color(green_glow)
                .stroke_width(1.5)
                .square_size(12.0)
                .small_line_length(12.0)
                .large_line_length(40.0)
                .corner_config(CornerConfig::all())
                .animation_mode(FrameAnimation::Assemble)
                .animation_progress(progress)
                .paint(Bounds::new(right_x, row_y, frame_w, frame_h), cx);
            let lbl = cx.text.layout(
                "Assemble",
                Point::new(right_x + 10.0, row_y + frame_h / 2.0),
                11.0,
                white,
            );
            cx.scene.draw_text(lbl);

            row_y += frame_h + 10.0;

            Frame::underline()
                .line_color(white)
                .bg_color(dark_bg)
                .glow_color(white_glow)
                .stroke_width(2.0)
                .square_size(12.0)
                .animation_mode(FrameAnimation::Draw)
                .draw_direction(DrawDirection::LeftToRight)
                .animation_progress(progress)
                .paint(Bounds::new(left_x, row_y, frame_w, frame_h), cx);
            let lbl = cx.text.layout(
                "Underline (Draw)",
                Point::new(left_x + 10.0, row_y + frame_h / 2.0),
                11.0,
                white,
            );
            cx.scene.draw_text(lbl);

            Frame::kranox()
                .line_color(white)
                .bg_color(dark_bg)
                .glow_color(cyan_glow)
                .stroke_width(2.0)
                .square_size(10.0)
                .small_line_length(10.0)
                .large_line_length(35.0)
                .animation_mode(FrameAnimation::Draw)
                .draw_direction(DrawDirection::EdgesIn)
                .animation_progress(progress)
                .paint(Bounds::new(right_x, row_y, frame_w, frame_h), cx);
            let lbl = cx.text.layout(
                "Kranox (EdgesIn)",
                Point::new(right_x + 10.0, row_y + frame_h / 2.0),
                11.0,
                white,
            );
            cx.scene.draw_text(lbl);

            row_y += frame_h + 14.0;

            let wallet_bounds = Bounds::new(left_x, row_y, inner.size.width, 80.0);
            Frame::corners()
                .line_color(white)
                .bg_color(dark_bg)
                .glow_color(cyan_glow)
                .stroke_width(2.0)
                .corner_length(24.0)
                .animation_mode(FrameAnimation::Draw)
                .draw_direction(DrawDirection::CenterOut)
                .animation_progress(progress)
                .paint(wallet_bounds, cx);

            let balance_label = cx.text.layout(
                "Balance",
                Point::new(wallet_bounds.origin.x + 20.0, wallet_bounds.origin.y + 14.0),
                11.0,
                muted,
            );
            cx.scene.draw_text(balance_label);

            let font_size = 28.0;
            let symbol_x = wallet_bounds.origin.x + 20.0;
            let symbol_y = wallet_bounds.origin.y + 28.0;
            draw_bitcoin_symbol(cx.scene, cx.text, symbol_x, symbol_y, font_size, white);

            let sats_amount = cx.text.layout(
                "42069",
                Point::new(symbol_x + font_size * 0.55, symbol_y),
                font_size,
                white,
            );
            cx.scene.draw_text(sats_amount);

            let usd_value = cx.text.layout(
                "~ $42.07",
                Point::new(wallet_bounds.origin.x + 20.0, wallet_bounds.origin.y + 60.0),
                13.0,
                muted,
            );
            cx.scene.draw_text(usd_value);
        });

        y += frames_height + SECTION_GAP;

        let hero_height = panel_height(LIGHT_DEMO_HERO_INNER_H);
        let hero_bounds = Bounds::new(bounds.origin.x, y, width, hero_height);
        draw_panel("Light demo hero frame", hero_bounds, cx, |inner, cx| {
            let pane_w = inner.size.width.min(520.0);
            let pane_h = inner.size.height.min(220.0);
            let pane_x = inner.origin.x + (inner.size.width - pane_w) / 2.0;
            let pane_y = inner.origin.y + (inner.size.height - pane_h) / 2.0;
            let text_alpha = ((progress - 0.2) / 0.8).clamp(0.0, 1.0);

            let white = Hsla::new(0.0, 0.0, 1.0, text_alpha);
            let muted = Hsla::new(0.0, 0.0, 0.7, text_alpha);
            let accent = Hsla::new(0.5, 1.0, 0.6, text_alpha);
            let dark_bg = Hsla::new(0.0, 0.0, 0.06, 0.9);
            let cyan_glow = Hsla::new(0.5, 1.0, 0.6, 0.7 * glow_pulse);

            let mut frame = Frame::nefrex()
                .line_color(white)
                .bg_color(dark_bg)
                .glow_color(cyan_glow)
                .stroke_width(2.0)
                .corner_config(CornerConfig::all())
                .square_size(14.0)
                .small_line_length(14.0)
                .large_line_length(50.0)
                .animation_mode(FrameAnimation::Assemble)
                .draw_direction(DrawDirection::CenterOut)
                .animation_progress(progress);
            frame.paint(Bounds::new(pane_x, pane_y, pane_w, pane_h), cx);

            if text_alpha > 0.01 {
                let title = cx.text.layout(
                    "OpenAgents",
                    Point::new(pane_x + 30.0, pane_y + 40.0),
                    32.0,
                    white,
                );
                cx.scene.draw_text(title);

                let subtitle = cx.text.layout(
                    "Decentralized AI Infrastructure",
                    Point::new(pane_x + 30.0, pane_y + 80.0),
                    16.0,
                    muted,
                );
                cx.scene.draw_text(subtitle);

                let body_lines = [
                    "Build autonomous agents",
                    "Deploy on decentralized compute",
                    "Earn Bitcoin for contributions",
                ];
                for (idx, line) in body_lines.iter().enumerate() {
                    let line_y = pane_y + 130.0 + idx as f32 * 28.0;
                    let bullet = cx.text.layout(">", Point::new(pane_x + 30.0, line_y), 14.0, accent);
                    cx.scene.draw_text(bullet);
                    let text = cx.text.layout(line, Point::new(pane_x + 44.0, line_y + 2.0), 13.0, muted);
                    cx.scene.draw_text(text);
                }
            }
        });
    }

    fn paint_toolcall_demo(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let demo_height = panel_height(TOOLCALL_DEMO_INNER_H);
        let panel_bounds = Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, demo_height);
        draw_panel("Toolcall UI demo", panel_bounds, cx, |inner, cx| {
            self.toolcall_demo.paint(inner, cx);
        });
    }

    fn paint_system_ui(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // Tooltip demos
        let tooltip_height = panel_height(180.0);
        let tooltip_bounds = Bounds::new(bounds.origin.x, y, width, tooltip_height);
        draw_panel("Tooltip positions", tooltip_bounds, cx, |inner, cx| {
            let positions = [
                ("Top", TooltipPosition::Top, 0),
                ("Bottom", TooltipPosition::Bottom, 1),
                ("Left", TooltipPosition::Left, 2),
                ("Right", TooltipPosition::Right, 3),
                ("Auto", TooltipPosition::Auto, 4),
            ];
            let tile_w = 140.0;
            let tile_h = 60.0;
            let gap = 16.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor() as usize;

            for (idx, (label, position, _)) in positions.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (tile_w + gap),
                    inner.origin.y + row as f32 * (tile_h + gap),
                    tile_w,
                    tile_h,
                );

                // Draw target button
                let btn_bounds = Bounds::new(
                    tile_bounds.origin.x + tile_w / 2.0 - 40.0,
                    tile_bounds.origin.y + tile_h / 2.0 - 12.0,
                    80.0,
                    24.0,
                );
                cx.scene.draw_quad(
                    Quad::new(btn_bounds)
                        .with_background(theme::bg::MUTED)
                        .with_border(theme::border::DEFAULT, 1.0),
                );
                let btn_text = cx.text.layout(
                    *label,
                    Point::new(btn_bounds.origin.x + 8.0, btn_bounds.origin.y + 6.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(btn_text);

                // Draw tooltip (always visible for demo)
                let mut tooltip = Tooltip::new(format!("Tooltip positioned {}", label.to_lowercase()))
                    .position(*position)
                    .target(btn_bounds);
                tooltip.show();
                tooltip.paint(tile_bounds, cx);
            }
        });
        y += tooltip_height + SECTION_GAP;

        // StatusBar demos
        let status_height = panel_height(120.0);
        let status_bounds = Bounds::new(bounds.origin.x, y, width, status_height);
        draw_panel("StatusBar variants", status_bounds, cx, |inner, cx| {
            // Top status bar
            let top_bar_bounds = Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 32.0);
            let mut top_bar = StatusBar::new()
                .position(StatusBarPosition::Top)
                .height(28.0)
                .items(vec![
                    StatusItem::mode("mode", Mode::Plan).left(),
                    StatusItem::text("file", "src/main.rs").center(),
                    StatusItem::model("model", Model::ClaudeOpus).right(),
                    StatusItem::status("status", Status::Online).right(),
                ]);
            top_bar.paint(top_bar_bounds, cx);

            // Bottom status bar
            let bot_bar_bounds = Bounds::new(inner.origin.x, inner.origin.y + 50.0, inner.size.width, 32.0);
            let mut bot_bar = StatusBar::new()
                .position(StatusBarPosition::Top)
                .height(28.0)
                .items(vec![
                    StatusItem::mode("mode", Mode::Act).left(),
                    StatusItem::text("branch", "main").left(),
                    StatusItem::text("line", "Ln 42, Col 8").center(),
                    StatusItem::status("status", Status::Busy).right(),
                    StatusItem::model("model", Model::ClaudeSonnet).right(),
                ]);
            bot_bar.paint(bot_bar_bounds, cx);
        });
        y += status_height + SECTION_GAP;

        // Notifications demos
        let notif_height = panel_height(260.0);
        let notif_bounds = Bounds::new(bounds.origin.x, y, width, notif_height);
        draw_panel("Notification levels", notif_bounds, cx, |inner, cx| {
            let levels = [
                ("Info", NotificationLevel::Info, "System update available"),
                ("Success", NotificationLevel::Success, "Build completed successfully"),
                ("Warning", NotificationLevel::Warning, "Deprecated API usage detected"),
                ("Error", NotificationLevel::Error, "Connection to server failed"),
            ];

            let notif_w = 320.0;
            let notif_h = 50.0;
            let gap = 12.0;

            for (idx, (title, level, message)) in levels.iter().enumerate() {
                let notif_bounds = Bounds::new(
                    inner.origin.x,
                    inner.origin.y + idx as f32 * (notif_h + gap),
                    notif_w,
                    notif_h,
                );

                // Draw notification preview manually
                cx.scene.draw_quad(
                    Quad::new(notif_bounds)
                        .with_background(theme::bg::SURFACE)
                        .with_border(level.color(), 2.0),
                );

                let icon_run = cx.text.layout(
                    level.icon(),
                    Point::new(notif_bounds.origin.x + 10.0, notif_bounds.origin.y + 10.0),
                    theme::font_size::LG,
                    level.color(),
                );
                cx.scene.draw_text(icon_run);

                let title_run = cx.text.layout(
                    *title,
                    Point::new(notif_bounds.origin.x + 40.0, notif_bounds.origin.y + 10.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(title_run);

                let msg_run = cx.text.layout(
                    *message,
                    Point::new(notif_bounds.origin.x + 40.0, notif_bounds.origin.y + 28.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(msg_run);
            }
        });
        y += notif_height + SECTION_GAP;

        // ContextMenu demo
        let menu_height = panel_height(200.0);
        let menu_bounds = Bounds::new(bounds.origin.x, y, width, menu_height);
        draw_panel("ContextMenu preview", menu_bounds, cx, |inner, cx| {
            // Draw a static preview of a context menu
            let menu_w = 180.0;
            let menu_h = 160.0;
            let menu_bounds = Bounds::new(inner.origin.x + 20.0, inner.origin.y + 10.0, menu_w, menu_h);

            cx.scene.draw_quad(
                Quad::new(menu_bounds)
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::border::DEFAULT, 1.0),
            );

            let items = [
                ("Cut", Some("Cmd+X"), false, false),
                ("Copy", Some("Cmd+C"), false, false),
                ("Paste", Some("Cmd+V"), true, false),
                ("---", None, false, true),
                ("Select All", Some("Cmd+A"), false, false),
            ];

            let item_h = 28.0;
            let sep_h = 9.0;
            let mut item_y = menu_bounds.origin.y + 4.0;

            for (idx, (label, shortcut, disabled, is_sep)) in items.iter().enumerate() {
                if *is_sep {
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(
                            menu_bounds.origin.x + 4.0,
                            item_y + sep_h / 2.0,
                            menu_w - 8.0,
                            1.0,
                        ))
                        .with_background(theme::border::DEFAULT),
                    );
                    item_y += sep_h;
                    continue;
                }

                let is_hovered = idx == 1; // Highlight "Copy"
                if is_hovered {
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(menu_bounds.origin.x + 4.0, item_y, menu_w - 8.0, item_h))
                            .with_background(theme::bg::MUTED),
                    );
                }

                let text_color = if *disabled { theme::text::MUTED } else { theme::text::PRIMARY };
                let label_run = cx.text.layout(
                    *label,
                    Point::new(menu_bounds.origin.x + 12.0, item_y + 8.0),
                    theme::font_size::SM,
                    text_color,
                );
                cx.scene.draw_text(label_run);

                if let Some(sc) = shortcut {
                    let sc_run = cx.text.layout(
                        *sc,
                        Point::new(menu_bounds.origin.x + menu_w - 60.0, item_y + 8.0),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(sc_run);
                }

                item_y += item_h;
            }

            // Description
            let desc = cx.text.layout(
                "Right-click context menu with shortcuts",
                Point::new(inner.origin.x + menu_w + 40.0, inner.origin.y + 80.0),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(desc);
        });
        y += menu_height + SECTION_GAP;

        // CommandPalette demo
        let palette_height = panel_height(240.0);
        let palette_bounds = Bounds::new(bounds.origin.x, y, width, palette_height);
        draw_panel("CommandPalette preview", palette_bounds, cx, |inner, cx| {
            let palette_w = 400.0;
            let palette_h = 200.0;
            let palette_x = inner.origin.x + (inner.size.width - palette_w) / 2.0;
            let palette_y = inner.origin.y + 10.0;

            // Palette container
            cx.scene.draw_quad(
                Quad::new(Bounds::new(palette_x, palette_y, palette_w, palette_h))
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::accent::PRIMARY, 1.0),
            );

            // Search input
            let input_h = 36.0;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(palette_x + 8.0, palette_y + 8.0, palette_w - 16.0, input_h))
                    .with_background(theme::bg::SURFACE)
                    .with_border(theme::border::DEFAULT, 1.0),
            );
            let search_text = cx.text.layout(
                "file",
                Point::new(palette_x + 16.0, palette_y + 18.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(search_text);

            // Command list
            let commands = [
                ("file.new", "New File", "Cmd+N"),
                ("file.open", "Open File", "Cmd+O"),
                ("file.save", "Save", "Cmd+S"),
                ("file.close", "Close Tab", "Cmd+W"),
            ];

            let item_h = 36.0;
            let list_y = palette_y + input_h + 16.0;

            for (idx, (id, label, shortcut)) in commands.iter().enumerate() {
                let item_y = list_y + idx as f32 * item_h;
                let is_selected = idx == 0;

                if is_selected {
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(palette_x + 4.0, item_y, palette_w - 8.0, item_h))
                            .with_background(theme::bg::MUTED),
                    );
                }

                let label_run = cx.text.layout(
                    *label,
                    Point::new(palette_x + 16.0, item_y + 10.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(label_run);

                let id_run = cx.text.layout(
                    *id,
                    Point::new(palette_x + 16.0, item_y + 24.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(id_run);

                let shortcut_run = cx.text.layout(
                    *shortcut,
                    Point::new(palette_x + palette_w - 70.0, item_y + 12.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(shortcut_run);
            }
        });
    }

    fn paint_chat_threads(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;
        let entry_gap = 12.0;

        // ========== Panel 1: Simple Conversation ==========
        let simple_height = panel_height(480.0);
        let simple_bounds = Bounds::new(bounds.origin.x, y, width, simple_height);
        draw_panel("Simple Conversation", simple_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            // User message
            let user_h = 100.0;
            let mut user_msg = UserMessage::new("Can you help me understand how async/await works in Rust?")
                .timestamp("10:30 AM");
            user_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, user_h), cx);
            entry_y += user_h + entry_gap;

            // Assistant response
            let asst_h = 180.0;
            let mut asst_msg = AssistantMessage::new(
                "Async/await in Rust allows you to write asynchronous code that looks synchronous. \
                 The key concepts are:\n\n\
                 1. `async fn` - declares a function that returns a Future\n\
                 2. `.await` - suspends execution until the Future completes\n\
                 3. An executor (like tokio) runs these Futures to completion\n\n\
                 Would you like me to show you a practical example?"
            )
            .model(Model::ClaudeSonnet)
            .timestamp("10:30 AM");
            asst_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, asst_h), cx);
            entry_y += asst_h + entry_gap;

            // Follow-up user message
            let user2_h = 80.0;
            let mut user_msg2 = UserMessage::new("Yes please! Show me a simple HTTP request example.")
                .timestamp("10:31 AM");
            user_msg2.paint(Bounds::new(inner.origin.x, entry_y, entry_w, user2_h), cx);
        });
        y += simple_height + SECTION_GAP;

        // ========== Panel 2: Multi-Tool Workflow ==========
        let multi_height = panel_height(600.0);
        let multi_bounds = Bounds::new(bounds.origin.x, y, width, multi_height);
        draw_panel("Multi-Tool Workflow", multi_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            // User request
            let user_h = 80.0;
            let mut user_msg = UserMessage::new("Find all TODO comments in the codebase and list them")
                .timestamp("2:15 PM");
            user_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, user_h), cx);
            entry_y += user_h + entry_gap;

            // Assistant thinking + tool call
            let asst_h = 60.0;
            let mut asst_msg = AssistantMessage::new("I'll search the codebase for TODO comments.")
                .model(Model::ClaudeSonnet)
                .timestamp("2:15 PM");
            asst_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, asst_h), cx);
            entry_y += asst_h + entry_gap;

            // Search tool call
            let search_h = 200.0;
            let mut search_tool = SearchToolCall::new("TODO")
                .status(ToolStatus::Success)
                .matches(vec![
                    SearchMatch { file: "src/main.rs".into(), line: 42, content: "TODO: Add error handling".into() },
                    SearchMatch { file: "src/lib.rs".into(), line: 78, content: "TODO: Implement caching".into() },
                    SearchMatch { file: "src/utils.rs".into(), line: 15, content: "TODO: Refactor this function".into() },
                    SearchMatch { file: "tests/integration.rs".into(), line: 23, content: "TODO: Add more test cases".into() },
                ]);
            search_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, search_h), cx);
            entry_y += search_h + entry_gap;

            // Terminal tool for file count
            let term_h = 100.0;
            let mut term_tool = TerminalToolCall::new("wc -l $(grep -rl 'TODO' src/)")
                .status(ToolStatus::Success)
                .exit_code(0)
                .output("  42 src/main.rs\n  78 src/lib.rs\n  15 src/utils.rs\n 135 total");
            term_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, term_h), cx);
            entry_y += term_h + entry_gap;

            // Final summary
            let summary_h = 80.0;
            let mut summary = AssistantMessage::new(
                "Found 4 TODO comments across 3 source files totaling 135 lines. \
                 The main areas needing attention are error handling, caching, and test coverage."
            )
            .model(Model::ClaudeSonnet)
            .timestamp("2:16 PM");
            summary.paint(Bounds::new(inner.origin.x, entry_y, entry_w, summary_h), cx);
        });
        y += multi_height + SECTION_GAP;

        // ========== Panel 3: Code Editing Session ==========
        let edit_height = panel_height(520.0);
        let edit_bounds = Bounds::new(bounds.origin.x, y, width, edit_height);
        draw_panel("Code Editing Session", edit_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            // User request
            let user_h = 80.0;
            let mut user_msg = UserMessage::new("Add error handling to the process_data function")
                .timestamp("3:42 PM");
            user_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, user_h), cx);
            entry_y += user_h + entry_gap;

            // Read tool
            let read_h = 100.0;
            let mut read_tool = ToolCallCard::new(ToolType::Read, "read_file")
                .status(ToolStatus::Success)
                .input("path: src/processor.rs")
                .output("Read 156 lines (4.2 KB)");
            read_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, read_h), cx);
            entry_y += read_h + entry_gap;

            // Diff tool showing the edit
            let diff_h = 180.0;
            let mut diff_tool = DiffToolCall::new("src/processor.rs")
                .status(ToolStatus::Success)
                .diff_type(DiffType::Unified)
                .lines(vec![
                    DiffLine { kind: DiffLineKind::Header, content: "@@ -45,6 +45,12 @@".into(), old_line: None, new_line: None },
                    DiffLine { kind: DiffLineKind::Context, content: "fn process_data(input: &str) -> String {".into(), old_line: Some(45), new_line: Some(45) },
                    DiffLine { kind: DiffLineKind::Deletion, content: "    input.parse().unwrap()".into(), old_line: Some(46), new_line: None },
                    DiffLine { kind: DiffLineKind::Addition, content: "    match input.parse() {".into(), old_line: None, new_line: Some(46) },
                    DiffLine { kind: DiffLineKind::Addition, content: "        Ok(val) => val,".into(), old_line: None, new_line: Some(47) },
                    DiffLine { kind: DiffLineKind::Addition, content: "        Err(e) => {".into(), old_line: None, new_line: Some(48) },
                    DiffLine { kind: DiffLineKind::Addition, content: "            eprintln!(\"Parse error: {}\", e);".into(), old_line: None, new_line: Some(49) },
                    DiffLine { kind: DiffLineKind::Addition, content: "            String::new()".into(), old_line: None, new_line: Some(50) },
                    DiffLine { kind: DiffLineKind::Addition, content: "        }".into(), old_line: None, new_line: Some(51) },
                    DiffLine { kind: DiffLineKind::Addition, content: "    }".into(), old_line: None, new_line: Some(52) },
                    DiffLine { kind: DiffLineKind::Context, content: "}".into(), old_line: Some(47), new_line: Some(53) },
                ]);
            diff_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, diff_h), cx);
            entry_y += diff_h + entry_gap;

            // Completion message
            let complete_h = 80.0;
            let mut complete = AssistantMessage::new(
                "I've added proper error handling with a match statement. The function now logs \
                 parse errors to stderr and returns an empty string instead of panicking."
            )
            .model(Model::ClaudeSonnet)
            .timestamp("3:43 PM");
            complete.paint(Bounds::new(inner.origin.x, entry_y, entry_w, complete_h), cx);
        });
        y += edit_height + SECTION_GAP;

        // ========== Panel 4: Search & Navigation ==========
        let search_height = panel_height(440.0);
        let search_bounds = Bounds::new(bounds.origin.x, y, width, search_height);
        draw_panel("Search & Navigation", search_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            // User question
            let user_h = 80.0;
            let mut user_msg = UserMessage::new("Where is the authentication logic implemented?")
                .timestamp("11:05 AM");
            user_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, user_h), cx);
            entry_y += user_h + entry_gap;

            // Glob search
            let glob_h = 100.0;
            let mut glob_tool = ToolCallCard::new(ToolType::Search, "glob")
                .status(ToolStatus::Success)
                .input("pattern: **/auth*.rs")
                .output("Found 3 files");
            glob_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, glob_h), cx);
            entry_y += glob_h + entry_gap;

            // Grep search
            let grep_h = 180.0;
            let mut grep_tool = SearchToolCall::new("fn authenticate")
                .status(ToolStatus::Success)
                .matches(vec![
                    SearchMatch { file: "src/auth/mod.rs".into(), line: 12, content: "pub fn authenticate(token: &str) -> Result<User, AuthError>".into() },
                    SearchMatch { file: "src/auth/jwt.rs".into(), line: 45, content: "fn authenticate_jwt(token: &str) -> Result<Claims, JwtError>".into() },
                    SearchMatch { file: "src/middleware/auth.rs".into(), line: 28, content: "async fn authenticate(req: Request) -> Result<Response, Error>".into() },
                ]);
            grep_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, grep_h), cx);
            entry_y += grep_h + entry_gap;

            // Answer
            let answer_h = 60.0;
            let mut answer = AssistantMessage::new(
                "Authentication is handled in `src/auth/mod.rs:12` with JWT validation in `src/auth/jwt.rs`."
            )
            .model(Model::ClaudeSonnet)
            .timestamp("11:05 AM");
            answer.paint(Bounds::new(inner.origin.x, entry_y, entry_w, answer_h), cx);
        });
        y += search_height + SECTION_GAP;

        // ========== Panel 5: Streaming Response ==========
        let stream_height = panel_height(320.0);
        let stream_bounds = Bounds::new(bounds.origin.x, y, width, stream_height);
        draw_panel("Streaming Response", stream_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            // User message
            let user_h = 80.0;
            let mut user_msg = UserMessage::new("Explain the visitor pattern in software design")
                .timestamp("4:20 PM");
            user_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, user_h), cx);
            entry_y += user_h + entry_gap;

            // Streaming assistant response
            let stream_h = 200.0;
            let mut stream_msg = AssistantMessage::new(
                "The Visitor pattern is a behavioral design pattern that lets you separate algorithms \
                 from the objects they operate on. It's useful when you have a complex object structure \
                 and want to perform operations on it without modifying the classes.\n\n\
                 Key components:\n\
                 - **Element**: objects being visited\n\
                 - **Visitor**: defines operations..."
            )
            .model(Model::ClaudeOpus)
            .streaming(true)
            .timestamp("4:20 PM");
            stream_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, stream_h), cx);
        });
        y += stream_height + SECTION_GAP;

        // ========== Panel 6: Complex Agent Session ==========
        let complex_height = panel_height(800.0);
        let complex_bounds = Bounds::new(bounds.origin.x, y, width, complex_height);
        draw_panel("Complex Agent Session", complex_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            // User request
            let user_h = 100.0;
            let mut user_msg = UserMessage::new(
                "Create a new API endpoint for user preferences with GET and POST methods. \
                 Include input validation and proper error responses."
            )
            .timestamp("9:00 AM");
            user_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, user_h), cx);
            entry_y += user_h + entry_gap;

            // Planning response
            let plan_h = 100.0;
            let mut plan = AssistantMessage::new(
                "I'll create the preferences endpoint. Here's my plan:\n\
                 1. Create preference types and validation\n\
                 2. Add route handlers for GET/POST\n\
                 3. Write integration tests\n\
                 4. Update API documentation"
            )
            .model(Model::ClaudeSonnet)
            .timestamp("9:00 AM");
            plan.paint(Bounds::new(inner.origin.x, entry_y, entry_w, plan_h), cx);
            entry_y += plan_h + entry_gap;

            // Write tool - creating new file
            let write_h = 100.0;
            let mut write_tool = ToolCallCard::new(ToolType::Write, "write_file")
                .status(ToolStatus::Success)
                .input("path: src/api/preferences.rs")
                .output("Created file (78 lines)");
            write_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, write_h), cx);
            entry_y += write_h + entry_gap;

            // Edit existing file
            let edit_h = 100.0;
            let mut edit_tool = ToolCallCard::new(ToolType::Edit, "edit_file")
                .status(ToolStatus::Success)
                .input("path: src/api/mod.rs")
                .output("Added route registration (+3 lines)");
            edit_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, edit_h), cx);
            entry_y += edit_h + entry_gap;

            // Run tests
            let test_h = 120.0;
            let mut test_tool = TerminalToolCall::new("cargo test preferences")
                .status(ToolStatus::Success)
                .exit_code(0)
                .output("running 5 tests\ntest api::preferences::tests::test_get_prefs ... ok\n\
                         test api::preferences::tests::test_post_prefs ... ok\n\
                         test api::preferences::tests::test_validation ... ok\n\n\
                         test result: ok. 5 passed; 0 failed");
            test_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, test_h), cx);
            entry_y += test_h + entry_gap;

            // Documentation update
            let doc_h = 100.0;
            let mut doc_tool = ToolCallCard::new(ToolType::Edit, "edit_file")
                .status(ToolStatus::Success)
                .input("path: docs/api.md")
                .output("Updated API docs (+45 lines)");
            doc_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, doc_h), cx);
            entry_y += doc_h + entry_gap;

            // Summary
            let summary_h = 100.0;
            let mut summary = AssistantMessage::new(
                "The preferences API is now complete:\n\
                 - `GET /api/preferences` returns user preferences\n\
                 - `POST /api/preferences` updates them with validation\n\
                 - All 5 tests pass and documentation is updated"
            )
            .model(Model::ClaudeSonnet)
            .timestamp("9:03 AM");
            summary.paint(Bounds::new(inner.origin.x, entry_y, entry_w, summary_h), cx);
        });
        y += complex_height + SECTION_GAP;

        // ========== Panel 7: Error Handling ==========
        let error_height = panel_height(280.0);
        let error_bounds = Bounds::new(bounds.origin.x, y, width, error_height);
        draw_panel("Error Handling", error_bounds, cx, |inner, cx| {
            let mut entry_y = inner.origin.y;
            let entry_w = inner.size.width;

            // User request
            let user_h = 60.0;
            let mut user_msg = UserMessage::new("Run the database migration script")
                .timestamp("5:30 PM");
            user_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, user_h), cx);
            entry_y += user_h + entry_gap;

            // Failed terminal command
            let term_h = 100.0;
            let mut term_tool = TerminalToolCall::new("./scripts/migrate.sh")
                .status(ToolStatus::Error)
                .exit_code(1)
                .output("Error: Connection refused\nDatabase server not running at localhost:5432");
            term_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, term_h), cx);
            entry_y += term_h + entry_gap;

            // Error response
            let error_h = 80.0;
            let mut error_msg = AssistantMessage::new(
                "The migration failed because the database server isn't running. \
                 Please start PostgreSQL with `sudo systemctl start postgresql` and try again."
            )
            .model(Model::ClaudeSonnet)
            .timestamp("5:30 PM");
            error_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, error_h), cx);
        });
    }

    fn paint_bitcoin_wallet(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Payment Method Icons ==========
        let methods_height = panel_height(200.0);
        let methods_bounds = Bounds::new(bounds.origin.x, y, width, methods_height);
        draw_panel("Payment Method Icons", methods_bounds, cx, |inner, cx| {
            let methods = [
                PaymentMethod::Lightning,
                PaymentMethod::Spark,
                PaymentMethod::OnChain,
                PaymentMethod::Token,
                PaymentMethod::Deposit,
                PaymentMethod::Withdraw,
            ];

            let tile_w = 140.0;
            let tile_h = 50.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, method) in methods.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Draw tile background
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(tile_x, tile_y, tile_w, tile_h))
                        .with_background(theme::bg::MUTED)
                        .with_border(method.color(), 1.0),
                );

                // Draw icon with label
                let mut icon = PaymentMethodIcon::new(*method).size(24.0).show_label(true);
                icon.paint(Bounds::new(tile_x + 12.0, tile_y + 14.0, tile_w - 24.0, 24.0), cx);
            }
        });
        y += methods_height + SECTION_GAP;

        // ========== Panel 2: Payment Status Badges ==========
        let status_height = panel_height(180.0);
        let status_bounds = Bounds::new(bounds.origin.x, y, width, status_height);
        draw_panel("Payment Status Badges", status_bounds, cx, |inner, cx| {
            let statuses = [
                (PaymentStatus::Pending, "Awaiting confirmation..."),
                (PaymentStatus::Completed, "Successfully sent!"),
                (PaymentStatus::Failed, "Transaction rejected"),
                (PaymentStatus::Expired, "Invoice expired"),
            ];

            let tile_w = 200.0;
            let tile_h = 60.0;
            let gap = 16.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (status, desc)) in statuses.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                cx.scene.draw_quad(
                    Quad::new(Bounds::new(tile_x, tile_y, tile_w, tile_h))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                // Status badge
                let mut badge = PaymentStatusBadge::new(*status);
                badge.paint(Bounds::new(tile_x + 8.0, tile_y + 8.0, 72.0, 20.0), cx);

                // Description
                let desc_run = cx.text.layout(
                    *desc,
                    Point::new(tile_x + 8.0, tile_y + 36.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(desc_run);
            }
        });
        y += status_height + SECTION_GAP;

        // ========== Panel 3: Network Badges ==========
        let network_height = panel_height(160.0);
        let network_bounds = Bounds::new(bounds.origin.x, y, width, network_height);
        draw_panel("Bitcoin Networks", network_bounds, cx, |inner, cx| {
            let networks = [
                (BitcoinNetwork::Mainnet, "Production - Real money!"),
                (BitcoinNetwork::Testnet, "Testing - Free test sats"),
                (BitcoinNetwork::Signet, "Staging - Controlled testnet"),
                (BitcoinNetwork::Regtest, "Local dev - Private network"),
            ];

            let tile_w = 220.0;
            let tile_h = 48.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (network, desc)) in networks.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                cx.scene.draw_quad(
                    Quad::new(Bounds::new(tile_x, tile_y, tile_w, tile_h))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                let mut badge = NetworkBadge::new(*network);
                badge.paint(Bounds::new(tile_x + 8.0, tile_y + 14.0, 64.0, 20.0), cx);

                let desc_run = cx.text.layout(
                    *desc,
                    Point::new(tile_x + 80.0, tile_y + 16.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(desc_run);
            }
        });
        y += network_height + SECTION_GAP;

        // ========== Panel 4: Bitcoin Amounts ==========
        let amounts_height = panel_height(200.0);
        let amounts_bounds = Bounds::new(bounds.origin.x, y, width, amounts_height);
        draw_panel("Bitcoin Amount Formatting", amounts_bounds, cx, |inner, cx| {
            let amounts_data = [
                (1000, AmountDirection::Neutral, BitcoinUnit::Sats, "Small amount"),
                (50000, AmountDirection::Incoming, BitcoinUnit::Sats, "Incoming payment"),
                (25000, AmountDirection::Outgoing, BitcoinUnit::Sats, "Outgoing payment"),
                (100_000_000, AmountDirection::Neutral, BitcoinUnit::Btc, "One Bitcoin"),
                (2_100_000_000_000_000, AmountDirection::Neutral, BitcoinUnit::Btc, "Max supply"),
            ];

            let row_h = 32.0;
            let gap = 8.0;

            for (idx, (sats, direction, unit, label)) in amounts_data.iter().enumerate() {
                let row_y = inner.origin.y + idx as f32 * (row_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(inner.origin.x, row_y + 8.0),
                    theme::font_size::SM,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Amount
                let mut amount = BitcoinAmount::new(*sats)
                    .direction(*direction)
                    .unit(*unit)
                    .font_size(theme::font_size::LG);
                amount.paint(Bounds::new(inner.origin.x + 180.0, row_y, 200.0, row_h), cx);
            }
        });
        y += amounts_height + SECTION_GAP;

        // ========== Panel 5: Balance Cards ==========
        let balance_height = panel_height(220.0);
        let balance_bounds = Bounds::new(bounds.origin.x, y, width, balance_height);
        draw_panel("Wallet Balance Cards", balance_bounds, cx, |inner, cx| {
            // Mainnet balance
            let mainnet_balance = WalletBalance::new(150000, 75000, 25000);
            let mut mainnet_card = BalanceCard::new(mainnet_balance)
                .network(BitcoinNetwork::Mainnet)
                .show_breakdown(true);
            mainnet_card.paint(Bounds::new(inner.origin.x, inner.origin.y, 300.0, 180.0), cx);

            // Testnet balance
            let testnet_balance = WalletBalance::new(1_000_000, 500_000, 0);
            let mut testnet_card = BalanceCard::new(testnet_balance)
                .network(BitcoinNetwork::Testnet)
                .show_breakdown(true);
            let card_x = inner.origin.x + 320.0;
            if card_x + 300.0 <= inner.origin.x + inner.size.width {
                testnet_card.paint(Bounds::new(card_x, inner.origin.y, 300.0, 180.0), cx);
            }
        });
        y += balance_height + SECTION_GAP;

        // ========== Panel 6: Payment Rows (Transaction History) ==========
        let txn_height = panel_height(300.0);
        let txn_bounds = Bounds::new(bounds.origin.x, y, width, txn_height);
        draw_panel("Transaction History", txn_bounds, cx, |inner, cx| {
            let transactions = [
                PaymentInfo::new("tx1", 50000, PaymentDirection::Receive)
                    .method(PaymentMethod::Lightning)
                    .status(PaymentStatus::Completed)
                    .timestamp("Dec 25, 10:30 AM")
                    .description("Zap from @alice"),
                PaymentInfo::new("tx2", 25000, PaymentDirection::Send)
                    .method(PaymentMethod::Spark)
                    .status(PaymentStatus::Completed)
                    .fee(10)
                    .timestamp("Dec 24, 3:15 PM")
                    .description("Coffee payment"),
                PaymentInfo::new("tx3", 100000, PaymentDirection::Receive)
                    .method(PaymentMethod::OnChain)
                    .status(PaymentStatus::Pending)
                    .timestamp("Dec 24, 1:00 PM")
                    .description("On-chain deposit"),
                PaymentInfo::new("tx4", 15000, PaymentDirection::Send)
                    .method(PaymentMethod::Lightning)
                    .status(PaymentStatus::Failed)
                    .timestamp("Dec 23, 8:45 PM")
                    .description("Invoice expired"),
            ];

            let row_h = 60.0;
            let gap = 8.0;

            for (idx, payment) in transactions.iter().enumerate() {
                let row_y = inner.origin.y + idx as f32 * (row_h + gap);
                let mut row = PaymentRow::new(payment.clone());
                row.paint(Bounds::new(inner.origin.x, row_y, inner.size.width, row_h), cx);
            }
        });
        y += txn_height + SECTION_GAP;

        // ========== Panel 7: Invoice Displays ==========
        let invoice_height = panel_height(320.0);
        let invoice_bounds = Bounds::new(bounds.origin.x, y, width, invoice_height);
        draw_panel("Invoice & Address Displays", invoice_bounds, cx, |inner, cx| {
            // Lightning invoice
            let ln_invoice = InvoiceInfo::new(
                InvoiceType::Bolt11,
                "lnbc500u1pn9xnxhpp5e5wfyknkdxqmz9f0vs4j8kqz3h5qf7c4xhp2s5ngrqj6u4m8qz",
            )
            .amount(50000)
            .description("Payment for services")
            .expiry("10 minutes")
            .status(PaymentStatus::Pending);
            let mut ln_display = InvoiceDisplay::new(ln_invoice).show_qr(true);
            ln_display.paint(Bounds::new(inner.origin.x, inner.origin.y, 320.0, 280.0), cx);

            // Spark address (compact)
            let spark_addr = InvoiceInfo::new(
                InvoiceType::SparkAddress,
                "sp1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
            )
            .status(PaymentStatus::Pending);
            let mut spark_display = InvoiceDisplay::new(spark_addr).show_qr(false).compact(true);
            let spark_x = inner.origin.x + 340.0;
            if spark_x + 320.0 <= inner.origin.x + inner.size.width {
                spark_display.paint(Bounds::new(spark_x, inner.origin.y, 320.0, 120.0), cx);
            }

            // Bitcoin address
            let btc_addr = InvoiceInfo::new(
                InvoiceType::OnChainAddress,
                "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
            )
            .status(PaymentStatus::Pending);
            let mut btc_display = InvoiceDisplay::new(btc_addr).show_qr(false).compact(true);
            if spark_x + 320.0 <= inner.origin.x + inner.size.width {
                btc_display.paint(Bounds::new(spark_x, inner.origin.y + 140.0, 320.0, 120.0), cx);
            }
        });
        y += invoice_height + SECTION_GAP;

        // ========== Panel 8: Complete Wallet Dashboard ==========
        let dashboard_height = panel_height(400.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel("Complete Wallet Dashboard", dashboard_bounds, cx, |inner, cx| {
            // Left column: Balance card
            let col_w = (inner.size.width - 20.0) / 2.0;

            let balance = WalletBalance::new(250000, 100000, 50000);
            let mut balance_card = BalanceCard::new(balance)
                .network(BitcoinNetwork::Mainnet)
                .show_breakdown(true);
            balance_card.paint(Bounds::new(inner.origin.x, inner.origin.y, col_w.min(320.0), 180.0), cx);

            // Below balance: Quick actions hints
            let actions_y = inner.origin.y + 200.0;
            let actions = ["Send Payment", "Receive", "History", "Settings"];
            let btn_w = 100.0;
            let btn_h = 32.0;
            let btn_gap = 12.0;

            for (idx, action) in actions.iter().enumerate() {
                let btn_x = inner.origin.x + idx as f32 * (btn_w + btn_gap);
                if btn_x + btn_w > inner.origin.x + col_w {
                    break;
                }

                cx.scene.draw_quad(
                    Quad::new(Bounds::new(btn_x, actions_y, btn_w, btn_h))
                        .with_background(theme::bg::MUTED)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                let btn_text = cx.text.layout(
                    *action,
                    Point::new(btn_x + 8.0, actions_y + 8.0),
                    theme::font_size::XS,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(btn_text);
            }

            // Right column: Recent transactions
            let right_x = inner.origin.x + col_w + 20.0;
            if right_x + col_w <= inner.origin.x + inner.size.width {
                let header_run = cx.text.layout(
                    "Recent Transactions",
                    Point::new(right_x, inner.origin.y),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(header_run);

                let recent = [
                    PaymentInfo::new("r1", 10000, PaymentDirection::Receive)
                        .method(PaymentMethod::Lightning)
                        .status(PaymentStatus::Completed)
                        .timestamp("Just now"),
                    PaymentInfo::new("r2", 5000, PaymentDirection::Send)
                        .method(PaymentMethod::Spark)
                        .status(PaymentStatus::Completed)
                        .timestamp("5 min ago"),
                    PaymentInfo::new("r3", 75000, PaymentDirection::Receive)
                        .method(PaymentMethod::OnChain)
                        .status(PaymentStatus::Pending)
                        .timestamp("1 hour ago"),
                ];

                let row_h = 56.0;
                let gap = 4.0;
                let txn_y = inner.origin.y + 28.0;

                for (idx, payment) in recent.iter().enumerate() {
                    let row_y = txn_y + idx as f32 * (row_h + gap);
                    let mut row = PaymentRow::new(payment.clone()).show_fee(false);
                    row.paint(Bounds::new(right_x, row_y, col_w.min(400.0), row_h), cx);
                }
            }
        });
    }
}

struct FocusDemo {
    items: Vec<&'static str>,
    focused: usize,
    active: Vec<bool>,
    hovered: Option<usize>,
}

impl FocusDemo {
    fn new() -> Self {
        let items = vec!["Focus A", "Focus B", "Focus C"];
        Self {
            focused: 0,
            active: vec![false; items.len()],
            items,
            hovered: None,
        }
    }

    fn paint(&self, bounds: Bounds, cx: &mut PaintContext) {
        let hint_height = 18.0;
        let mut hint = Text::new("Tab or Shift+Tab to move. Enter toggles.")
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED);
        hint.paint(
            Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, hint_height),
            cx,
        );

        let items_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + hint_height + 10.0,
            bounds.size.width,
            bounds.size.height - hint_height - 10.0,
        );
        let gap = 12.0;
        let item_width =
            ((items_bounds.size.width - gap * (self.items.len() as f32 - 1.0))
                / self.items.len() as f32)
                .max(0.0);
        let item_height = 36.0;

        for (index, label) in self.items.iter().enumerate() {
            let x = items_bounds.origin.x + index as f32 * (item_width + gap);
            let y = items_bounds.origin.y;
            let item_bounds = Bounds::new(x, y, item_width, item_height);

            let is_focused = self.focused == index;
            let is_active = self.active[index];
            let is_hovered = self.hovered == Some(index);

            let border = if is_focused {
                theme::accent::PRIMARY
            } else {
                theme::border::DEFAULT
            };
            let bg = if is_active {
                theme::accent::PRIMARY.with_alpha(0.2)
            } else if is_hovered {
                theme::bg::HOVER
            } else {
                theme::bg::SURFACE
            };

            cx.scene.draw_quad(
                Quad::new(item_bounds)
                    .with_background(bg)
                    .with_border(border, 1.0),
            );

            let mut text = Text::new(*label)
                .font_size(theme::font_size::SM)
                .color(theme::text::PRIMARY);
            text.paint(
                Bounds::new(
                    item_bounds.origin.x + 8.0,
                    item_bounds.origin.y + 8.0,
                    item_bounds.size.width - 16.0,
                    item_bounds.size.height,
                ),
                cx,
            );
        }
    }

    fn handle_event(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
        let hint_height = 18.0;
        let items_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + hint_height + 10.0,
            bounds.size.width,
            bounds.size.height - hint_height - 10.0,
        );
        let gap = 12.0;
        let item_width =
            ((items_bounds.size.width - gap * (self.items.len() as f32 - 1.0))
                / self.items.len() as f32)
                .max(0.0);
        let item_height = 36.0;

        match event {
            InputEvent::MouseMove { x, y } => {
                let mut hover = None;
                for i in 0..self.items.len() {
                    let item_bounds = Bounds::new(
                        items_bounds.origin.x + i as f32 * (item_width + gap),
                        items_bounds.origin.y,
                        item_width,
                        item_height,
                    );
                    if item_bounds.contains(Point::new(*x, *y)) {
                        hover = Some(i);
                        break;
                    }
                }
                if hover != self.hovered {
                    self.hovered = hover;
                    return true;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left {
                    for i in 0..self.items.len() {
                        let item_bounds = Bounds::new(
                            items_bounds.origin.x + i as f32 * (item_width + gap),
                            items_bounds.origin.y,
                            item_width,
                            item_height,
                        );
                        if item_bounds.contains(Point::new(*x, *y)) {
                            self.focused = i;
                            self.active[i] = !self.active[i];
                            return true;
                        }
                    }
                }
            }
            InputEvent::KeyDown { key, modifiers } => {
                match key {
                    Key::Named(NamedKey::Tab) => {
                        if modifiers.shift {
                            self.focused =
                                (self.focused + self.items.len() - 1) % self.items.len();
                        } else {
                            self.focused = (self.focused + 1) % self.items.len();
                        }
                        return true;
                    }
                    Key::Named(NamedKey::Enter) => {
                        self.active[self.focused] = !self.active[self.focused];
                        return true;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        false
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)]
enum PanePriority {
    Background,
    Normal,
    Elevated,
    Urgent,
    Critical,
}

impl PanePriority {
    fn glow_color(&self) -> Option<Hsla> {
        match self {
            PanePriority::Background | PanePriority::Normal => None,
            PanePriority::Elevated => Some(Hsla::new(0.5, 1.0, 0.6, 0.8)),
            PanePriority::Urgent => Some(Hsla::new(0.125, 1.0, 0.5, 0.9)),
            PanePriority::Critical => Some(Hsla::new(0.0, 1.0, 0.5, 1.0)),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum PaneState {
    Creating,
    Open,
    Minimized,
    Closing,
}

struct ToolcallPane {
    title: String,
    target_x: f32,
    target_y: f32,
    target_w: f32,
    target_h: f32,
    x_anim: Animation<f32>,
    y_anim: Animation<f32>,
    w_anim: Animation<f32>,
    h_anim: Animation<f32>,
    alpha_anim: Animation<f32>,
    priority: PanePriority,
    custom_glow: Option<Hsla>,
    frame_style: FrameStyle,
    frame_animation: FrameAnimation,
    draw_direction: DrawDirection,
    state: PaneState,
    z_index: i32,
    shake: SpringAnimation<f32>,
    shake_target: f32,
    shake_phase: u8,
    content_type: String,
}

impl ToolcallPane {
    fn new(_id: &str, title: &str, x: f32, y: f32, w: f32, h: f32) -> Self {
        let x_anim = Animation::new(x, x, Duration::from_millis(500))
            .easing(Easing::EaseOutCubic);
        let y_anim = Animation::new(y, y, Duration::from_millis(500))
            .easing(Easing::EaseOutCubic);
        let w_anim = Animation::new(w, w, Duration::from_millis(300))
            .easing(Easing::EaseOutCubic);
        let h_anim = Animation::new(h, h, Duration::from_millis(300))
            .easing(Easing::EaseOutCubic);
        let mut alpha_anim = Animation::new(0.0, 1.0, Duration::from_millis(400))
            .easing(Easing::EaseOut);
        alpha_anim.start();

        Self {
            title: title.to_string(),
            target_x: x,
            target_y: y,
            target_w: w,
            target_h: h,
            x_anim,
            y_anim,
            w_anim,
            h_anim,
            alpha_anim,
            priority: PanePriority::Normal,
            custom_glow: None,
            frame_style: FrameStyle::Corners,
            frame_animation: FrameAnimation::Fade,
            draw_direction: DrawDirection::CenterOut,
            state: PaneState::Creating,
            z_index: 0,
            shake: SpringAnimation::new(0.0, 0.0).stiffness(300.0).damping(10.0),
            shake_target: 0.0,
            shake_phase: 0,
            content_type: "generic".to_string(),
        }
    }

    fn move_to(&mut self, x: f32, y: f32, animate: bool) {
        self.target_x = x;
        self.target_y = y;
        if animate {
            self.x_anim = Animation::new(self.x_anim.current_value(), x, Duration::from_millis(400))
                .easing(Easing::EaseInOutCubic);
            self.y_anim = Animation::new(self.y_anim.current_value(), y, Duration::from_millis(400))
                .easing(Easing::EaseInOutCubic);
            self.x_anim.start();
            self.y_anim.start();
        }
    }

    fn resize_to(&mut self, w: f32, h: f32, animate: bool) {
        self.target_w = w;
        self.target_h = h;
        if animate {
            self.w_anim = Animation::new(self.w_anim.current_value(), w, Duration::from_millis(300))
                .easing(Easing::EaseInOutCubic);
            self.h_anim = Animation::new(self.h_anim.current_value(), h, Duration::from_millis(300))
                .easing(Easing::EaseInOutCubic);
            self.w_anim.start();
            self.h_anim.start();
        }
    }

    fn set_priority(&mut self, priority: PanePriority) {
        self.priority = priority;
    }

    fn set_glow(&mut self, color: Option<Hsla>) {
        self.custom_glow = color;
    }

    fn request_attention(&mut self) {
        self.shake_phase = 1;
        self.shake_target = 15.0;
        self.shake.set_target(15.0);
    }

    fn minimize(&mut self) {
        self.state = PaneState::Minimized;
        self.h_anim = Animation::new(self.h_anim.current_value(), 30.0, Duration::from_millis(300))
            .easing(Easing::EaseInOutCubic);
        self.h_anim.start();
    }

    fn close(&mut self) {
        self.state = PaneState::Closing;
        self.alpha_anim = Animation::new(1.0, 0.0, Duration::from_millis(300))
            .easing(Easing::EaseIn);
        self.alpha_anim.start();
    }

    fn tick(&mut self, dt: Duration) {
        self.x_anim.tick(dt);
        self.y_anim.tick(dt);
        self.w_anim.tick(dt);
        self.h_anim.tick(dt);
        self.alpha_anim.tick(dt);
        self.shake.tick(dt);

        if self.shake_phase > 0 && self.shake.is_settled() {
            match self.shake_phase {
                1 => {
                    self.shake_target = -12.0;
                    self.shake.set_target(-12.0);
                    self.shake_phase = 2;
                }
                2 => {
                    self.shake_target = 0.0;
                    self.shake.set_target(0.0);
                    self.shake_phase = 3;
                }
                _ => {
                    self.shake_phase = 0;
                }
            }
        }

        if self.state == PaneState::Creating && self.alpha_anim.is_finished() {
            self.state = PaneState::Open;
        }
    }

    fn current_bounds(&self) -> Bounds {
        let shake_offset = if self.shake_phase > 0 { self.shake.current() } else { 0.0 };
        Bounds::new(
            self.x_anim.current_value() + shake_offset,
            self.y_anim.current_value(),
            self.w_anim.current_value(),
            self.h_anim.current_value(),
        )
    }

    fn glow_color(&self) -> Option<Hsla> {
        self.custom_glow.or_else(|| self.priority.glow_color())
    }

    fn is_visible(&self) -> bool {
        self.alpha_anim.current_value() > 0.01
    }
}

struct ToolCallLog {
    entries: Vec<(f32, String)>,
    max_entries: usize,
}

impl ToolCallLog {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 8,
        }
    }

    fn add(&mut self, time: f32, msg: String) {
        self.entries.push((time, msg));
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
    }
}

struct ToolcallDemo {
    panes: HashMap<String, ToolcallPane>,
    z_counter: i32,
    tool_log: ToolCallLog,
    elapsed: f32,
    scenario_index: usize,
    dots_anim: Animation<f32>,
    frame_anim: Animation<f32>,
}

impl ToolcallDemo {
    fn new() -> Self {
        let (dots_anim, frame_anim) = toolcall_animations();
        Self {
            panes: HashMap::new(),
            z_counter: 0,
            tool_log: ToolCallLog::new(),
            elapsed: 0.0,
            scenario_index: 0,
            dots_anim,
            frame_anim,
        }
    }

    fn tick(&mut self, dt: Duration) {
        self.elapsed += dt.as_secs_f32();
        self.run_script();
        self.dots_anim.tick(dt);
        self.frame_anim.tick(dt);
        for pane in self.panes.values_mut() {
            pane.tick(dt);
        }
        self.panes.retain(|_, pane| pane.is_visible() || pane.state != PaneState::Closing);
    }

    fn paint(&self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.push_clip(bounds);
        cx.scene.draw_quad(Quad::new(bounds).with_background(theme::bg::APP));

        let width = bounds.size.width;
        let height = bounds.size.height;
        let origin = bounds.origin;

        let dots_progress = self.dots_anim.current_value();
        let dots_height = (height - 180.0).max(0.0);
        let mut dots_grid = DotsGrid::new()
            .color(Hsla::new(0.0, 0.0, 0.3, 0.25))
            .shape(DotShape::Cross)
            .distance(28.0)
            .size(5.0)
            .cross_thickness(1.0)
            .origin(DotsOrigin::Center)
            .easing(Easing::EaseOut)
            .animation_progress(dots_progress);
        dots_grid.paint(
            Bounds::new(origin.x, origin.y + 40.0, width, dots_height),
            cx,
        );

        let title = cx.text.layout(
            "Toolcall UI Demo",
            Point::new(origin.x + 20.0, origin.y + 18.0),
            16.0,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title);

        let subtitle = cx.text.layout(
            "Auto-animated panes + glow",
            Point::new(origin.x + width - 250.0, origin.y + 22.0),
            11.0,
            theme::text::MUTED,
        );
        cx.scene.draw_text(subtitle);

        cx.scene.draw_quad(
            Quad::new(Bounds::new(origin.x, origin.y + 40.0, width, 2.0))
                .with_background(theme::accent::PRIMARY.with_alpha(0.5)),
        );

        let mut panes: Vec<_> = self.panes.values().collect();
        panes.sort_by_key(|pane| pane.z_index);

        for pane in panes {
            if !pane.is_visible() {
                continue;
            }

            let bounds = pane.current_bounds();
            let bounds = Bounds::new(
                bounds.origin.x + origin.x,
                bounds.origin.y + origin.y,
                bounds.size.width,
                bounds.size.height,
            );
            let alpha = pane.alpha_anim.current_value();

            let white = Hsla::new(0.0, 0.0, 1.0, alpha);
            let dark_bg = Hsla::new(0.0, 0.0, 0.08, 0.85 * alpha);
            let muted = Hsla::new(0.0, 0.0, 0.6, alpha);

            let glow = pane.glow_color().map(|c| c.with_alpha(c.a * alpha));
            let frame_progress = self.frame_anim.current_value();
            let mut frame = demo_frame(pane.frame_style)
                .line_color(white)
                .bg_color(dark_bg)
                .stroke_width(2.0)
                .animation_mode(pane.frame_animation)
                .draw_direction(pane.draw_direction)
                .animation_progress(frame_progress);

            if let Some(glow) = glow {
                frame = frame.glow_color(glow);
            }

            frame.paint(bounds, cx);

            let title_run = cx.text.layout(
                &pane.title,
                Point::new(bounds.origin.x + 12.0, bounds.origin.y + 14.0),
                13.0,
                white,
            );
            cx.scene.draw_text(title_run);

            let type_run = cx.text.layout(
                &pane.content_type,
                Point::new(bounds.origin.x + 12.0, bounds.origin.y + 32.0),
                10.0,
                muted,
            );
            cx.scene.draw_text(type_run);

            if pane.state == PaneState::Minimized {
                continue;
            }

            let content_y = bounds.origin.y + 50.0;
            let content_h = bounds.size.height - 60.0;
            if content_h > 20.0 {
                let content_color = Hsla::new(0.0, 0.0, 0.15, 0.5 * alpha);
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        bounds.origin.x + 8.0,
                        content_y,
                        bounds.size.width - 16.0,
                        content_h,
                    ))
                    .with_background(content_color),
                );

                let placeholder = match pane.content_type.as_str() {
                    "code" => "fn main() {\n    println!(\"Hello\");\n}",
                    "terminal" => "$ cargo test\n   Compiling...\n   Finished",
                    "chat" => "AI: How can I help?\nUser: Fix the bug",
                    "diagnostics" => "error[E0308]: mismatched types\n  --> src/main.rs:42",
                    _ => "Content placeholder",
                };
                let text_run = cx.text.layout(
                    placeholder,
                    Point::new(bounds.origin.x + 14.0, content_y + 8.0),
                    11.0,
                    Hsla::new(0.0, 0.0, 0.7, alpha),
                );
                cx.scene.draw_text(text_run);
            }
        }

        let log_h = 130.0;
        let log_y = origin.y + height - 140.0;
        let log_bounds = Bounds::new(origin.x, log_y, width, log_h);

        if log_bounds.size.height > 0.0 {
            cx.scene.draw_quad(
                Quad::new(log_bounds).with_background(Hsla::new(0.0, 0.0, 0.05, 0.95)),
            );
            cx.scene.draw_quad(
                Quad::new(Bounds::new(origin.x, log_y, width, 1.0))
                    .with_background(theme::accent::PRIMARY.with_alpha(0.3)),
            );

            let log_title = cx.text.layout(
                "Tool Call Log",
                Point::new(origin.x + 15.0, log_y + 10.0),
                12.0,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(log_title);

            let mut entry_y = log_y + 30.0;
            for (time, msg) in &self.tool_log.entries {
                let time_str = format!("[{time:.1}s]");
                let time_run = cx.text.layout(
                    &time_str,
                    Point::new(origin.x + 15.0, entry_y),
                    11.0,
                    theme::accent::PRIMARY,
                );
                cx.scene.draw_text(time_run);

                let msg_run = cx.text.layout(
                    &format!("ui_pane.{}", msg),
                    Point::new(origin.x + 70.0, entry_y),
                    11.0,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(msg_run);

                entry_y += 14.0;
            }
        }

        cx.scene.pop_clip();
    }

    fn run_script(&mut self) {
        let t = self.elapsed;

        if self.scenario_index == 0 && t >= 0.5 {
            self.create_pane("editor", "Code Editor", 50.0, 60.0, 450.0, 250.0, "code");
            self.scenario_index = 1;
        }
        if self.scenario_index == 1 && t >= 1.0 {
            self.create_pane("terminal", "Terminal", 50.0, 340.0, 450.0, 180.0, "terminal");
            self.scenario_index = 2;
        }
        if self.scenario_index == 2 && t >= 1.5 {
            self.create_pane("chat", "AI Assistant", 540.0, 60.0, 340.0, 230.0, "chat");
            self.scenario_index = 3;
        }
        if self.scenario_index == 3 && t >= 2.0 {
            self.create_pane("diagnostics", "Diagnostics", 540.0, 320.0, 340.0, 200.0, "diagnostics");
            self.scenario_index = 4;
        }

        if self.scenario_index == 4 && t >= 3.0 {
            self.set_priority("diagnostics", PanePriority::Urgent);
            self.scenario_index = 5;
        }
        if self.scenario_index == 5 && t >= 3.3 {
            self.focus_pane("diagnostics");
            self.scenario_index = 6;
        }
        if self.scenario_index == 6 && t >= 3.6 {
            if let Some(pane) = self.panes.get_mut("diagnostics") {
                pane.request_attention();
            }
            self.tool_log.add(t, "Animate { id: \"diagnostics\", animation: \"Pulse\" }".to_string());
            self.scenario_index = 7;
        }

        if self.scenario_index == 7 && t >= 5.0 {
            self.set_priority("diagnostics", PanePriority::Normal);
            self.set_glow("diagnostics", None);
            self.scenario_index = 8;
        }
        if self.scenario_index == 8 && t >= 5.3 {
            self.focus_pane("editor");
            self.set_glow("editor", Some(Hsla::new(0.389, 1.0, 0.5, 0.8)));
            self.scenario_index = 9;
        }

        if self.scenario_index == 9 && t >= 6.5 {
            self.move_pane("terminal", 50.0, 330.0);
            self.scenario_index = 10;
        }
        if self.scenario_index == 10 && t >= 6.8 {
            self.resize_pane("terminal", 500.0, 270.0);
            self.scenario_index = 11;
        }
        if self.scenario_index == 11 && t >= 7.1 {
            self.set_priority("terminal", PanePriority::Elevated);
            self.focus_pane("terminal");
            self.scenario_index = 12;
        }

        if self.scenario_index == 12 && t >= 8.5 {
            self.minimize_pane("terminal");
            self.scenario_index = 13;
        }
        if self.scenario_index == 13 && t >= 9.0 {
            self.request_attention("chat", "All tests passed!");
            self.scenario_index = 14;
        }

        if self.scenario_index == 14 && t >= 11.0 {
            self.close_pane("diagnostics");
            self.scenario_index = 15;
        }
        if self.scenario_index == 15 && t >= 13.0 {
            self.reset();
        }
    }

    fn reset(&mut self) {
        let (dots_anim, frame_anim) = toolcall_animations();
        self.panes.clear();
        self.z_counter = 0;
        self.tool_log = ToolCallLog::new();
        self.elapsed = 0.0;
        self.scenario_index = 0;
        self.dots_anim = dots_anim;
        self.frame_anim = frame_anim;
    }

    fn create_pane(
        &mut self,
        id: &str,
        title: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        content_type: &str,
    ) {
        let mut pane = ToolcallPane::new(id, title, x, y, w, h);
        pane.content_type = content_type.to_string();

        match content_type {
            "code" => {
                pane.frame_style = FrameStyle::Corners;
                pane.frame_animation = FrameAnimation::Draw;
                pane.draw_direction = DrawDirection::CenterOut;
            }
            "terminal" => {
                pane.frame_style = FrameStyle::Lines;
                pane.frame_animation = FrameAnimation::Draw;
                pane.draw_direction = DrawDirection::LeftToRight;
            }
            "chat" => {
                pane.frame_style = FrameStyle::Nefrex;
                pane.frame_animation = FrameAnimation::Assemble;
                pane.draw_direction = DrawDirection::CenterOut;
            }
            "diagnostics" => {
                pane.frame_style = FrameStyle::Octagon;
                pane.frame_animation = FrameAnimation::Flicker;
                pane.draw_direction = DrawDirection::EdgesIn;
            }
            _ => {}
        }

        self.z_counter += 1;
        pane.z_index = self.z_counter;
        self.panes.insert(id.to_string(), pane);
        self.tool_log.add(
            self.elapsed,
            format!("CreatePane {{ id: \"{}\", title: \"{}\" }}", id, title),
        );
    }

    fn focus_pane(&mut self, id: &str) {
        self.z_counter += 1;
        if let Some(pane) = self.panes.get_mut(id) {
            pane.z_index = self.z_counter;
        }
        self.tool_log
            .add(self.elapsed, format!("Focus {{ id: \"{}\" }}", id));
    }

    fn set_priority(&mut self, id: &str, priority: PanePriority) {
        if let Some(pane) = self.panes.get_mut(id) {
            pane.set_priority(priority);
        }
        let p_str = match priority {
            PanePriority::Background => "Background",
            PanePriority::Normal => "Normal",
            PanePriority::Elevated => "Elevated",
            PanePriority::Urgent => "Urgent",
            PanePriority::Critical => "Critical",
        };
        self.tool_log.add(
            self.elapsed,
            format!("SetPriority {{ id: \"{}\", priority: \"{}\" }}", id, p_str),
        );
    }

    fn set_glow(&mut self, id: &str, color: Option<Hsla>) {
        if let Some(pane) = self.panes.get_mut(id) {
            pane.set_glow(color);
        }
        let color_str = color
            .map(|c| format!("#{:02x}{:02x}{:02x}", (c.l * 255.0) as u8, (c.s * 255.0) as u8, (c.h as u8)))
            .unwrap_or_else(|| "none".to_string());
        self.tool_log.add(
            self.elapsed,
            format!("SetGlow {{ id: \"{}\", color: \"{}\" }}", id, color_str),
        );
    }

    fn move_pane(&mut self, id: &str, x: f32, y: f32) {
        if let Some(pane) = self.panes.get_mut(id) {
            pane.move_to(x, y, true);
        }
        self.tool_log.add(
            self.elapsed,
            format!("MovePane {{ id: \"{}\", x: {}, y: {} }}", id, x, y),
        );
    }

    fn resize_pane(&mut self, id: &str, w: f32, h: f32) {
        if let Some(pane) = self.panes.get_mut(id) {
            pane.resize_to(w, h, true);
        }
        self.tool_log.add(
            self.elapsed,
            format!("ResizePane {{ id: \"{}\", w: {}, h: {} }}", id, w, h),
        );
    }

    fn request_attention(&mut self, id: &str, msg: &str) {
        if let Some(pane) = self.panes.get_mut(id) {
            pane.request_attention();
            pane.set_priority(PanePriority::Urgent);
        }
        self.focus_pane(id);
        self.tool_log.add(
            self.elapsed,
            format!("RequestAttention {{ id: \"{}\", msg: \"{}\" }}", id, msg),
        );
    }

    fn minimize_pane(&mut self, id: &str) {
        if let Some(pane) = self.panes.get_mut(id) {
            pane.minimize();
        }
        self.tool_log.add(
            self.elapsed,
            format!("SetState {{ id: \"{}\", state: \"Minimized\" }}", id),
        );
    }

    fn close_pane(&mut self, id: &str) {
        if let Some(pane) = self.panes.get_mut(id) {
            pane.close();
        }
        self.tool_log
            .add(self.elapsed, format!("ClosePane {{ id: \"{}\" }}", id));
    }
}

fn draw_panel(
    title: &str,
    bounds: Bounds,
    cx: &mut PaintContext,
    paint: impl FnOnce(Bounds, &mut PaintContext),
) {
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let title_bounds = Bounds::new(
        bounds.origin.x + PANEL_PADDING,
        bounds.origin.y + PANEL_PADDING,
        bounds.size.width - PANEL_PADDING * 2.0,
        24.0,
    );
    let mut title_text = Text::new(title)
        .font_size(theme::font_size::BASE)
        .color(theme::text::PRIMARY);
    title_text.paint(title_bounds, cx);

    let inner = Bounds::new(
        bounds.origin.x + PANEL_PADDING,
        bounds.origin.y + PANEL_PADDING + 28.0,
        (bounds.size.width - PANEL_PADDING * 2.0).max(0.0),
        (bounds.size.height - PANEL_PADDING * 2.0 - 28.0).max(0.0),
    );
    paint(inner, cx);
}

fn panel_inner(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x + PANEL_PADDING,
        bounds.origin.y + PANEL_PADDING + 28.0,
        (bounds.size.width - PANEL_PADDING * 2.0).max(0.0),
        (bounds.size.height - PANEL_PADDING * 2.0 - 28.0).max(0.0),
    )
}

fn draw_tile(
    bounds: Bounds,
    label: &str,
    cx: &mut PaintContext,
    paint: impl FnOnce(Bounds, &mut PaintContext),
) {
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let mut text = Text::new(label)
        .font_size(theme::font_size::SM)
        .color(theme::text::MUTED);
    text.paint(
        Bounds::new(
            bounds.origin.x + 10.0,
            bounds.origin.y + 8.0,
            bounds.size.width - 20.0,
            20.0,
        ),
        cx,
    );

    let inner = Bounds::new(
        bounds.origin.x + 10.0,
        bounds.origin.y + 32.0,
        (bounds.size.width - 20.0).max(0.0),
        (bounds.size.height - 42.0).max(0.0),
    );
    paint(inner, cx);
}

fn paint_centered(component: &mut impl wgpui::Component, bounds: Bounds, cx: &mut PaintContext) {
    let (w, h) = component.size_hint();
    let width = w.unwrap_or(bounds.size.width).min(bounds.size.width);
    let height = h.unwrap_or(bounds.size.height).min(bounds.size.height);
    let x = bounds.origin.x + (bounds.size.width - width) / 2.0;
    let y = bounds.origin.y + (bounds.size.height - height) / 2.0;
    component.paint(Bounds::new(x, y, width, height), cx);
}

fn center_bounds(bounds: Bounds, width: f32, height: f32) -> Bounds {
    Bounds::new(
        bounds.origin.x + (bounds.size.width - width) / 2.0,
        bounds.origin.y + (bounds.size.height - height) / 2.0,
        width.min(bounds.size.width),
        height.min(bounds.size.height),
    )
}

fn component_event(
    component: &mut impl wgpui::Component,
    event: &InputEvent,
    bounds: Bounds,
    cx: &mut EventContext,
) -> bool {
    matches!(component.event(event, bounds, cx), EventResult::Handled)
}

struct GridMetrics {
    cols: usize,
    height: f32,
}

fn grid_metrics(width: f32, items: usize, tile_w: f32, tile_h: f32, gap: f32) -> GridMetrics {
    let cols = (((width + gap) / (tile_w + gap)).floor() as usize).max(1);
    let rows = if items == 0 {
        0
    } else {
        (items + cols - 1) / cols
    };
    let height = if rows == 0 {
        0.0
    } else {
        rows as f32 * tile_h + (rows as f32 - 1.0) * gap
    };
    GridMetrics { cols, height }
}

fn panel_height(inner_height: f32) -> f32 {
    inner_height + PANEL_PADDING * 2.0 + 28.0
}

fn toolcall_animations() -> (Animation<f32>, Animation<f32>) {
    let mut dots_anim = Animation::new(0.0_f32, 1.0, Duration::from_millis(2000))
        .easing(Easing::Linear)
        .iterations(0)
        .alternate();
    dots_anim.start();

    let mut frame_anim = Animation::new(0.0_f32, 1.0, Duration::from_millis(2500))
        .easing(Easing::EaseInOutCubic)
        .iterations(0)
        .alternate();
    frame_anim.start();

    (dots_anim, frame_anim)
}

fn stacked_height(panels: &[f32]) -> f32 {
    let mut height = 0.0;
    for (idx, panel) in panels.iter().enumerate() {
        if idx > 0 {
            height += SECTION_GAP;
        }
        height += *panel;
    }
    height
}

fn inset_bounds(bounds: Bounds, inset: f32) -> Bounds {
    Bounds::new(
        bounds.origin.x + inset,
        bounds.origin.y + inset,
        (bounds.size.width - inset * 2.0).max(0.0),
        (bounds.size.height - inset * 2.0).max(0.0),
    )
}

fn draw_bitcoin_symbol(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    x: f32,
    y: f32,
    font_size: f32,
    color: Hsla,
) {
    let bar_h = font_size * 0.18;
    let bar_w = 2.0;
    let bar_x1 = x + font_size * 0.15;
    let bar_x2 = x + font_size * 0.38;

    scene.draw_quad(Quad::new(Bounds::new(bar_x1, y - bar_h + 2.0, bar_w, bar_h)).with_background(color));
    scene.draw_quad(Quad::new(Bounds::new(bar_x2, y - bar_h + 2.0, bar_w, bar_h)).with_background(color));
    scene.draw_quad(Quad::new(Bounds::new(bar_x1, y + font_size - 4.0, bar_w, bar_h)).with_background(color));
    scene.draw_quad(Quad::new(Bounds::new(bar_x2, y + font_size - 4.0, bar_w, bar_h)).with_background(color));

    let b = text_system.layout("B", Point::new(x, y), font_size, color);
    scene.draw_text(b);
}

fn frame_style_short(style: FrameStyle) -> &'static str {
    match style {
        FrameStyle::Corners => "Crn",
        FrameStyle::Lines => "Lin",
        FrameStyle::Octagon => "Oct",
        FrameStyle::Underline => "Und",
        FrameStyle::Nefrex => "Nef",
        FrameStyle::Kranox => "Krn",
        FrameStyle::Nero => "Nro",
        FrameStyle::Header => "Hdr",
        FrameStyle::Circle => "Cir",
    }
}

fn frame_style_label(style: FrameStyle) -> &'static str {
    match style {
        FrameStyle::Corners => "Corners",
        FrameStyle::Lines => "Lines",
        FrameStyle::Octagon => "Octagon",
        FrameStyle::Underline => "Underline",
        FrameStyle::Nefrex => "Nefrex",
        FrameStyle::Kranox => "Kranox",
        FrameStyle::Nero => "Nero",
        FrameStyle::Header => "Header",
        FrameStyle::Circle => "Circle",
    }
}

fn demo_frame(style: FrameStyle) -> Frame {
    match style {
        FrameStyle::Corners => Frame::corners().corner_length(18.0),
        FrameStyle::Lines => Frame::lines(),
        FrameStyle::Octagon => Frame::octagon().corner_length(14.0),
        FrameStyle::Underline => Frame::underline().square_size(12.0),
        FrameStyle::Nefrex => Frame::nefrex()
            .corner_config(CornerConfig::diagonal())
            .square_size(10.0)
            .small_line_length(10.0)
            .large_line_length(35.0),
        FrameStyle::Kranox => Frame::kranox()
            .square_size(10.0)
            .small_line_length(10.0)
            .large_line_length(35.0),
        FrameStyle::Nero => Frame::nero().corner_length(20.0),
        FrameStyle::Header => Frame::header().corner_length(12.0).header_bottom(true),
        FrameStyle::Circle => Frame::circle().circle_segments(48),
    }
}

fn frame_animation_label(animation: FrameAnimation) -> &'static str {
    match animation {
        FrameAnimation::Fade => "Fade",
        FrameAnimation::Draw => "Draw",
        FrameAnimation::Flicker => "Flicker",
        FrameAnimation::Assemble => "Asm",
    }
}

fn draw_direction_label(direction: DrawDirection) -> &'static str {
    match direction {
        DrawDirection::LeftToRight => "L->R",
        DrawDirection::RightToLeft => "R->L",
        DrawDirection::TopToBottom => "T->B",
        DrawDirection::BottomToTop => "B->T",
        DrawDirection::CenterOut => "Center",
        DrawDirection::EdgesIn => "Edges",
    }
}

fn dot_shape_label(shape: DotShape) -> &'static str {
    match shape {
        DotShape::Box => "Box",
        DotShape::Circle => "Circle",
        DotShape::Cross => "Cross",
    }
}

fn dots_origin_label(origin: DotsOrigin) -> &'static str {
    match origin {
        DotsOrigin::Left => "L",
        DotsOrigin::Right => "R",
        DotsOrigin::Top => "T",
        DotsOrigin::Bottom => "B",
        DotsOrigin::Center => "C",
        DotsOrigin::Point(_, _) => "P",
    }
}

fn line_direction_label(direction: LineDirection) -> &'static str {
    match direction {
        LineDirection::Right => "Right",
        LineDirection::Left => "Left",
        LineDirection::Down => "Down",
        LineDirection::Up => "Up",
    }
}

fn arwes_frames_height(bounds: Bounds) -> f32 {
    let available = (bounds.size.width - PANEL_PADDING * 2.0).max(0.0);
    let permutations = FRAME_STYLES.len() * FRAME_ANIMATIONS.len() * FRAME_DIRECTIONS.len();
    let glow_palette = FRAME_STYLES.len() * FRAME_ANIMATIONS.len() * GLOW_PRESETS.len();
    let panels = [
        panel_height(grid_metrics(available, permutations, FRAME_TILE_W, FRAME_TILE_H, FRAME_TILE_GAP).height),
        panel_height(grid_metrics(available, FRAME_STYLES.len() * 2, FRAME_VARIANT_W, FRAME_VARIANT_H, FRAME_TILE_GAP).height),
        panel_height(grid_metrics(available, FRAME_STYLES.len() * 2, FRAME_VARIANT_W, FRAME_VARIANT_H, FRAME_TILE_GAP).height),
        panel_height(grid_metrics(available, glow_palette, FRAME_VARIANT_W, FRAME_VARIANT_H, FRAME_TILE_GAP).height),
        panel_height(grid_metrics(available, 16, FRAME_VARIANT_W, FRAME_VARIANT_H, FRAME_TILE_GAP).height),
        panel_height(grid_metrics(available, 2, FRAME_VARIANT_W, FRAME_VARIANT_H, FRAME_TILE_GAP).height),
        panel_height(grid_metrics(available, 4, FRAME_VARIANT_W, FRAME_VARIANT_H, FRAME_TILE_GAP).height),
    ];
    stacked_height(&panels)
}

fn arwes_backgrounds_height(bounds: Bounds) -> f32 {
    let available = (bounds.size.width - PANEL_PADDING * 2.0).max(0.0);
    let dots_count = DOT_SHAPES.len() * DOT_ORIGINS.len() * 2;
    let moving_count = LINE_DIRECTIONS.len() * 2;
    let panels = [
        panel_height(grid_metrics(available, dots_count, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height),
        panel_height(grid_metrics(available, 4, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height),
        panel_height(grid_metrics(available, 8, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height),
        panel_height(grid_metrics(available, moving_count, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height),
        panel_height(grid_metrics(available, 6, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height),
    ];
    stacked_height(&panels)
}

fn arwes_text_effects_height(bounds: Bounds) -> f32 {
    let available = (bounds.size.width - PANEL_PADDING * 2.0).max(0.0);
    let panels = [
        panel_height(grid_metrics(available, 8, TEXT_TILE_W, TEXT_TILE_H, TEXT_TILE_GAP).height),
        panel_height(grid_metrics(available, 6, TEXT_TILE_W, TEXT_TILE_H, TEXT_TILE_GAP).height),
    ];
    stacked_height(&panels)
}

fn arwes_illuminator_height(bounds: Bounds) -> f32 {
    let available = (bounds.size.width - PANEL_PADDING * 2.0).max(0.0);
    let panels = [
        panel_height(grid_metrics(available, 8, ILLUMINATOR_TILE_W, ILLUMINATOR_TILE_H, ILLUMINATOR_TILE_GAP).height),
        panel_height(grid_metrics(available, 4, ILLUMINATOR_TILE_W, ILLUMINATOR_TILE_H, ILLUMINATOR_TILE_GAP).height),
    ];
    stacked_height(&panels)
}

fn hud_widgets_height(bounds: Bounds) -> f32 {
    let available = (bounds.size.width - PANEL_PADDING * 2.0).max(0.0);
    let panels = [
        panel_height(grid_metrics(available, 6, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height),
        panel_height(grid_metrics(available, 6, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height),
        panel_height(grid_metrics(available, 6, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height),
    ];
    stacked_height(&panels)
}

fn light_demo_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(LIGHT_DEMO_FRAMES_INNER_H),
        panel_height(LIGHT_DEMO_HERO_INNER_H),
    ];
    stacked_height(&panels)
}

fn toolcall_demo_height(_bounds: Bounds) -> f32 {
    panel_height(TOOLCALL_DEMO_INNER_H)
}

fn system_ui_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(180.0),  // Tooltip demos
        panel_height(120.0),  // StatusBar demos
        panel_height(260.0),  // Notifications demos
        panel_height(200.0),  // ContextMenu demo
        panel_height(240.0),  // CommandPalette demo
    ];
    stacked_height(&panels)
}

fn chat_threads_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(480.0),  // Simple Conversation
        panel_height(600.0),  // Multi-Tool Workflow
        panel_height(520.0),  // Code Editing Session
        panel_height(440.0),  // Search & Navigation
        panel_height(320.0),  // Streaming Response
        panel_height(800.0),  // Complex Agent Session
        panel_height(280.0),  // Error Handling
    ];
    stacked_height(&panels)
}

fn bitcoin_wallet_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(200.0),  // Payment Method Icons
        panel_height(180.0),  // Payment Status Badges
        panel_height(160.0),  // Network Badges
        panel_height(200.0),  // Bitcoin Amounts
        panel_height(220.0),  // Balance Cards
        panel_height(300.0),  // Payment Rows (Transaction History)
        panel_height(320.0),  // Invoice Displays
        panel_height(400.0),  // Complete Wallet Dashboard
    ];
    stacked_height(&panels)
}
