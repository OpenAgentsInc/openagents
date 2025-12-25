use std::sync::Arc;
use std::time::Duration;

use wgpui::{
    AnimatorState, Bounds, Component, EventContext, EventResult, Hsla, Illuminator, InputEvent,
    Key, Modifiers, MouseButton, NamedKey, PaintContext, Point, Quad, Scene, Size, Text,
    TextDecipher, TextEffectTiming, TextSequence, TextSystem, theme,
};
use wgpui::components::atoms::{
    CheckpointBadge, ContentType, ContentTypeIcon, EntryMarker, EntryType, FeedbackButton,
    KeybindingHint, Mode, ModeBadge, Model, ModelBadge, PermissionAction,
    PermissionButton, Status, StatusDot, StreamingIndicator, ThinkingToggle, ToolIcon, ToolStatus,
    ToolStatusBadge, ToolType,
};
use wgpui::components::hud::{
    CornerConfig, DotsGrid, DotsOrigin, DotShape, DrawDirection, Frame, FrameAnimation, FrameStyle,
    GridLinesBackground, LineDirection, MovingLinesBackground, PuffsBackground,
};
use wgpui::components::molecules::{
    CheckpointRestore, DiffHeader, DiffType, MessageHeader, ModeSelector, ModelSelector,
    PermissionBar, ThinkingBlock, ToolHeader,
};
use wgpui::components::organisms::{
    AssistantMessage, PermissionDialog, SearchToolCall, TerminalToolCall, ThreadControls,
    ThreadEntry, ThreadEntryType, ToolCallCard, UserMessage,
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
const NAV_ITEM_HEIGHT: f32 = 32.0;
const GAP: f32 = 20.0;
const PANEL_PADDING: f32 = 12.0;
const SECTION_GAP: f32 = 24.0;
const FRAME_TILE_W: f32 = 170.0;
const FRAME_TILE_H: f32 = 110.0;
const FRAME_TILE_GAP: f32 = 12.0;
const FRAME_VARIANT_W: f32 = 160.0;
const FRAME_VARIANT_H: f32 = 100.0;
const BG_TILE_W: f32 = 180.0;
const BG_TILE_H: f32 = 120.0;
const BG_TILE_GAP: f32 = 12.0;
const TEXT_TILE_W: f32 = 240.0;
const TEXT_TILE_H: f32 = 80.0;
const TEXT_TILE_GAP: f32 = 12.0;
const ILLUMINATOR_TILE_W: f32 = 200.0;
const ILLUMINATOR_TILE_H: f32 = 140.0;
const ILLUMINATOR_TILE_GAP: f32 = 12.0;
const SECTION_OVERVIEW: usize = 0;
const SECTION_ATOMS: usize = 1;
const SECTION_MOLECULES: usize = 2;
const SECTION_ORGANISMS: usize = 3;
const SECTION_INTERACTIONS: usize = 4;
const SECTION_ARWES_FRAMES: usize = 5;
const SECTION_ARWES_BACKGROUNDS: usize = 6;
const SECTION_ARWES_TEXT: usize = 7;
const SECTION_ARWES_ILLUMINATOR: usize = 8;

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
        ];
        let nav_len = nav_items.len();

        Self {
            nav_items,
            active_section: 0,
            hover_nav: None,
            scroll_offsets: vec![0.0; nav_len],
            cursor_position: Point::new(0.0, 0.0),
            event_context: EventContext::new(),
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
        }
    }

    fn tick(&mut self) {
        self.streaming_indicator.tick();
        self.assistant_message.tick();
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
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
            ),
            cx,
        );

        let mut subtitle_text = Text::new(subtitle)
            .font_size(theme::font_size::SM)
            .color(theme::text::MUTED);
        subtitle_text.paint(
            Bounds::new(
                bounds.origin.x,
                bounds.origin.y + 26.0,
                bounds.size.width,
                bounds.size.height,
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
                                    .distance(26.0)
                                    .size(2.0)
                                    .opacity(0.8)
                                    .color(Hsla::new(190.0, 0.4, 0.6, 0.6));
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
                        .distance(26.0)
                        .size(2.0)
                        .opacity(0.8)
                        .animation_progress(*progress)
                        .color(Hsla::new(190.0, 0.4, 0.6, 0.6));
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
                            .spacing(28.0)
                            .line_width(1.0)
                            .color(Hsla::new(190.0, 0.3, 0.7, 0.5))
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
                            .line_width(1.0)
                            .color(Hsla::new(200.0, 0.3, 0.7, 0.4))
                            .sets(4)
                            .cycle_duration(Duration::from_secs(6));
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
                ("Default", PuffsBackground::new()),
                (
                    "Dense",
                    PuffsBackground::new()
                        .quantity(18)
                        .layers(12)
                        .radius_offset((6.0, 60.0)),
                ),
                (
                    "Sparse",
                    PuffsBackground::new()
                        .quantity(6)
                        .layers(5)
                        .radius_offset((2.0, 24.0)),
                ),
                (
                    "Warm",
                    PuffsBackground::new()
                        .color(Hsla::new(32.0, 0.8, 0.6, 0.2))
                        .quantity(12)
                        .layers(10),
                ),
                (
                    "Wide",
                    PuffsBackground::new()
                        .quantity(8)
                        .padding(80.0)
                        .radius_offset((6.0, 50.0)),
                ),
                (
                    "Offset",
                    PuffsBackground::new()
                        .quantity(10)
                        .y_offset((-20.0, -120.0))
                        .x_offset((10.0, 40.0)),
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
        20.0,
    );
    let mut title_text = Text::new(title)
        .font_size(theme::font_size::SM)
        .color(theme::text::PRIMARY);
    title_text.paint(title_bounds, cx);

    let inner = Bounds::new(
        bounds.origin.x + PANEL_PADDING,
        bounds.origin.y + PANEL_PADDING + 22.0,
        (bounds.size.width - PANEL_PADDING * 2.0).max(0.0),
        (bounds.size.height - PANEL_PADDING * 2.0 - 22.0).max(0.0),
    );
    paint(inner, cx);
}

fn panel_inner(bounds: Bounds) -> Bounds {
    Bounds::new(
        bounds.origin.x + PANEL_PADDING,
        bounds.origin.y + PANEL_PADDING + 22.0,
        (bounds.size.width - PANEL_PADDING * 2.0).max(0.0),
        (bounds.size.height - PANEL_PADDING * 2.0 - 22.0).max(0.0),
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
        .font_size(theme::font_size::XS)
        .color(theme::text::MUTED);
    text.paint(
        Bounds::new(
            bounds.origin.x + 8.0,
            bounds.origin.y + 6.0,
            bounds.size.width - 16.0,
            16.0,
        ),
        cx,
    );

    let inner = Bounds::new(
        bounds.origin.x + 8.0,
        bounds.origin.y + 26.0,
        (bounds.size.width - 16.0).max(0.0),
        (bounds.size.height - 32.0).max(0.0),
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
    inner_height + PANEL_PADDING * 2.0 + 22.0
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
    let panels = [
        panel_height(grid_metrics(available, permutations, FRAME_TILE_W, FRAME_TILE_H, FRAME_TILE_GAP).height),
        panel_height(grid_metrics(available, FRAME_STYLES.len() * 2, FRAME_VARIANT_W, FRAME_VARIANT_H, FRAME_TILE_GAP).height),
        panel_height(grid_metrics(available, FRAME_STYLES.len() * 2, FRAME_VARIANT_W, FRAME_VARIANT_H, FRAME_TILE_GAP).height),
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
