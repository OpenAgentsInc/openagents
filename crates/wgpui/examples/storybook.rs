use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use wgpui::components::atoms::{
    AgentScheduleBadge, AgentStatus, AgentStatusBadge, AgentType, AmountDirection, ApmGauge,
    ApmLevel, Bech32Entity, Bech32Type, BitcoinAmount, BitcoinNetwork, BitcoinUnit, BountyBadge,
    BountyStatus, CheckpointBadge, ContentType, ContentTypeIcon, ContributionStatus, DaemonStatus,
    DaemonStatusBadge, EarningsBadge, EarningsType, EntryMarker, EntryType, EventKind,
    EventKindBadge, FeedbackButton, GoalPriority, GoalProgressBadge, GoalStatus, IssueStatus,
    IssueStatusBadge, JobStatus, JobStatusBadge, KeybindingHint, LicenseStatus, MarketType,
    MarketTypeBadge, Mode, ModeBadge, Model, ModelBadge, NetworkBadge, ParallelAgentBadge,
    ParallelAgentStatus, PaymentMethod, PaymentMethodIcon, PaymentStatus, PaymentStatusBadge,
    PermissionAction, PermissionButton, PrStatus, PrStatusBadge, RelayStatus, RelayStatusBadge,
    RelayStatusDot, ReputationBadge, ResourceType, ResourceUsageBar, SessionStatus,
    SessionStatusBadge, SkillLicenseBadge, SkillType, StackLayerBadge, StackLayerStatus, Status,
    StatusDot, StreamingIndicator, ThinkingToggle, ThresholdKeyBadge, TickEventBadge, TickOutcome,
    ToolIcon, ToolStatus, ToolStatusBadge, ToolType, TrajectorySource, TrajectorySourceBadge,
    TrajectoryStatus, TrajectoryStatusBadge, TriggerType, TrustTier,
};
use wgpui::components::atoms::{BreadcrumbItem, SessionBreadcrumb};
use wgpui::components::hud::{
    CornerConfig, DotShape, DotsGrid, DotsOrigin, DrawDirection, Frame, FrameAnimation, FrameStyle,
    GridLinesBackground, LineDirection, MovingLinesBackground, NotificationLevel,
    PuffsBackground, ResizablePane, Reticle, Scanlines, SignalMeter, StatusBar, StatusBarPosition,
    StatusItem, Tooltip, TooltipPosition,
};
use wgpui::components::molecules::{
    AddressCard, AddressType, AgentProfileCard, AgentProfileInfo, ApmComparisonCard,
    ApmSessionData, ApmSessionRow, ComparisonSession, ContactCard, ContactInfo,
    ContactVerification, DataFormat, DataLicense, DatasetCard, DatasetInfo, DmBubble, DmDirection,
    DmMessage, EncryptionStatus, IssueInfo, IssueLabel, IssueRow, MnemonicDisplay,
    PermissionDecision, PermissionHistory, PermissionHistoryItem, PermissionRule,
    PermissionRuleRow, PermissionScope, PrEvent, PrEventType, PrTimelineItem, ProviderCard,
    ProviderInfo, ProviderSpecs, ProviderStatus, RepoCard, RepoInfo, RepoVisibility, ReviewState,
    SessionCard, SessionInfo, SessionSearchBar, SigningRequestCard, SigningRequestInfo,
    SigningType, SigningUrgency, SkillCard, SkillCategory, SkillInfo, SkillInstallStatus,
    TransactionDirection, TransactionInfo, TransactionRow, ZapCard, ZapInfo,
};
use wgpui::components::molecules::{
    BalanceCard, CheckpointRestore, DiffHeader, DiffType, InvoiceDisplay, InvoiceInfo, InvoiceType,
    MessageHeader, ModeSelector, ModelSelector, PaymentDirection, PaymentInfo, PaymentRow,
    PermissionBar, RelayInfo, RelayRow, ThinkingBlock, ToolHeader, WalletBalance,
};
use wgpui::components::molecules::{EntryActions, TerminalHeader};
use wgpui::components::organisms::{
    AgentAction, AgentGoal, AgentGoalStatus, AgentStateInspector, ApmLeaderboard, DmThread,
    EventData, EventInspector, IntervalUnit, KeyShare, LeaderboardEntry, PeerStatus, ReceiveFlow,
    ReceiveStep, ReceiveType, RelayManager, ResourceUsage, ScheduleConfig, ScheduleData,
    ScheduleType, SendFlow, SendStep, SigningRequest, TagData, ThresholdKeyManager, ThresholdPeer,
    ZapFlow,
};
use wgpui::components::organisms::{
    AssistantMessage, DiffLine, DiffLineKind, DiffToolCall, PermissionDialog, SearchMatch,
    SearchToolCall, TerminalToolCall, ThreadControls, ThreadEntry, ThreadEntryType, ToolCallCard,
    UserMessage,
};
use wgpui::components::sections::{
    MessageEditor, ThreadFeedback, ThreadHeader, TrajectoryEntry, TrajectoryView,
};
use wgpui::renderer::Renderer;
use wgpui::{
    Animation, AnimatorState, Bounds, Component, Easing, EventContext, EventResult, Hsla,
    Illuminator, InputEvent, Key, Modifiers, MouseButton, NamedKey, PaintContext, Point, Quad,
    Scene, Size, SpringAnimation, Text, TextDecipher, TextEffectTiming, TextSequence, TextSystem,
    theme,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};
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
const SECTION_NOSTR_PROTOCOL: usize = 15;
const SECTION_GITAFTER: usize = 16;
const SECTION_SOVEREIGN_AGENTS: usize = 17;
const SECTION_MARKETPLACE: usize = 18;
const SECTION_AUTOPILOT: usize = 19;
const SECTION_THREAD_COMPONENTS: usize = 20;
const SECTION_SESSIONS: usize = 21;
const SECTION_PERMISSIONS: usize = 22;
const SECTION_APM_METRICS: usize = 23;
const SECTION_WALLET_FLOWS: usize = 24;
const SECTION_GITAFTER_FLOWS: usize = 25;
const SECTION_MARKETPLACE_FLOWS: usize = 26;
const SECTION_NOSTR_FLOWS: usize = 27;
const SECTION_SOVEREIGN_AGENT_FLOWS: usize = 28;
const HOT_RELOAD_POLL_MS: u64 = 500;

#[derive(Clone, Copy)]
struct GlowPreset {
    short: &'static str,
    color: Hsla,
}

const GLOW_PRESETS: [GlowPreset; 8] = [
    GlowPreset {
        short: "Wht",
        color: Hsla::new(0.0, 0.0, 1.0, 0.6),
    },
    GlowPreset {
        short: "Cyn",
        color: Hsla::new(180.0, 1.0, 0.7, 0.5),
    },
    GlowPreset {
        short: "Pur",
        color: Hsla::new(280.0, 1.0, 0.7, 0.5),
    },
    GlowPreset {
        short: "Grn",
        color: Hsla::new(120.0, 1.0, 0.6, 0.5),
    },
    GlowPreset {
        short: "C2",
        color: Hsla::new(0.5, 1.0, 0.6, 0.8),
    },
    GlowPreset {
        short: "Org",
        color: Hsla::new(0.125, 1.0, 0.5, 0.9),
    },
    GlowPreset {
        short: "Red",
        color: Hsla::new(0.0, 1.0, 0.5, 1.0),
    },
    GlowPreset {
        short: "G2",
        color: Hsla::new(0.389, 1.0, 0.5, 0.8),
    },
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
    let args: Vec<String> = std::env::args().collect();
    if hot_reload_requested(&args) {
        start_hot_reload_watcher(args);
    }

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("Event loop failed");
}

fn hot_reload_requested(args: &[String]) -> bool {
    args.iter()
        .any(|arg| arg == "--hot" || arg == "--hot-reload")
}

// Restart the storybook process when the compiled binary changes.
fn start_hot_reload_watcher(args: Vec<String>) {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(_) => return,
    };
    let last_modified = match std::fs::metadata(&exe).and_then(|meta| meta.modified()) {
        Ok(time) => time,
        Err(_) => return,
    };

    std::thread::spawn(move || {
        let last_modified = last_modified;
        loop {
            std::thread::sleep(Duration::from_millis(HOT_RELOAD_POLL_MS));
            let Ok(meta) = std::fs::metadata(&exe) else {
                continue;
            };
            let Ok(modified) = meta.modified() else {
                continue;
            };
            if modified > last_modified {
                let _ = std::process::Command::new(&exe)
                    .args(args.iter().skip(1))
                    .spawn();
                std::process::exit(0);
            }
        }
    });
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
            WindowEvent::MouseInput {
                state: mouse_state,
                button,
                ..
            } => {
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
                state.story.paint(
                    bounds,
                    &mut PaintContext::new(&mut scene, &mut state.text_system, state.scale_factor),
                );

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

                let scale_factor = state.window.scale_factor() as f32;
                state
                    .renderer
                    .prepare(&state.device, &state.queue, &scene, scale_factor);
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
            "Nostr Protocol",
            "GitAfter",
            "Sovereign Agents",
            "Marketplace",
            "Autopilot",
            "Thread Components",
            "Sessions",
            "Permissions",
            "APM Metrics",
            "Wallet Flows",
            "GitAfter Flows",
            "Marketplace Flows",
            "Nostr Flows",
            "Agent Flows",
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
            SECTION_ATOMS => atoms_height(bounds),
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
            SECTION_NOSTR_PROTOCOL => nostr_protocol_height(bounds),
            SECTION_GITAFTER => gitafter_height(bounds),
            SECTION_SOVEREIGN_AGENTS => sovereign_agents_height(bounds),
            SECTION_MARKETPLACE => marketplace_height(bounds),
            SECTION_AUTOPILOT => autopilot_height(bounds),
            SECTION_THREAD_COMPONENTS => thread_components_height(bounds),
            SECTION_SESSIONS => sessions_height(bounds),
            SECTION_PERMISSIONS => permissions_height(bounds),
            SECTION_APM_METRICS => apm_metrics_height(bounds),
            SECTION_WALLET_FLOWS => wallet_flows_height(bounds),
            SECTION_GITAFTER_FLOWS => gitafter_flows_height(bounds),
            SECTION_MARKETPLACE_FLOWS => marketplace_flows_height(bounds),
            SECTION_NOSTR_FLOWS => nostr_flows_height(bounds),
            SECTION_SOVEREIGN_AGENT_FLOWS => sovereign_agent_flows_height(bounds),
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
        let scroll = self
            .scroll_offsets
            .get(self.active_section)
            .copied()
            .unwrap_or(0.0);
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
            SECTION_NOSTR_PROTOCOL => self.paint_nostr_protocol(content_bounds, cx),
            SECTION_GITAFTER => self.paint_gitafter(content_bounds, cx),
            SECTION_SOVEREIGN_AGENTS => self.paint_sovereign_agents(content_bounds, cx),
            SECTION_MARKETPLACE => self.paint_marketplace(content_bounds, cx),
            SECTION_AUTOPILOT => self.paint_autopilot(content_bounds, cx),
            SECTION_THREAD_COMPONENTS => self.paint_thread_components(content_bounds, cx),
            SECTION_SESSIONS => self.paint_sessions(content_bounds, cx),
            SECTION_PERMISSIONS => self.paint_permissions(content_bounds, cx),
            SECTION_APM_METRICS => self.paint_apm_metrics(content_bounds, cx),
            SECTION_WALLET_FLOWS => self.paint_wallet_flows(content_bounds, cx),
            SECTION_GITAFTER_FLOWS => self.paint_gitafter_flows(content_bounds, cx),
            SECTION_MARKETPLACE_FLOWS => self.paint_marketplace_flows(content_bounds, cx),
            SECTION_NOSTR_FLOWS => self.paint_nostr_flows(content_bounds, cx),
            SECTION_SOVEREIGN_AGENT_FLOWS => self.paint_sovereign_agent_flows(content_bounds, cx),
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

        let scroll = self
            .scroll_offsets
            .get(self.active_section)
            .copied()
            .unwrap_or(0.0);
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
            SECTION_INTERACTIONS => {
                handled |= self.handle_interactions_event(&event, content_bounds)
            }
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
                cx.scene
                    .draw_quad(Quad::new(item_bounds).with_background(bg));
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
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Tool & Status Atoms ==========
        let tool_height = panel_height(140.0);
        let tool_bounds = Bounds::new(bounds.origin.x, y, width, tool_height);
        draw_panel("Tool & Status Atoms", tool_bounds, cx, |inner, cx| {
            let mut x = inner.origin.x;
            let row_y = inner.origin.y;

            // Tool icons
            for tool_type in &[
                ToolType::Bash,
                ToolType::Read,
                ToolType::Edit,
                ToolType::Search,
            ] {
                let mut icon = ToolIcon::new(*tool_type);
                icon.paint(Bounds::new(x, row_y, 28.0, 22.0), cx);
                x += 36.0;
            }

            // Tool status badges
            x = inner.origin.x;
            let status_y = row_y + 35.0;
            for status in &[ToolStatus::Running, ToolStatus::Success, ToolStatus::Error] {
                let mut badge = ToolStatusBadge::new(*status);
                badge.paint(Bounds::new(x, status_y, 90.0, 22.0), cx);
                x += 100.0;
            }

            // Status dots
            x = inner.origin.x;
            let dots_y = status_y + 35.0;
            for status in &[Status::Online, Status::Busy, Status::Away, Status::Error] {
                let mut dot = StatusDot::new(*status).size(10.0);
                dot.paint(Bounds::new(x, dots_y, 12.0, 12.0), cx);
                let label = match status {
                    Status::Online => "Online",
                    Status::Busy => "Busy",
                    Status::Away => "Away",
                    Status::Error => "Error",
                    _ => "",
                };
                let label_run = cx.text.layout(
                    label,
                    Point::new(x + 16.0, dots_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);
                x += 70.0;
            }
        });
        y += tool_height + SECTION_GAP;

        // ========== Panel 2: Mode & Model Atoms ==========
        let mode_height = panel_height(160.0);
        let mode_bounds = Bounds::new(bounds.origin.x, y, width, mode_height);
        draw_panel("Mode & Model Atoms", mode_bounds, cx, |inner, cx| {
            // Mode badges
            let mut x = inner.origin.x;
            for mode in &[Mode::Normal, Mode::Act, Mode::Plan] {
                let mut badge = ModeBadge::new(*mode);
                badge.paint(Bounds::new(x, inner.origin.y, 70.0, 22.0), cx);
                x += 80.0;
            }

            // Model badges
            x = inner.origin.x;
            let model_y = inner.origin.y + 35.0;
            for model in &[Model::ClaudeSonnet, Model::ClaudeOpus, Model::ClaudeHaiku] {
                let mut badge = ModelBadge::new(*model);
                badge.paint(Bounds::new(x, model_y, 100.0, 22.0), cx);
                x += 110.0;
            }

            // Content types
            x = inner.origin.x;
            let content_y = model_y + 35.0;
            for content in &[
                ContentType::Markdown,
                ContentType::Code,
                ContentType::Image,
                ContentType::Text,
            ] {
                let mut icon = ContentTypeIcon::new(*content);
                icon.paint(Bounds::new(x, content_y, 28.0, 22.0), cx);
                x += 36.0;
            }

            // Entry markers
            x = inner.origin.x + 180.0;
            for entry in &[
                EntryType::User,
                EntryType::Assistant,
                EntryType::Tool,
                EntryType::System,
            ] {
                let mut marker = EntryMarker::new(*entry);
                marker.paint(Bounds::new(x, content_y, 28.0, 22.0), cx);
                x += 36.0;
            }
        });
        y += mode_height + SECTION_GAP;

        // ========== Panel 3: Agent Status Badges ==========
        let agent_height = panel_height(180.0);
        let agent_bounds = Bounds::new(bounds.origin.x, y, width, agent_height);
        draw_panel("Agent Status Badges", agent_bounds, cx, |inner, cx| {
            // Agent status badges
            let mut x = inner.origin.x;
            for (status, atype) in &[
                (AgentStatus::Idle, AgentType::Human),
                (AgentStatus::Online, AgentType::Sovereign),
                (AgentStatus::Busy, AgentType::Sovereign),
                (AgentStatus::Error, AgentType::Custodial),
            ] {
                let mut badge = AgentStatusBadge::new(*status).agent_type(*atype);
                badge.paint(Bounds::new(x, inner.origin.y, 120.0, 22.0), cx);
                x += 130.0;
            }

            // Agent schedule badges (heartbeat intervals)
            x = inner.origin.x;
            let sched_y = inner.origin.y + 35.0;
            for seconds in &[60, 300, 900, 3600] {
                let mut badge = AgentScheduleBadge::new(*seconds);
                badge.paint(Bounds::new(x, sched_y, 100.0, 22.0), cx);
                x += 110.0;
            }

            // Goal progress badges
            x = inner.origin.x;
            let goal_y = sched_y + 35.0;
            for (progress, status, priority) in &[
                (0.0, GoalStatus::NotStarted, GoalPriority::Low),
                (0.5, GoalStatus::InProgress, GoalPriority::Medium),
                (1.0, GoalStatus::Completed, GoalPriority::High),
                (0.3, GoalStatus::Blocked, GoalPriority::Critical),
            ] {
                let mut badge = GoalProgressBadge::new(*progress)
                    .status(*status)
                    .priority(*priority);
                badge.paint(Bounds::new(x, goal_y, 130.0, 22.0), cx);
                x += 140.0;
            }

            // Stack layer badges
            x = inner.origin.x;
            let stack_y = goal_y + 35.0;
            for (layer, status) in &[
                (1, StackLayerStatus::Pending),
                (2, StackLayerStatus::Ready),
                (3, StackLayerStatus::Merged),
            ] {
                let mut badge = StackLayerBadge::new(*layer, 3).status(status.clone());
                badge.paint(Bounds::new(x, stack_y, 100.0, 22.0), cx);
                x += 110.0;
            }
        });
        y += agent_height + SECTION_GAP;

        // ========== Panel 4: Bitcoin & Payment Atoms ==========
        let btc_height = panel_height(180.0);
        let btc_bounds = Bounds::new(bounds.origin.x, y, width, btc_height);
        draw_panel("Bitcoin & Payment Atoms", btc_bounds, cx, |inner, cx| {
            // Bitcoin amounts
            let mut x = inner.origin.x;
            for (sats, unit, dir) in &[
                (100_000u64, BitcoinUnit::Sats, AmountDirection::Incoming),
                (100_000u64, BitcoinUnit::Btc, AmountDirection::Outgoing),
                (50_000u64, BitcoinUnit::Sats, AmountDirection::Neutral),
            ] {
                let mut badge = BitcoinAmount::new(*sats).unit(*unit).direction(*dir);
                badge.paint(Bounds::new(x, inner.origin.y, 130.0, 22.0), cx);
                x += 140.0;
            }

            // Network badges
            x = inner.origin.x;
            let net_y = inner.origin.y + 35.0;
            for network in &[
                BitcoinNetwork::Mainnet,
                BitcoinNetwork::Testnet,
                BitcoinNetwork::Signet,
                BitcoinNetwork::Regtest,
            ] {
                let mut badge = NetworkBadge::new(*network);
                badge.paint(Bounds::new(x, net_y, 80.0, 22.0), cx);
                x += 90.0;
            }

            // Payment method icons
            x = inner.origin.x;
            let method_y = net_y + 35.0;
            for method in &[
                PaymentMethod::Lightning,
                PaymentMethod::OnChain,
                PaymentMethod::Spark,
            ] {
                let mut icon = PaymentMethodIcon::new(*method);
                icon.paint(Bounds::new(x, method_y, 28.0, 22.0), cx);
                x += 36.0;
            }

            // Payment status badges
            x = inner.origin.x + 120.0;
            for status in &[
                PaymentStatus::Pending,
                PaymentStatus::Completed,
                PaymentStatus::Failed,
            ] {
                let mut badge = PaymentStatusBadge::new(*status);
                badge.paint(Bounds::new(x, method_y, 90.0, 22.0), cx);
                x += 100.0;
            }

            // Threshold key badges
            x = inner.origin.x;
            let key_y = method_y + 35.0;
            let mut key1 = ThresholdKeyBadge::new(1, 3);
            key1.paint(Bounds::new(x, key_y, 80.0, 22.0), cx);
            let mut key2 = ThresholdKeyBadge::new(2, 3);
            key2.paint(Bounds::new(x + 90.0, key_y, 80.0, 22.0), cx);
        });
        y += btc_height + SECTION_GAP;

        // ========== Panel 5: Nostr Protocol Atoms ==========
        let nostr_height = panel_height(180.0);
        let nostr_bounds = Bounds::new(bounds.origin.x, y, width, nostr_height);
        draw_panel("Nostr Protocol Atoms", nostr_bounds, cx, |inner, cx| {
            // Relay status badges
            let mut x = inner.origin.x;
            for status in &[
                RelayStatus::Connected,
                RelayStatus::Connecting,
                RelayStatus::Disconnected,
                RelayStatus::Error,
            ] {
                let mut badge = RelayStatusBadge::new(*status);
                badge.paint(Bounds::new(x, inner.origin.y, 160.0, 22.0), cx);
                x += 170.0;
            }

            // Relay status dots
            x = inner.origin.x;
            let dot_y = inner.origin.y + 35.0;
            for status in &[
                RelayStatus::Connected,
                RelayStatus::Connecting,
                RelayStatus::Disconnected,
                RelayStatus::Error,
            ] {
                let mut dot = RelayStatusDot::new(*status);
                dot.paint(Bounds::new(x, dot_y, 12.0, 12.0), cx);
                x += 24.0;
            }

            // Event kind badges
            x = inner.origin.x;
            let event_y = dot_y + 30.0;
            for kind in &[
                EventKind::TextNote,
                EventKind::EncryptedDm,
                EventKind::Reaction,
                EventKind::Repost,
            ] {
                let mut badge = EventKindBadge::new(*kind);
                badge.paint(Bounds::new(x, event_y, 90.0, 22.0), cx);
                x += 100.0;
            }

            // Bech32 entities
            x = inner.origin.x;
            let bech_y = event_y + 35.0;
            for btype in &[Bech32Type::Npub, Bech32Type::Note, Bech32Type::Nevent] {
                let mut entity = Bech32Entity::new(*btype, "abc123def456");
                entity.paint(Bounds::new(x, bech_y, 140.0, 22.0), cx);
                x += 150.0;
            }
        });
        y += nostr_height + SECTION_GAP;

        // ========== Panel 6: GitAfter Atoms ==========
        let git_height = panel_height(180.0);
        let git_bounds = Bounds::new(bounds.origin.x, y, width, git_height);
        draw_panel("GitAfter Atoms", git_bounds, cx, |inner, cx| {
            // Issue status badges
            let mut x = inner.origin.x;
            for status in &[
                IssueStatus::Open,
                IssueStatus::InProgress,
                IssueStatus::Closed,
                IssueStatus::Claimed,
            ] {
                let mut badge = IssueStatusBadge::new(*status);
                badge.paint(Bounds::new(x, inner.origin.y, 100.0, 22.0), cx);
                x += 110.0;
            }

            // PR status badges
            x = inner.origin.x;
            let pr_y = inner.origin.y + 35.0;
            for status in &[
                PrStatus::Open,
                PrStatus::Merged,
                PrStatus::Closed,
                PrStatus::Draft,
            ] {
                let mut badge = PrStatusBadge::new(*status);
                badge.paint(Bounds::new(x, pr_y, 80.0, 22.0), cx);
                x += 90.0;
            }

            // Bounty badges
            x = inner.origin.x;
            let bounty_y = pr_y + 35.0;
            for (status, sats) in &[
                (BountyStatus::Active, 50000u64),
                (BountyStatus::Claimed, 100000u64),
                (BountyStatus::Paid, 250000u64),
                (BountyStatus::Expired, 10000u64),
            ] {
                let mut badge = BountyBadge::new(*sats).status(*status);
                badge.paint(Bounds::new(x, bounty_y, 130.0, 22.0), cx);
                x += 140.0;
            }

            // Tick event badges
            x = inner.origin.x;
            let tick_y = bounty_y + 35.0;
            let mut request = TickEventBadge::request();
            request.paint(Bounds::new(x, tick_y, 100.0, 22.0), cx);
            x += 110.0;
            let mut result_success = TickEventBadge::result(TickOutcome::Success);
            result_success.paint(Bounds::new(x, tick_y, 100.0, 22.0), cx);
            x += 110.0;
            let mut result_fail = TickEventBadge::result(TickOutcome::Failure);
            result_fail.paint(Bounds::new(x, tick_y, 100.0, 22.0), cx);
        });
        y += git_height + SECTION_GAP;

        // ========== Panel 7: Marketplace Atoms ==========
        let market_height = panel_height(180.0);
        let market_bounds = Bounds::new(bounds.origin.x, y, width, market_height);
        draw_panel("Marketplace Atoms", market_bounds, cx, |inner, cx| {
            // Market type badges
            let mut x = inner.origin.x;
            for mtype in &[
                MarketType::Compute,
                MarketType::Skills,
                MarketType::Data,
                MarketType::Trajectories,
            ] {
                let mut badge = MarketTypeBadge::new(*mtype);
                badge.paint(Bounds::new(x, inner.origin.y, 90.0, 22.0), cx);
                x += 100.0;
            }

            // Job status badges
            x = inner.origin.x;
            let job_y = inner.origin.y + 35.0;
            for status in &[
                JobStatus::Pending,
                JobStatus::Processing,
                JobStatus::Completed,
                JobStatus::Failed,
            ] {
                let mut badge = JobStatusBadge::new(*status);
                badge.paint(Bounds::new(x, job_y, 100.0, 22.0), cx);
                x += 110.0;
            }

            // Reputation badges
            x = inner.origin.x;
            let rep_y = job_y + 35.0;
            for tier in &[
                TrustTier::New,
                TrustTier::Established,
                TrustTier::Trusted,
                TrustTier::Expert,
            ] {
                let mut badge = ReputationBadge::new(*tier);
                badge.paint(Bounds::new(x, rep_y, 100.0, 22.0), cx);
                x += 110.0;
            }

            // Trajectory source badges
            x = inner.origin.x;
            let traj_y = rep_y + 35.0;
            for source in &[
                TrajectorySource::Claude,
                TrajectorySource::Cursor,
                TrajectorySource::Codex,
            ] {
                let mut badge = TrajectorySourceBadge::new(*source);
                badge.paint(Bounds::new(x, traj_y, 90.0, 22.0), cx);
                x += 100.0;
            }

            // Trajectory status badges
            for status in &[
                TrajectoryStatus::Verified,
                TrajectoryStatus::Partial,
                TrajectoryStatus::Suspicious,
            ] {
                let mut badge = TrajectoryStatusBadge::new(*status);
                badge.paint(Bounds::new(x, traj_y, 100.0, 22.0), cx);
                x += 110.0;
            }
        });
        y += market_height + SECTION_GAP;

        // ========== Panel 8: Autopilot Atoms ==========
        let auto_height = panel_height(180.0);
        let auto_bounds = Bounds::new(bounds.origin.x, y, width, auto_height);
        draw_panel("Autopilot Atoms", auto_bounds, cx, |inner, cx| {
            // Session status badges
            let mut x = inner.origin.x;
            for status in &[
                SessionStatus::Pending,
                SessionStatus::Running,
                SessionStatus::Completed,
                SessionStatus::Failed,
            ] {
                let mut badge = SessionStatusBadge::new(*status);
                badge.paint(Bounds::new(x, inner.origin.y, 90.0, 22.0), cx);
                x += 100.0;
            }

            // APM gauges
            x = inner.origin.x;
            let apm_y = inner.origin.y + 35.0;
            for apm in &[0.0, 15.0, 45.0, 80.0] {
                let mut gauge = ApmGauge::new(*apm).compact(true);
                gauge.paint(Bounds::new(x, apm_y, 70.0, 22.0), cx);
                x += 80.0;
            }

            // Resource usage bars
            x = inner.origin.x;
            let res_y = apm_y + 35.0;
            for (rtype, pct) in &[
                (ResourceType::Memory, 35.0),
                (ResourceType::Memory, 75.0),
                (ResourceType::Cpu, 50.0),
            ] {
                let mut bar = ResourceUsageBar::new(*rtype, *pct).bar_width(50.0);
                bar.paint(Bounds::new(x, res_y, 140.0, 22.0), cx);
                x += 150.0;
            }

            // Daemon status badges
            x = inner.origin.x;
            let daemon_y = res_y + 35.0;
            for status in &[
                DaemonStatus::Offline,
                DaemonStatus::Online,
                DaemonStatus::Error,
            ] {
                let mut badge = DaemonStatusBadge::new(*status).compact(true);
                badge.paint(Bounds::new(x, daemon_y, 28.0, 22.0), cx);
                x += 36.0;
            }

            // Parallel agent badges
            x = inner.origin.x + 120.0;
            for (idx, status) in &[
                (0, ParallelAgentStatus::Idle),
                (1, ParallelAgentStatus::Running),
                (2, ParallelAgentStatus::Completed),
            ] {
                let mut badge = ParallelAgentBadge::new(*idx, *status).compact(true);
                badge.paint(Bounds::new(x, daemon_y, 50.0, 22.0), cx);
                x += 60.0;
            }
        });
        y += auto_height + SECTION_GAP;

        // ========== Panel 9: Interactive Atoms ==========
        let interact_height = panel_height(160.0);
        let interact_bounds = Bounds::new(bounds.origin.x, y, width, interact_height);
        draw_panel("Interactive Atoms", interact_bounds, cx, |inner, cx| {
            // Permission buttons
            let mut x = inner.origin.x;
            for action in &[
                PermissionAction::AllowOnce,
                PermissionAction::AllowAlways,
                PermissionAction::Deny,
            ] {
                let mut btn = PermissionButton::new(*action);
                btn.paint(Bounds::new(x, inner.origin.y, 100.0, 26.0), cx);
                x += 110.0;
            }

            // Feedback buttons
            x = inner.origin.x;
            let feedback_y = inner.origin.y + 38.0;
            let mut up = FeedbackButton::thumbs_up();
            up.paint(Bounds::new(x, feedback_y, 32.0, 26.0), cx);
            let mut down = FeedbackButton::thumbs_down();
            down.paint(Bounds::new(x + 40.0, feedback_y, 32.0, 26.0), cx);

            // Thinking toggle
            let mut toggle = ThinkingToggle::new().expanded(true);
            toggle.paint(Bounds::new(x + 100.0, feedback_y, 100.0, 26.0), cx);

            // Keybinding hints
            x = inner.origin.x;
            let key_y = feedback_y + 38.0;
            let mut hint1 = KeybindingHint::single("K");
            hint1.paint(Bounds::new(x, key_y, 24.0, 22.0), cx);
            let mut hint2 = KeybindingHint::combo(&["Ctrl", "K"]);
            hint2.paint(Bounds::new(x + 32.0, key_y, 60.0, 22.0), cx);
            let mut hint3 = KeybindingHint::combo(&["Cmd", "Shift", "P"]);
            hint3.paint(Bounds::new(x + 100.0, key_y, 100.0, 22.0), cx);

            // Checkpoint badges
            let mut cp1 = CheckpointBadge::new("v1.0").active(false);
            cp1.paint(Bounds::new(x + 220.0, key_y, 60.0, 22.0), cx);
            let mut cp2 = CheckpointBadge::new("v1.2").active(true);
            cp2.paint(Bounds::new(x + 290.0, key_y, 60.0, 22.0), cx);

            // Streaming indicator
            self.streaming_indicator
                .paint(Bounds::new(x + 370.0, key_y, 80.0, 22.0), cx);

            // Skill license badges
            x = inner.origin.x;
            let skill_y = key_y + 32.0;
            for (stype, lstatus) in &[
                (SkillType::Code, LicenseStatus::Active),
                (SkillType::Data, LicenseStatus::Expired),
                (SkillType::Model, LicenseStatus::Pending),
            ] {
                let mut badge = SkillLicenseBadge::new(*stype, *lstatus);
                badge.paint(Bounds::new(x, skill_y, 110.0, 22.0), cx);
                x += 120.0;
            }

            // Earnings badges (compact)
            for etype in &[
                EarningsType::Compute,
                EarningsType::Skills,
                EarningsType::Data,
            ] {
                let mut badge = EarningsBadge::new(*etype, 25000).compact(true);
                badge.paint(Bounds::new(x, skill_y, 70.0, 22.0), cx);
                x += 80.0;
            }
        });
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
            let mut tool_header =
                ToolHeader::new(ToolType::Read, "read_file").status(ToolStatus::Success);
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
            let right = Bounds::new(
                inner.origin.x + col_width + col_gap,
                inner.origin.y,
                col_width,
                inner.size.height,
            );

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
        let grid = grid_metrics(
            available,
            permutations,
            FRAME_TILE_W,
            FRAME_TILE_H,
            FRAME_TILE_GAP,
        );
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
        let flicker_grid = grid_metrics(
            available,
            flicker_count,
            FRAME_VARIANT_W,
            FRAME_VARIANT_H,
            FRAME_TILE_GAP,
        );
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
        let glow_grid = grid_metrics(
            available,
            glow_count,
            FRAME_VARIANT_W,
            FRAME_VARIANT_H,
            FRAME_TILE_GAP,
        );
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
        let glow_palette_grid = grid_metrics(
            available,
            glow_palette_count,
            FRAME_VARIANT_W,
            FRAME_VARIANT_H,
            FRAME_TILE_GAP,
        );
        let glow_palette_height = panel_height(glow_palette_grid.height);
        let glow_palette_bounds = Bounds::new(bounds.origin.x, y, width, glow_palette_height);
        draw_panel(
            "Glow palette x animation",
            glow_palette_bounds,
            cx,
            |inner, cx| {
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
            },
        );
        y += glow_palette_height + SECTION_GAP;

        let nefrex_count = 16;
        let nefrex_grid = grid_metrics(
            available,
            nefrex_count,
            FRAME_VARIANT_W,
            FRAME_VARIANT_H,
            FRAME_TILE_GAP,
        );
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
        let header_grid = grid_metrics(
            available,
            header_count,
            FRAME_VARIANT_W,
            FRAME_VARIANT_H,
            FRAME_TILE_GAP,
        );
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
        let circle_grid = grid_metrics(
            available,
            circle_count,
            FRAME_VARIANT_W,
            FRAME_VARIANT_H,
            FRAME_TILE_GAP,
        );
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
                let grid = grid_metrics(
                    inner.size.width,
                    dots_count,
                    BG_TILE_W,
                    BG_TILE_H,
                    BG_TILE_GAP,
                );
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
        let dots_state_grid = grid_metrics(
            available,
            dots_state_count,
            BG_TILE_W,
            BG_TILE_H,
            BG_TILE_GAP,
        );
        let dots_state_height = panel_height(dots_state_grid.height);
        let dots_state_bounds = Bounds::new(bounds.origin.x, y, width, dots_state_height);
        draw_panel(
            "DotsGrid progress states",
            dots_state_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    dots_state_count,
                    BG_TILE_W,
                    BG_TILE_H,
                    BG_TILE_GAP,
                );
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
            },
        );
        y += dots_state_height + SECTION_GAP;

        let grid_lines_count = 8;
        let grid_lines_grid = grid_metrics(
            available,
            grid_lines_count,
            BG_TILE_W,
            BG_TILE_H,
            BG_TILE_GAP,
        );
        let grid_lines_height = panel_height(grid_lines_grid.height);
        let grid_lines_bounds = Bounds::new(bounds.origin.x, y, width, grid_lines_height);
        draw_panel(
            "GridLines permutations (orientation x dash)",
            grid_lines_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    grid_lines_count,
                    BG_TILE_W,
                    BG_TILE_H,
                    BG_TILE_GAP,
                );
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
            },
        );
        y += grid_lines_height + SECTION_GAP;

        let moving_count = LINE_DIRECTIONS.len() * 2;
        let moving_grid = grid_metrics(available, moving_count, BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
        let moving_height = panel_height(moving_grid.height);
        let moving_bounds = Bounds::new(bounds.origin.x, y, width, moving_height);
        draw_panel(
            "MovingLines permutations (direction x spacing)",
            moving_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    moving_count,
                    BG_TILE_W,
                    BG_TILE_H,
                    BG_TILE_GAP,
                );
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
                        let label =
                            format!("{} {}", line_direction_label(direction), spacing as i32);
                        draw_tile(tile_bounds, &label, cx, |inner, cx| {
                            let mut lines = MovingLinesBackground::new()
                                .direction(direction)
                                .spacing(spacing)
                                .line_width(2.5)
                                .color(Hsla::new(45.0, 0.9, 0.6, 0.85))
                                .sets(5)
                                .cycle_duration(Duration::from_secs(4));
                            lines.update_with_delta(
                                AnimatorState::Entered,
                                Duration::from_millis(600),
                            );
                            lines.paint(inner, cx);
                        });
                        idx += 1;
                    }
                }
            },
        );
        y += moving_height + SECTION_GAP;

        let puff_presets = 6;
        let puff_grid = grid_metrics(available, puff_presets, BG_TILE_W, BG_TILE_H, BG_TILE_GAP);
        let puff_height = panel_height(puff_grid.height);
        let puff_bounds = Bounds::new(bounds.origin.x, y, width, puff_height);
        draw_panel("Puffs permutations", puff_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                puff_presets,
                BG_TILE_W,
                BG_TILE_H,
                BG_TILE_GAP,
            );
            let presets: Vec<(&str, PuffsBackground)> = vec![
                (
                    "Cyan",
                    PuffsBackground::new()
                        .color(Hsla::new(180.0, 0.9, 0.5, 0.4))
                        .quantity(12)
                        .layers(8),
                ),
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
        let sequence_grid = grid_metrics(
            available,
            sequence_presets,
            TEXT_TILE_W,
            TEXT_TILE_H,
            TEXT_TILE_GAP,
        );
        let sequence_height = panel_height(sequence_grid.height);
        let sequence_bounds = Bounds::new(bounds.origin.x, y, width, sequence_height);
        draw_panel(
            "TextSequence permutations",
            sequence_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    sequence_presets,
                    TEXT_TILE_W,
                    TEXT_TILE_H,
                    TEXT_TILE_GAP,
                );
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
                let mut entering = TextSequence::new("Sequence reveal").timing(
                    TextEffectTiming::new(Duration::from_millis(900), Duration::from_millis(50)),
                );
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
            },
        );
        y += sequence_height + SECTION_GAP;

        let decipher_presets = 6;
        let decipher_grid = grid_metrics(
            available,
            decipher_presets,
            TEXT_TILE_W,
            TEXT_TILE_H,
            TEXT_TILE_GAP,
        );
        let decipher_height = panel_height(decipher_grid.height);
        let decipher_bounds = Bounds::new(bounds.origin.x, y, width, decipher_height);
        draw_panel(
            "TextDecipher permutations",
            decipher_bounds,
            cx,
            |inner, cx| {
                let grid = grid_metrics(
                    inner.size.width,
                    decipher_presets,
                    TEXT_TILE_W,
                    TEXT_TILE_H,
                    TEXT_TILE_GAP,
                );
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
            },
        );
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

        let scan_grid = grid_metrics(
            available,
            scan_presets.len(),
            BG_TILE_W,
            BG_TILE_H,
            BG_TILE_GAP,
        );
        let scan_height = panel_height(scan_grid.height);
        let scan_bounds = Bounds::new(bounds.origin.x, y, width, scan_height);
        draw_panel("Scanline sweeps", scan_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                scan_presets.len(),
                BG_TILE_W,
                BG_TILE_H,
                BG_TILE_GAP,
            );
            for (idx, (label, spacing, scan_width, opacity, hue, offset)) in
                scan_presets.iter().enumerate()
            {
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

        let meter_grid = grid_metrics(
            available,
            meter_presets.len(),
            BG_TILE_W,
            BG_TILE_H,
            BG_TILE_GAP,
        );
        let meter_height = panel_height(meter_grid.height);
        let meter_bounds = Bounds::new(bounds.origin.x, y, width, meter_height);
        draw_panel("Signal meters", meter_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                meter_presets.len(),
                BG_TILE_W,
                BG_TILE_H,
                BG_TILE_GAP,
            );
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

        let reticle_grid = grid_metrics(
            available,
            reticle_presets.len(),
            BG_TILE_W,
            BG_TILE_H,
            BG_TILE_GAP,
        );
        let reticle_height = panel_height(reticle_grid.height);
        let reticle_bounds = Bounds::new(bounds.origin.x, y, width, reticle_height);
        draw_panel("Reticle variants", reticle_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                reticle_presets.len(),
                BG_TILE_W,
                BG_TILE_H,
                BG_TILE_GAP,
            );
            for (idx, (label, line_length, gap, center, tick, hue)) in
                reticle_presets.iter().enumerate()
            {
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
        y += reticle_height + SECTION_GAP;

        // Resizable pane demos
        let resizable_presets: [(&str, bool, bool, f32); 6] = [
            ("Default", true, false, 8.0),
            ("Visible", true, true, 8.0),
            ("Large", true, true, 12.0),
            ("Small", true, true, 4.0),
            ("Disabled", false, false, 8.0),
            ("Styled", true, true, 10.0),
        ];

        let resizable_grid = grid_metrics(
            available,
            resizable_presets.len(),
            BG_TILE_W,
            BG_TILE_H,
            BG_TILE_GAP,
        );
        let resizable_height = panel_height(resizable_grid.height);
        let resizable_bounds = Bounds::new(bounds.origin.x, y, width, resizable_height);
        draw_panel("Resizable panes", resizable_bounds, cx, |inner, cx| {
            let grid = grid_metrics(
                inner.size.width,
                resizable_presets.len(),
                BG_TILE_W,
                BG_TILE_H,
                BG_TILE_GAP,
            );
            for (idx, (label, resizable, show_handles, handle_size)) in
                resizable_presets.iter().enumerate()
            {
                let row = idx / grid.cols;
                let col = idx % grid.cols;
                let tile_bounds = Bounds::new(
                    inner.origin.x + col as f32 * (BG_TILE_W + BG_TILE_GAP),
                    inner.origin.y + row as f32 * (BG_TILE_H + BG_TILE_GAP),
                    BG_TILE_W,
                    BG_TILE_H,
                );
                draw_tile(tile_bounds, label, cx, |inner, cx| {
                    let (handle_color, handle_hover_color, bg, border) = if *label == "Styled" {
                        (
                            Hsla::new(180.0, 0.6, 0.4, 0.4),
                            Hsla::new(180.0, 0.8, 0.6, 0.8),
                            Hsla::new(180.0, 0.2, 0.1, 0.6),
                            Hsla::new(180.0, 0.6, 0.5, 0.8),
                        )
                    } else {
                        (
                            Hsla::new(0.0, 0.0, 0.5, 0.3),
                            Hsla::new(180.0, 0.6, 0.5, 0.6),
                            Hsla::new(0.0, 0.0, 0.15, 0.5),
                            Hsla::new(0.0, 0.0, 0.4, 0.6),
                        )
                    };
                    let mut pane = ResizablePane::new()
                        .resizable(*resizable)
                        .show_handles(*show_handles)
                        .handle_size(*handle_size)
                        .handle_color(handle_color)
                        .handle_hover_color(handle_hover_color)
                        .background(bg)
                        .border_color(border)
                        .border_width(1.0);
                    pane.paint(inset_bounds(inner, 4.0), cx);
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
                    let bullet =
                        cx.text
                            .layout(">", Point::new(pane_x + 30.0, line_y), 14.0, accent);
                    cx.scene.draw_text(bullet);
                    let text =
                        cx.text
                            .layout(line, Point::new(pane_x + 44.0, line_y + 2.0), 13.0, muted);
                    cx.scene.draw_text(text);
                }
            }
        });
    }

    fn paint_toolcall_demo(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let demo_height = panel_height(TOOLCALL_DEMO_INNER_H);
        let panel_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            demo_height,
        );
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
                let mut tooltip =
                    Tooltip::new(format!("Tooltip positioned {}", label.to_lowercase()))
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
            let top_bar_bounds =
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 32.0);
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
            let bot_bar_bounds = Bounds::new(
                inner.origin.x,
                inner.origin.y + 50.0,
                inner.size.width,
                32.0,
            );
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
                (
                    "Success",
                    NotificationLevel::Success,
                    "Build completed successfully",
                ),
                (
                    "Warning",
                    NotificationLevel::Warning,
                    "Deprecated API usage detected",
                ),
                (
                    "Error",
                    NotificationLevel::Error,
                    "Connection to server failed",
                ),
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
            let menu_bounds =
                Bounds::new(inner.origin.x + 20.0, inner.origin.y + 10.0, menu_w, menu_h);

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
                        Quad::new(Bounds::new(
                            menu_bounds.origin.x + 4.0,
                            item_y,
                            menu_w - 8.0,
                            item_h,
                        ))
                        .with_background(theme::bg::MUTED),
                    );
                }

                let text_color = if *disabled {
                    theme::text::MUTED
                } else {
                    theme::text::PRIMARY
                };
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
                Quad::new(Bounds::new(
                    palette_x + 8.0,
                    palette_y + 8.0,
                    palette_w - 16.0,
                    input_h,
                ))
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
                        Quad::new(Bounds::new(
                            palette_x + 4.0,
                            item_y,
                            palette_w - 8.0,
                            item_h,
                        ))
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
            let mut user_msg =
                UserMessage::new("Can you help me understand how async/await works in Rust?")
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
                 Would you like me to show you a practical example?",
            )
            .model(Model::ClaudeSonnet)
            .timestamp("10:30 AM");
            asst_msg.paint(Bounds::new(inner.origin.x, entry_y, entry_w, asst_h), cx);
            entry_y += asst_h + entry_gap;

            // Follow-up user message
            let user2_h = 80.0;
            let mut user_msg2 =
                UserMessage::new("Yes please! Show me a simple HTTP request example.")
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
            let mut user_msg =
                UserMessage::new("Find all TODO comments in the codebase and list them")
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
                    SearchMatch {
                        file: "src/main.rs".into(),
                        line: 42,
                        content: "TODO: Add error handling".into(),
                    },
                    SearchMatch {
                        file: "src/lib.rs".into(),
                        line: 78,
                        content: "TODO: Implement caching".into(),
                    },
                    SearchMatch {
                        file: "src/utils.rs".into(),
                        line: 15,
                        content: "TODO: Refactor this function".into(),
                    },
                    SearchMatch {
                        file: "tests/integration.rs".into(),
                        line: 23,
                        content: "TODO: Add more test cases".into(),
                    },
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
                 The main areas needing attention are error handling, caching, and test coverage.",
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
                    DiffLine {
                        kind: DiffLineKind::Header,
                        content: "@@ -45,6 +45,12 @@".into(),
                        old_line: None,
                        new_line: None,
                    },
                    DiffLine {
                        kind: DiffLineKind::Context,
                        content: "fn process_data(input: &str) -> String {".into(),
                        old_line: Some(45),
                        new_line: Some(45),
                    },
                    DiffLine {
                        kind: DiffLineKind::Deletion,
                        content: "    input.parse().unwrap()".into(),
                        old_line: Some(46),
                        new_line: None,
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "    match input.parse() {".into(),
                        old_line: None,
                        new_line: Some(46),
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "        Ok(val) => val,".into(),
                        old_line: None,
                        new_line: Some(47),
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "        Err(e) => {".into(),
                        old_line: None,
                        new_line: Some(48),
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "            eprintln!(\"Parse error: {}\", e);".into(),
                        old_line: None,
                        new_line: Some(49),
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "            String::new()".into(),
                        old_line: None,
                        new_line: Some(50),
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "        }".into(),
                        old_line: None,
                        new_line: Some(51),
                    },
                    DiffLine {
                        kind: DiffLineKind::Addition,
                        content: "    }".into(),
                        old_line: None,
                        new_line: Some(52),
                    },
                    DiffLine {
                        kind: DiffLineKind::Context,
                        content: "}".into(),
                        old_line: Some(47),
                        new_line: Some(53),
                    },
                ]);
            diff_tool.paint(Bounds::new(inner.origin.x, entry_y, entry_w, diff_h), cx);
            entry_y += diff_h + entry_gap;

            // Completion message
            let complete_h = 80.0;
            let mut complete = AssistantMessage::new(
                "I've added proper error handling with a match statement. The function now logs \
                 parse errors to stderr and returns an empty string instead of panicking.",
            )
            .model(Model::ClaudeSonnet)
            .timestamp("3:43 PM");
            complete.paint(
                Bounds::new(inner.origin.x, entry_y, entry_w, complete_h),
                cx,
            );
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
                    SearchMatch {
                        file: "src/auth/mod.rs".into(),
                        line: 12,
                        content: "pub fn authenticate(token: &str) -> Result<User, AuthError>"
                            .into(),
                    },
                    SearchMatch {
                        file: "src/auth/jwt.rs".into(),
                        line: 45,
                        content: "fn authenticate_jwt(token: &str) -> Result<Claims, JwtError>"
                            .into(),
                    },
                    SearchMatch {
                        file: "src/middleware/auth.rs".into(),
                        line: 28,
                        content: "async fn authenticate(req: Request) -> Result<Response, Error>"
                            .into(),
                    },
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
                 Include input validation and proper error responses.",
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
                 4. Update API documentation",
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
                .output(
                    "running 5 tests\ntest api::preferences::tests::test_get_prefs ... ok\n\
                         test api::preferences::tests::test_post_prefs ... ok\n\
                         test api::preferences::tests::test_validation ... ok\n\n\
                         test result: ok. 5 passed; 0 failed",
                );
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
                 - All 5 tests pass and documentation is updated",
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
            let mut user_msg =
                UserMessage::new("Run the database migration script").timestamp("5:30 PM");
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
                 Please start PostgreSQL with `sudo systemctl start postgresql` and try again.",
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
                icon.paint(
                    Bounds::new(tile_x + 12.0, tile_y + 14.0, tile_w - 24.0, 24.0),
                    cx,
                );
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
        draw_panel(
            "Bitcoin Amount Formatting",
            amounts_bounds,
            cx,
            |inner, cx| {
                let amounts_data = [
                    (
                        1000,
                        AmountDirection::Neutral,
                        BitcoinUnit::Sats,
                        "Small amount",
                    ),
                    (
                        50000,
                        AmountDirection::Incoming,
                        BitcoinUnit::Sats,
                        "Incoming payment",
                    ),
                    (
                        25000,
                        AmountDirection::Outgoing,
                        BitcoinUnit::Sats,
                        "Outgoing payment",
                    ),
                    (
                        100_000_000,
                        AmountDirection::Neutral,
                        BitcoinUnit::Btc,
                        "One Bitcoin",
                    ),
                    (
                        2_100_000_000_000_000,
                        AmountDirection::Neutral,
                        BitcoinUnit::Btc,
                        "Max supply",
                    ),
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
            },
        );
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
            mainnet_card.paint(
                Bounds::new(inner.origin.x, inner.origin.y, 300.0, 180.0),
                cx,
            );

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
                row.paint(
                    Bounds::new(inner.origin.x, row_y, inner.size.width, row_h),
                    cx,
                );
            }
        });
        y += txn_height + SECTION_GAP;

        // ========== Panel 7: Invoice Displays ==========
        let invoice_height = panel_height(320.0);
        let invoice_bounds = Bounds::new(bounds.origin.x, y, width, invoice_height);
        draw_panel(
            "Invoice & Address Displays",
            invoice_bounds,
            cx,
            |inner, cx| {
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
                ln_display.paint(
                    Bounds::new(inner.origin.x, inner.origin.y, 320.0, 280.0),
                    cx,
                );

                // Spark address (compact)
                let spark_addr = InvoiceInfo::new(
                    InvoiceType::SparkAddress,
                    "sp1qw508d6qejxtdg4y5r3zarvary0c5xw7kxpjzsx",
                )
                .status(PaymentStatus::Pending);
                let mut spark_display =
                    InvoiceDisplay::new(spark_addr).show_qr(false).compact(true);
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
                    btc_display.paint(
                        Bounds::new(spark_x, inner.origin.y + 140.0, 320.0, 120.0),
                        cx,
                    );
                }
            },
        );
        y += invoice_height + SECTION_GAP;

        // ========== Panel 8: Complete Wallet Dashboard ==========
        let dashboard_height = panel_height(400.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel(
            "Complete Wallet Dashboard",
            dashboard_bounds,
            cx,
            |inner, cx| {
                // Left column: Balance card
                let col_w = (inner.size.width - 20.0) / 2.0;

                let balance = WalletBalance::new(250000, 100000, 50000);
                let mut balance_card = BalanceCard::new(balance)
                    .network(BitcoinNetwork::Mainnet)
                    .show_breakdown(true);
                balance_card.paint(
                    Bounds::new(inner.origin.x, inner.origin.y, col_w.min(320.0), 180.0),
                    cx,
                );

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
            },
        );
    }

    fn paint_nostr_protocol(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Relay Status Indicators ==========
        let status_height = panel_height(160.0);
        let status_bounds = Bounds::new(bounds.origin.x, y, width, status_height);
        draw_panel("Relay Status Indicators", status_bounds, cx, |inner, cx| {
            let statuses = [
                RelayStatus::Connected,
                RelayStatus::Connecting,
                RelayStatus::Disconnected,
                RelayStatus::Error,
                RelayStatus::Authenticating,
            ];

            // Row 1: Status dots
            let mut dot_x = inner.origin.x;
            let dot_y = inner.origin.y;
            let dot_run = cx.text.layout(
                "Status Dots:",
                Point::new(dot_x, dot_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(dot_run);

            dot_x = inner.origin.x;
            for status in &statuses {
                let mut dot = RelayStatusDot::new(*status).size(12.0).show_label(true);
                dot.paint(Bounds::new(dot_x, dot_y + 20.0, 60.0, 16.0), cx);
                dot_x += 80.0;
            }

            // Row 2: Status badges
            let badge_y = dot_y + 56.0;
            let badge_run = cx.text.layout(
                "Status Badges:",
                Point::new(inner.origin.x, badge_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(badge_run);

            let mut badge_x = inner.origin.x;
            for status in &statuses {
                let mut badge = RelayStatusBadge::new(*status);
                badge.paint(Bounds::new(badge_x, badge_y + 20.0, 90.0, 22.0), cx);
                badge_x += 100.0;
            }
        });
        y += status_height + SECTION_GAP;

        // ========== Panel 2: Event Kind Badges ==========
        let kinds_height = panel_height(280.0);
        let kinds_bounds = Bounds::new(bounds.origin.x, y, width, kinds_height);
        draw_panel("Event Kind Badges", kinds_bounds, cx, |inner, cx| {
            let kinds = [
                (EventKind::TextNote, "Social"),
                (EventKind::Metadata, "Identity"),
                (EventKind::Contacts, "Identity"),
                (EventKind::EncryptedDm, "Messaging"),
                (EventKind::Reaction, "Social"),
                (EventKind::ZapReceipt, "Payments"),
                (EventKind::RepoAnnounce, "Git"),
                (EventKind::Issue, "Git"),
                (EventKind::Patch, "Git"),
                (EventKind::PullRequest, "Git"),
                (EventKind::AgentProfile, "Agents"),
                (EventKind::TrajectorySession, "Agents"),
                (EventKind::DvmTextRequest, "DVM"),
                (EventKind::DvmTextResult, "DVM"),
                (EventKind::LongFormContent, "Content"),
                (EventKind::Custom(99999), "Custom"),
            ];

            let tile_w = 100.0;
            let tile_h = 50.0;
            let gap = 8.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (kind, category)) in kinds.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Category label
                let cat_run = cx.text.layout(
                    *category,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(cat_run);

                // Event kind badge
                let mut badge = EventKindBadge::new(kind.clone()).show_number(true);
                badge.paint(Bounds::new(tile_x, tile_y + 16.0, tile_w - 4.0, 22.0), cx);
            }
        });
        y += kinds_height + SECTION_GAP;

        // ========== Panel 3: Bech32 Entities ==========
        let entities_height = panel_height(200.0);
        let entities_bounds = Bounds::new(bounds.origin.x, y, width, entities_height);
        draw_panel(
            "Bech32 Entities (NIP-19)",
            entities_bounds,
            cx,
            |inner, cx| {
                let entities = [
                    (
                        Bech32Type::Npub,
                        "npub1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqsutj2v5",
                    ),
                    (
                        Bech32Type::Note,
                        "note1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq5nkr4f",
                    ),
                    (
                        Bech32Type::Nevent,
                        "nevent1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqnmupg",
                    ),
                    (
                        Bech32Type::Nprofile,
                        "nprofile1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqsdp8el",
                    ),
                    (
                        Bech32Type::Nsec,
                        "nsec1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq9wlz3w",
                    ),
                    (Bech32Type::Nrelay, "nrelay1qqxrgvfex9j3n8qerc94kk"),
                ];

                let row_h = 40.0;
                let gap = 8.0;

                for (idx, (entity_type, value)) in entities.iter().enumerate() {
                    let row_y = inner.origin.y + idx as f32 * (row_h + gap);
                    let mut entity = Bech32Entity::new(*entity_type, *value)
                        .show_prefix_badge(true)
                        .truncate(true);
                    entity.paint(
                        Bounds::new(
                            inner.origin.x,
                            row_y,
                            inner.size.width.min(400.0),
                            row_h - 4.0,
                        ),
                        cx,
                    );
                }
            },
        );
        y += entities_height + SECTION_GAP;

        // ========== Panel 4: Relay Connection List ==========
        let relays_height = panel_height(300.0);
        let relays_bounds = Bounds::new(bounds.origin.x, y, width, relays_height);
        draw_panel("Relay Connection List", relays_bounds, cx, |inner, cx| {
            let relays = [
                RelayInfo::new("wss://relay.damus.io")
                    .status(RelayStatus::Connected)
                    .read(true)
                    .write(true)
                    .events(15420, 342)
                    .latency(45),
                RelayInfo::new("wss://nos.lol")
                    .status(RelayStatus::Connected)
                    .read(true)
                    .write(true)
                    .events(8934, 156)
                    .latency(78),
                RelayInfo::new("wss://relay.nostr.band")
                    .status(RelayStatus::Connecting)
                    .read(true)
                    .write(false)
                    .events(0, 0),
                RelayInfo::new("wss://purplepag.es")
                    .status(RelayStatus::Connected)
                    .read(true)
                    .write(false)
                    .events(2341, 0)
                    .latency(120),
                RelayInfo::new("wss://relay.snort.social")
                    .status(RelayStatus::Disconnected)
                    .read(true)
                    .write(true)
                    .events(0, 0),
                RelayInfo::new("wss://offchain.pub")
                    .status(RelayStatus::Error)
                    .read(true)
                    .write(true)
                    .events(0, 0),
            ];

            let row_h = 44.0;
            let gap = 4.0;

            for (idx, relay) in relays.iter().enumerate() {
                let row_y = inner.origin.y + idx as f32 * (row_h + gap);
                let mut row = RelayRow::new(relay.clone());
                row.paint(
                    Bounds::new(inner.origin.x, row_y, inner.size.width.min(500.0), row_h),
                    cx,
                );
            }
        });
        y += relays_height + SECTION_GAP;

        // ========== Panel 5: Complete Relay Dashboard ==========
        let dashboard_height = panel_height(320.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel(
            "Complete Relay Dashboard",
            dashboard_bounds,
            cx,
            |inner, cx| {
                // Dashboard header
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width,
                        40.0,
                    ))
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::accent::PRIMARY, 1.0),
                );

                let header_run = cx.text.layout(
                    "Nostr Relay Pool",
                    Point::new(inner.origin.x + 12.0, inner.origin.y + 12.0),
                    theme::font_size::BASE,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(header_run);

                // Stats summary
                let stats_run = cx.text.layout(
                    "4 Connected | 1 Connecting | 1 Error",
                    Point::new(
                        inner.origin.x + inner.size.width - 220.0,
                        inner.origin.y + 14.0,
                    ),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(stats_run);

                // Split into two columns
                let col_gap = 24.0;
                let col_w = (inner.size.width - col_gap) / 2.0;
                let content_y = inner.origin.y + 52.0;

                // Left column: Active relays
                let left_x = inner.origin.x;
                let active_label = cx.text.layout(
                    "Active Relays",
                    Point::new(left_x, content_y),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(active_label);

                let active_relays = [
                    RelayInfo::new("wss://relay.damus.io")
                        .status(RelayStatus::Connected)
                        .events(15420, 342)
                        .latency(45),
                    RelayInfo::new("wss://nos.lol")
                        .status(RelayStatus::Connected)
                        .events(8934, 156)
                        .latency(78),
                ];

                let row_h = 36.0;
                let row_gap = 4.0;
                let relay_y = content_y + 24.0;

                for (idx, relay) in active_relays.iter().enumerate() {
                    let row_y = relay_y + idx as f32 * (row_h + row_gap);
                    let mut row = RelayRow::new(relay.clone()).compact(true);
                    row.paint(Bounds::new(left_x, row_y, col_w.min(320.0), row_h), cx);
                }

                // Right column: Event statistics
                let right_x = inner.origin.x + col_w + col_gap;
                let stats_label = cx.text.layout(
                    "Event Statistics",
                    Point::new(right_x, content_y),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(stats_label);

                // Event kind summary
                let event_summary = [
                    ("Notes (kind:1)", "12,453"),
                    ("Reactions (kind:7)", "8,921"),
                    ("Profiles (kind:0)", "1,234"),
                    ("Zaps (kind:9735)", "456"),
                    ("DMs (kind:4)", "89"),
                ];

                let stat_y = content_y + 24.0;
                for (idx, (label, count)) in event_summary.iter().enumerate() {
                    let y_pos = stat_y + idx as f32 * 28.0;

                    let label_run = cx.text.layout(
                        *label,
                        Point::new(right_x, y_pos),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(label_run);

                    let count_run = cx.text.layout(
                        *count,
                        Point::new(right_x + 140.0, y_pos),
                        theme::font_size::SM,
                        theme::text::PRIMARY,
                    );
                    cx.scene.draw_text(count_run);
                }
            },
        );
    }

    fn paint_gitafter(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Issue Status Badges ==========
        let issue_height = panel_height(160.0);
        let issue_bounds = Bounds::new(bounds.origin.x, y, width, issue_height);
        draw_panel("Issue Status Badges", issue_bounds, cx, |inner, cx| {
            let statuses = [
                IssueStatus::Open,
                IssueStatus::Claimed,
                IssueStatus::InProgress,
                IssueStatus::Closed,
                IssueStatus::Draft,
            ];

            let tile_w = 100.0;
            let gap = 12.0;

            for (idx, status) in statuses.iter().enumerate() {
                let tile_x = inner.origin.x + idx as f32 * (tile_w + gap);
                let tile_y = inner.origin.y;

                // Label
                let label_run = cx.text.layout(
                    status.label(),
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = IssueStatusBadge::new(*status);
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 80.0, 22.0), cx);

                // Compact version
                let mut compact = IssueStatusBadge::new(*status).compact(true);
                compact.paint(Bounds::new(tile_x, tile_y + 48.0, 24.0, 22.0), cx);
            }
        });
        y += issue_height + SECTION_GAP;

        // ========== Panel 2: PR Status Badges ==========
        let pr_height = panel_height(180.0);
        let pr_bounds = Bounds::new(bounds.origin.x, y, width, pr_height);
        draw_panel("PR Status Badges", pr_bounds, cx, |inner, cx| {
            let statuses = [
                PrStatus::Draft,
                PrStatus::Open,
                PrStatus::NeedsReview,
                PrStatus::Approved,
                PrStatus::ChangesRequested,
                PrStatus::Merged,
                PrStatus::Closed,
            ];

            let tile_w = 80.0;
            let tile_h = 60.0;
            let gap = 8.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, status) in statuses.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    status.label(),
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = PrStatusBadge::new(*status);
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 70.0, 22.0), cx);
            }
        });
        y += pr_height + SECTION_GAP;

        // ========== Panel 3: Bounty Badges ==========
        let bounty_height = panel_height(140.0);
        let bounty_bounds = Bounds::new(bounds.origin.x, y, width, bounty_height);
        draw_panel("Bounty Badges", bounty_bounds, cx, |inner, cx| {
            let bounties = [
                (500, BountyStatus::Active),
                (5000, BountyStatus::Active),
                (50000, BountyStatus::Claimed),
                (100000, BountyStatus::Paid),
                (25000, BountyStatus::Expired),
            ];

            let tile_w = 110.0;
            let gap = 12.0;

            for (idx, (amount, status)) in bounties.iter().enumerate() {
                let tile_x = inner.origin.x + idx as f32 * (tile_w + gap);
                let tile_y = inner.origin.y;

                // Status label
                let label_run = cx.text.layout(
                    status.label(),
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Bounty badge
                let mut badge = BountyBadge::new(*amount).status(*status);
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 100.0, 24.0), cx);
            }
        });
        y += bounty_height + SECTION_GAP;

        // ========== Panel 4: Stack Layer Indicators ==========
        let stack_height = panel_height(160.0);
        let stack_bounds = Bounds::new(bounds.origin.x, y, width, stack_height);
        draw_panel("Stack Layer Indicators", stack_bounds, cx, |inner, cx| {
            let stacks = [
                (1, 4, StackLayerStatus::Merged),
                (2, 4, StackLayerStatus::Ready),
                (3, 4, StackLayerStatus::Pending),
                (4, 4, StackLayerStatus::Blocked),
            ];

            let tile_w = 120.0;
            let gap = 12.0;

            for (idx, (layer, total, status)) in stacks.iter().enumerate() {
                let tile_x = inner.origin.x + idx as f32 * (tile_w + gap);
                let tile_y = inner.origin.y;

                // Full badge
                let mut badge = StackLayerBadge::new(*layer, *total).status(*status);
                badge.paint(Bounds::new(tile_x, tile_y, 80.0, 24.0), cx);

                // Compact badge
                let mut compact = StackLayerBadge::new(*layer, *total)
                    .status(*status)
                    .compact(true);
                compact.paint(Bounds::new(tile_x, tile_y + 32.0, 36.0, 22.0), cx);
            }
        });
        y += stack_height + SECTION_GAP;

        // ========== Panel 5: Agent Status Badges ==========
        let agent_height = panel_height(180.0);
        let agent_bounds = Bounds::new(bounds.origin.x, y, width, agent_height);
        draw_panel(
            "Agent Status & Type Badges",
            agent_bounds,
            cx,
            |inner, cx| {
                let statuses = [
                    AgentStatus::Online,
                    AgentStatus::Busy,
                    AgentStatus::Idle,
                    AgentStatus::Offline,
                    AgentStatus::Error,
                ];

                // Row 1: Agent statuses
                let mut x = inner.origin.x;
                for status in &statuses {
                    let mut badge = AgentStatusBadge::new(*status).show_dot(true);
                    badge.paint(Bounds::new(x, inner.origin.y, 80.0, 24.0), cx);
                    x += 90.0;
                }

                // Row 2: Agent types
                let types = [AgentType::Human, AgentType::Sovereign, AgentType::Custodial];

                let mut x = inner.origin.x;
                let row_y = inner.origin.y + 40.0;
                for agent_type in &types {
                    let mut badge =
                        AgentStatusBadge::new(AgentStatus::Online).agent_type(*agent_type);
                    badge.paint(Bounds::new(x, row_y, 100.0, 24.0), cx);
                    x += 110.0;
                }

                // Row 3: Combined status + type
                let combined = [
                    (AgentType::Sovereign, AgentStatus::Busy, "Working on issue"),
                    (AgentType::Human, AgentStatus::Online, "Reviewing PRs"),
                    (AgentType::Sovereign, AgentStatus::Idle, "Waiting for work"),
                ];

                let mut x = inner.origin.x;
                let row_y = inner.origin.y + 80.0;
                for (agent_type, status, desc) in &combined {
                    // Badge
                    let mut badge = AgentStatusBadge::new(*status).agent_type(*agent_type);
                    badge.paint(Bounds::new(x, row_y, 100.0, 24.0), cx);

                    // Description
                    let desc_run = cx.text.layout(
                        *desc,
                        Point::new(x, row_y + 28.0),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(desc_run);
                    x += 140.0;
                }
            },
        );
        y += agent_height + SECTION_GAP;

        // ========== Panel 6: Trajectory Status Badges ==========
        let traj_height = panel_height(160.0);
        let traj_bounds = Bounds::new(bounds.origin.x, y, width, traj_height);
        draw_panel("Trajectory Status Badges", traj_bounds, cx, |inner, cx| {
            let statuses = [
                TrajectoryStatus::Verified,
                TrajectoryStatus::Partial,
                TrajectoryStatus::HasGaps,
                TrajectoryStatus::Suspicious,
                TrajectoryStatus::Mismatch,
                TrajectoryStatus::Unknown,
            ];

            let tile_w = 100.0;
            let tile_h = 50.0;
            let gap = 8.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, status) in statuses.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Full badge
                let mut badge = TrajectoryStatusBadge::new(*status);
                badge.paint(Bounds::new(tile_x, tile_y, 95.0, 22.0), cx);

                // Compact
                let mut compact = TrajectoryStatusBadge::new(*status).compact(true);
                compact.paint(Bounds::new(tile_x + tile_w - 28.0, tile_y, 24.0, 22.0), cx);
            }
        });
        y += traj_height + SECTION_GAP;

        // ========== Panel 7: Complete GitAfter Dashboard ==========
        let dashboard_height = panel_height(360.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel(
            "GitAfter Dashboard Preview",
            dashboard_bounds,
            cx,
            |inner, cx| {
                // Header bar
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width,
                        40.0,
                    ))
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::accent::PRIMARY, 1.0),
                );

                let title_run = cx.text.layout(
                    "openagents/openagents",
                    Point::new(inner.origin.x + 12.0, inner.origin.y + 12.0),
                    theme::font_size::BASE,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(title_run);

                // Issue row example
                let issue_y = inner.origin.y + 52.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(inner.origin.x, issue_y, inner.size.width, 56.0))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                // Issue status
                let mut issue_status = IssueStatusBadge::new(IssueStatus::Open);
                issue_status.paint(
                    Bounds::new(inner.origin.x + 8.0, issue_y + 17.0, 60.0, 22.0),
                    cx,
                );

                // Issue title
                let issue_title = cx.text.layout(
                    "#42: Add NIP-SA trajectory publishing",
                    Point::new(inner.origin.x + 76.0, issue_y + 8.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(issue_title);

                // Bounty
                let mut bounty = BountyBadge::new(50000).status(BountyStatus::Active);
                bounty.paint(
                    Bounds::new(inner.origin.x + 76.0, issue_y + 28.0, 90.0, 22.0),
                    cx,
                );

                // Agent claimant
                let claimed_run = cx.text.layout(
                    "Claimed by npub1agent...",
                    Point::new(inner.origin.x + 180.0, issue_y + 32.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(claimed_run);

                // PR row example
                let pr_y = issue_y + 68.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(inner.origin.x, pr_y, inner.size.width, 72.0))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                // PR status
                let mut pr_status = PrStatusBadge::new(PrStatus::Open);
                pr_status.paint(
                    Bounds::new(inner.origin.x + 8.0, pr_y + 8.0, 60.0, 22.0),
                    cx,
                );

                // Stack layer
                let mut stack_layer = StackLayerBadge::new(2, 4).status(StackLayerStatus::Ready);
                stack_layer.paint(
                    Bounds::new(inner.origin.x + 76.0, pr_y + 8.0, 80.0, 24.0),
                    cx,
                );

                // PR title
                let pr_title = cx.text.layout(
                    "Layer 2: Wire trajectory events to relay pool",
                    Point::new(inner.origin.x + 164.0, pr_y + 10.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(pr_title);

                // Agent author + trajectory
                let mut agent =
                    AgentStatusBadge::new(AgentStatus::Busy).agent_type(AgentType::Sovereign);
                agent.paint(
                    Bounds::new(inner.origin.x + 8.0, pr_y + 38.0, 100.0, 24.0),
                    cx,
                );

                let mut traj = TrajectoryStatusBadge::new(TrajectoryStatus::Verified);
                traj.paint(
                    Bounds::new(inner.origin.x + 116.0, pr_y + 40.0, 80.0, 22.0),
                    cx,
                );

                // "depends on layer 1" indicator
                let depends_run = cx.text.layout(
                    "Depends on: Layer 1 (merged)",
                    Point::new(inner.origin.x + 210.0, pr_y + 44.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(depends_run);
            },
        );
    }

    fn paint_sovereign_agents(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Threshold Key Badges ==========
        let threshold_height = panel_height(160.0);
        let threshold_bounds = Bounds::new(bounds.origin.x, y, width, threshold_height);
        draw_panel("Threshold Key Badges", threshold_bounds, cx, |inner, cx| {
            let configs = [
                (2, 3, 2, "2-of-3 (ready)"),
                (2, 3, 1, "2-of-3 (partial)"),
                (3, 5, 3, "3-of-5 (ready)"),
                (3, 5, 2, "3-of-5 (partial)"),
                (2, 3, 0, "2-of-3 (unknown)"),
            ];

            let tile_w = 130.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (threshold, total, available, label)) in configs.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Full badge
                let mut badge =
                    ThresholdKeyBadge::new(*threshold, *total).shares_available(*available);
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 120.0, 24.0), cx);
            }
        });
        y += threshold_height + SECTION_GAP;

        // ========== Panel 2: Agent Schedule Badges ==========
        let schedule_height = panel_height(180.0);
        let schedule_bounds = Bounds::new(bounds.origin.x, y, width, schedule_height);
        draw_panel("Agent Schedule Badges", schedule_bounds, cx, |inner, cx| {
            let schedules = [
                (
                    900,
                    vec![TriggerType::Mention, TriggerType::DirectMessage],
                    "15m + mentions/DMs",
                ),
                (
                    3600,
                    vec![TriggerType::Zap, TriggerType::Issue],
                    "1h + zaps/issues",
                ),
                (7200, vec![TriggerType::PullRequest], "2h + PRs"),
                (300, vec![], "5m heartbeat only"),
            ];

            let tile_w = 160.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (seconds, triggers, label)) in schedules.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Full badge
                let mut badge = AgentScheduleBadge::new(*seconds).triggers(triggers.clone());
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 140.0, 24.0), cx);
            }
        });
        y += schedule_height + SECTION_GAP;

        // ========== Panel 3: Goal Progress Badges ==========
        let goal_height = panel_height(160.0);
        let goal_bounds = Bounds::new(bounds.origin.x, y, width, goal_height);
        draw_panel("Goal Progress Badges", goal_bounds, cx, |inner, cx| {
            let goals = [
                (
                    0.0,
                    GoalStatus::NotStarted,
                    GoalPriority::Medium,
                    "Not started",
                ),
                (
                    0.35,
                    GoalStatus::InProgress,
                    GoalPriority::High,
                    "In progress",
                ),
                (
                    0.65,
                    GoalStatus::InProgress,
                    GoalPriority::Critical,
                    "Critical",
                ),
                (
                    1.0,
                    GoalStatus::Completed,
                    GoalPriority::Medium,
                    "Completed",
                ),
                (0.5, GoalStatus::Blocked, GoalPriority::High, "Blocked"),
                (0.8, GoalStatus::Failed, GoalPriority::Critical, "Failed"),
            ];

            let tile_w = 140.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (progress, status, priority, label)) in goals.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = GoalProgressBadge::new(*progress)
                    .status(*status)
                    .priority(*priority);
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 125.0, 22.0), cx);
            }
        });
        y += goal_height + SECTION_GAP;

        // ========== Panel 4: Tick Event Badges ==========
        let tick_height = panel_height(180.0);
        let tick_bounds = Bounds::new(bounds.origin.x, y, width, tick_height);
        draw_panel("Tick Event Badges", tick_bounds, cx, |inner, cx| {
            // Row 1: Tick outcomes
            let outcomes = [
                TickOutcome::Pending,
                TickOutcome::Success,
                TickOutcome::Failure,
                TickOutcome::Timeout,
                TickOutcome::Skipped,
            ];

            let mut x = inner.origin.x;
            for outcome in &outcomes {
                let mut badge = TickEventBadge::result(*outcome).duration_ms(1500);
                badge.paint(Bounds::new(x, inner.origin.y, 110.0, 22.0), cx);
                x += 120.0;
            }

            // Row 2: Request vs Result
            let row_y = inner.origin.y + 40.0;
            let req_label = cx.text.layout(
                "Request",
                Point::new(inner.origin.x, row_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(req_label);

            let mut request = TickEventBadge::request();
            request.paint(Bounds::new(inner.origin.x, row_y + 18.0, 80.0, 22.0), cx);

            let res_label = cx.text.layout(
                "Result (success, 2.3s)",
                Point::new(inner.origin.x + 120.0, row_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(res_label);

            let mut result = TickEventBadge::result(TickOutcome::Success).duration_ms(2300);
            result.paint(
                Bounds::new(inner.origin.x + 120.0, row_y + 18.0, 130.0, 22.0),
                cx,
            );

            // Compact versions
            let compact_y = row_y + 50.0;
            let compact_label = cx.text.layout(
                "Compact:",
                Point::new(inner.origin.x, compact_y + 2.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(compact_label);

            let mut x = inner.origin.x + 60.0;
            for outcome in &outcomes {
                let mut compact = TickEventBadge::result(*outcome).compact(true);
                compact.paint(Bounds::new(x, compact_y, 28.0, 22.0), cx);
                x += 36.0;
            }
        });
        y += tick_height + SECTION_GAP;

        // ========== Panel 5: Skill License Badges ==========
        let skill_height = panel_height(180.0);
        let skill_bounds = Bounds::new(bounds.origin.x, y, width, skill_height);
        draw_panel("Skill License Badges", skill_bounds, cx, |inner, cx| {
            let skills = [
                (SkillType::Code, LicenseStatus::Active, Some("git-rebase")),
                (SkillType::Data, LicenseStatus::Active, Some("market-data")),
                (SkillType::Model, LicenseStatus::Pending, Some("sonnet-4.5")),
                (SkillType::Tool, LicenseStatus::Expired, Some("browser-use")),
                (SkillType::Code, LicenseStatus::Revoked, None),
            ];

            let tile_w = 150.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (skill_type, status, name)) in skills.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Type label
                let type_label = match skill_type {
                    SkillType::Code => "Code Skill",
                    SkillType::Data => "Data Skill",
                    SkillType::Model => "Model Skill",
                    SkillType::Tool => "Tool Skill",
                };
                let label_run = cx.text.layout(
                    type_label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = SkillLicenseBadge::new(*skill_type, *status);
                if let Some(n) = name {
                    badge = badge.name(*n);
                }
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 140.0, 22.0), cx);
            }
        });
        y += skill_height + SECTION_GAP;

        // ========== Panel 6: Complete Agent Dashboard ==========
        let dashboard_height = panel_height(400.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel(
            "Sovereign Agent Dashboard Preview",
            dashboard_bounds,
            cx,
            |inner, cx| {
                // Header bar with agent identity
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width,
                        50.0,
                    ))
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::accent::PRIMARY, 1.0),
                );

                // Agent icon + name
                let agent_icon = cx.text.layout(
                    "🤖",
                    Point::new(inner.origin.x + 12.0, inner.origin.y + 14.0),
                    theme::font_size::LG,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(agent_icon);

                let agent_name = cx.text.layout(
                    "code-monkey-42",
                    Point::new(inner.origin.x + 40.0, inner.origin.y + 8.0),
                    theme::font_size::BASE,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(agent_name);

                let npub = cx.text.layout(
                    "npub1agent42xyz...",
                    Point::new(inner.origin.x + 40.0, inner.origin.y + 28.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(npub);

                // Status badges on right side of header
                let mut status =
                    AgentStatusBadge::new(AgentStatus::Online).agent_type(AgentType::Sovereign);
                status.paint(
                    Bounds::new(
                        inner.origin.x + inner.size.width - 200.0,
                        inner.origin.y + 13.0,
                        100.0,
                        24.0,
                    ),
                    cx,
                );

                let mut threshold = ThresholdKeyBadge::new(2, 3).shares_available(2);
                threshold.paint(
                    Bounds::new(
                        inner.origin.x + inner.size.width - 90.0,
                        inner.origin.y + 13.0,
                        80.0,
                        24.0,
                    ),
                    cx,
                );

                // Schedule row
                let sched_y = inner.origin.y + 60.0;
                let sched_label = cx.text.layout(
                    "Schedule:",
                    Point::new(inner.origin.x + 8.0, sched_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(sched_label);

                let mut schedule = AgentScheduleBadge::new(900).triggers(vec![
                    TriggerType::Mention,
                    TriggerType::Zap,
                    TriggerType::Issue,
                ]);
                schedule.paint(Bounds::new(inner.origin.x + 70.0, sched_y, 140.0, 24.0), cx);

                // Goals section
                let goals_y = sched_y + 35.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(inner.origin.x, goals_y, inner.size.width, 90.0))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                let goals_title = cx.text.layout(
                    "Current Goals",
                    Point::new(inner.origin.x + 8.0, goals_y + 6.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(goals_title);

                // Goal 1
                let mut goal1 = GoalProgressBadge::new(0.75)
                    .status(GoalStatus::InProgress)
                    .priority(GoalPriority::High);
                goal1.paint(
                    Bounds::new(inner.origin.x + 8.0, goals_y + 28.0, 125.0, 22.0),
                    cx,
                );
                let goal1_desc = cx.text.layout(
                    "Fix d-006 Phase 4 issues",
                    Point::new(inner.origin.x + 142.0, goals_y + 32.0),
                    theme::font_size::XS,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(goal1_desc);

                // Goal 2
                let mut goal2 = GoalProgressBadge::new(0.3)
                    .status(GoalStatus::InProgress)
                    .priority(GoalPriority::Medium);
                goal2.paint(
                    Bounds::new(inner.origin.x + 8.0, goals_y + 56.0, 125.0, 22.0),
                    cx,
                );
                let goal2_desc = cx.text.layout(
                    "Publish trajectory events",
                    Point::new(inner.origin.x + 142.0, goals_y + 60.0),
                    theme::font_size::XS,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(goal2_desc);

                // Skills section
                let skills_y = goals_y + 100.0;
                let skills_label = cx.text.layout(
                    "Licensed Skills:",
                    Point::new(inner.origin.x + 8.0, skills_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(skills_label);

                let mut skill1 =
                    SkillLicenseBadge::new(SkillType::Code, LicenseStatus::Active).name("git-ops");
                skill1.paint(
                    Bounds::new(inner.origin.x + 100.0, skills_y, 120.0, 22.0),
                    cx,
                );

                let mut skill2 = SkillLicenseBadge::new(SkillType::Model, LicenseStatus::Active)
                    .name("opus-4.5");
                skill2.paint(
                    Bounds::new(inner.origin.x + 230.0, skills_y, 130.0, 22.0),
                    cx,
                );

                // Recent ticks section
                let ticks_y = skills_y + 35.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        ticks_y,
                        inner.size.width,
                        100.0,
                    ))
                    .with_background(theme::bg::SURFACE)
                    .with_border(theme::border::DEFAULT, 1.0),
                );

                let ticks_title = cx.text.layout(
                    "Recent Ticks",
                    Point::new(inner.origin.x + 8.0, ticks_y + 6.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(ticks_title);

                // Tick timeline
                let tick_row_y = ticks_y + 30.0;
                let times = ["2m ago", "17m ago", "32m ago", "47m ago"];
                let outcomes = [
                    TickOutcome::Success,
                    TickOutcome::Success,
                    TickOutcome::Failure,
                    TickOutcome::Success,
                ];
                let durations = [1200, 890, 0, 2300];

                for (i, ((time, outcome), dur)) in times
                    .iter()
                    .zip(outcomes.iter())
                    .zip(durations.iter())
                    .enumerate()
                {
                    let tick_x = inner.origin.x + 8.0 + i as f32 * 100.0;

                    let time_run = cx.text.layout(
                        *time,
                        Point::new(tick_x, tick_row_y),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(time_run);

                    let mut tick = if *dur > 0 {
                        TickEventBadge::result(*outcome).duration_ms(*dur as u64)
                    } else {
                        TickEventBadge::result(*outcome)
                    };
                    tick.paint(Bounds::new(tick_x, tick_row_y + 16.0, 90.0, 22.0), cx);
                }

                // Trajectory hash
                let traj_y = ticks_y + 72.0;
                let traj_label = cx.text.layout(
                    "Current trajectory:",
                    Point::new(inner.origin.x + 8.0, traj_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(traj_label);

                let mut traj = TrajectoryStatusBadge::new(TrajectoryStatus::Verified);
                traj.paint(Bounds::new(inner.origin.x + 120.0, traj_y, 80.0, 22.0), cx);

                let hash = cx.text.layout(
                    "hash: 7c6267e85a...",
                    Point::new(inner.origin.x + 210.0, traj_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(hash);
            },
        );
    }

    fn paint_marketplace(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Market Type Badges ==========
        let market_height = panel_height(140.0);
        let market_bounds = Bounds::new(bounds.origin.x, y, width, market_height);
        draw_panel("Market Type Badges", market_bounds, cx, |inner, cx| {
            let types = [
                MarketType::Compute,
                MarketType::Skills,
                MarketType::Data,
                MarketType::Trajectories,
            ];

            let mut x = inner.origin.x;
            for market_type in &types {
                // Full badge
                let mut badge = MarketTypeBadge::new(*market_type);
                badge.paint(Bounds::new(x, inner.origin.y, 90.0, 22.0), cx);

                // Compact badge
                let mut compact = MarketTypeBadge::new(*market_type).compact(true);
                compact.paint(Bounds::new(x, inner.origin.y + 30.0, 28.0, 22.0), cx);

                x += 100.0;
            }
        });
        y += market_height + SECTION_GAP;

        // ========== Panel 2: Job Status Badges ==========
        let job_height = panel_height(180.0);
        let job_bounds = Bounds::new(bounds.origin.x, y, width, job_height);
        draw_panel(
            "Job Status Badges (NIP-90 DVM)",
            job_bounds,
            cx,
            |inner, cx| {
                let statuses = [
                    (JobStatus::Pending, None, "Pending"),
                    (JobStatus::Processing, None, "Processing"),
                    (JobStatus::Streaming, None, "Streaming"),
                    (JobStatus::Completed, Some(150), "Completed"),
                    (JobStatus::Failed, None, "Failed"),
                    (JobStatus::Cancelled, None, "Cancelled"),
                ];

                let tile_w = 110.0;
                let tile_h = 55.0;
                let gap = 12.0;
                let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

                for (idx, (status, cost, label)) in statuses.iter().enumerate() {
                    let row = idx / cols;
                    let col = idx % cols;
                    let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                    let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                    // Label
                    let label_run = cx.text.layout(
                        *label,
                        Point::new(tile_x, tile_y),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(label_run);

                    // Badge
                    let mut badge = JobStatusBadge::new(*status);
                    if let Some(sats) = cost {
                        badge = badge.cost_sats(*sats);
                    }
                    badge.paint(Bounds::new(tile_x, tile_y + 18.0, 100.0, 22.0), cx);
                }
            },
        );
        y += job_height + SECTION_GAP;

        // ========== Panel 3: Reputation Badges ==========
        let rep_height = panel_height(160.0);
        let rep_bounds = Bounds::new(bounds.origin.x, y, width, rep_height);
        draw_panel(
            "Reputation & Trust Tier Badges",
            rep_bounds,
            cx,
            |inner, cx| {
                let tiers = [
                    (TrustTier::New, None, "New provider"),
                    (TrustTier::Established, Some(0.85), "Established"),
                    (TrustTier::Trusted, Some(0.95), "Trusted"),
                    (TrustTier::Expert, Some(0.99), "Expert"),
                ];

                let tile_w = 130.0;
                let tile_h = 55.0;
                let gap = 12.0;
                let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

                for (idx, (tier, rate, label)) in tiers.iter().enumerate() {
                    let row = idx / cols;
                    let col = idx % cols;
                    let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                    let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                    // Label
                    let label_run = cx.text.layout(
                        *label,
                        Point::new(tile_x, tile_y),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(label_run);

                    // Badge
                    let mut badge = ReputationBadge::new(*tier);
                    if let Some(r) = rate {
                        badge = badge.success_rate(*r);
                    }
                    badge.paint(Bounds::new(tile_x, tile_y + 18.0, 120.0, 22.0), cx);
                }
            },
        );
        y += rep_height + SECTION_GAP;

        // ========== Panel 4: Trajectory Source Badges ==========
        let traj_height = panel_height(180.0);
        let traj_bounds = Bounds::new(bounds.origin.x, y, width, traj_height);
        draw_panel("Trajectory Source Badges", traj_bounds, cx, |inner, cx| {
            let sources = [
                (
                    TrajectorySource::Claude,
                    Some(ContributionStatus::Accepted),
                    Some(42),
                ),
                (
                    TrajectorySource::Cursor,
                    Some(ContributionStatus::Pending),
                    Some(15),
                ),
                (
                    TrajectorySource::Codex,
                    Some(ContributionStatus::Scanned),
                    Some(8),
                ),
                (
                    TrajectorySource::Windsurf,
                    Some(ContributionStatus::Redacted),
                    Some(23),
                ),
                (TrajectorySource::Custom, None, None),
            ];

            let tile_w = 180.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (source, status, count)) in sources.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Source label
                let label_run = cx.text.layout(
                    source.label(),
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = TrajectorySourceBadge::new(*source);
                if let Some(s) = status {
                    badge = badge.status(*s);
                }
                if let Some(c) = count {
                    badge = badge.session_count(*c);
                }
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 170.0, 22.0), cx);
            }
        });
        y += traj_height + SECTION_GAP;

        // ========== Panel 5: Earnings Badges ==========
        let earn_height = panel_height(180.0);
        let earn_bounds = Bounds::new(bounds.origin.x, y, width, earn_height);
        draw_panel("Earnings Badges", earn_bounds, cx, |inner, cx| {
            let earnings = [
                (EarningsType::Total, 1_250_000),
                (EarningsType::Compute, 500_000),
                (EarningsType::Skills, 350_000),
                (EarningsType::Data, 250_000),
                (EarningsType::Trajectories, 150_000),
            ];

            // Row 1: Full earnings badges
            let mut x = inner.origin.x;
            for (earnings_type, amount) in &earnings {
                let mut badge = EarningsBadge::new(*earnings_type, *amount);
                badge.paint(Bounds::new(x, inner.origin.y, 160.0, 22.0), cx);
                x += 170.0;
                if x > inner.origin.x + inner.size.width - 100.0 {
                    break;
                }
            }

            // Row 2: Compact versions
            let row_y = inner.origin.y + 40.0;
            let compact_label = cx.text.layout(
                "Compact:",
                Point::new(inner.origin.x, row_y + 2.0),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(compact_label);

            let mut x = inner.origin.x + 60.0;
            for (earnings_type, amount) in &earnings {
                let mut badge = EarningsBadge::new(*earnings_type, *amount).compact(true);
                badge.paint(Bounds::new(x, row_y, 70.0, 22.0), cx);
                x += 80.0;
            }
        });
        y += earn_height + SECTION_GAP;

        // ========== Panel 6: Complete Marketplace Dashboard ==========
        let dashboard_height = panel_height(400.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel(
            "Marketplace Dashboard Preview",
            dashboard_bounds,
            cx,
            |inner, cx| {
                // Header bar
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width,
                        50.0,
                    ))
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::accent::PRIMARY, 1.0),
                );

                // Title
                let title = cx.text.layout(
                    "Unified Marketplace",
                    Point::new(inner.origin.x + 12.0, inner.origin.y + 8.0),
                    theme::font_size::BASE,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(title);

                // Market type tabs
                let mut x = inner.origin.x + 12.0;
                let tab_y = inner.origin.y + 28.0;
                for market_type in &[
                    MarketType::Compute,
                    MarketType::Skills,
                    MarketType::Data,
                    MarketType::Trajectories,
                ] {
                    let mut badge = MarketTypeBadge::new(*market_type);
                    badge.paint(Bounds::new(x, tab_y, 80.0, 20.0), cx);
                    x += 90.0;
                }

                // Earnings summary on right
                let mut total_earn =
                    EarningsBadge::new(EarningsType::Total, 1_250_000).compact(true);
                total_earn.paint(
                    Bounds::new(
                        inner.origin.x + inner.size.width - 90.0,
                        inner.origin.y + 14.0,
                        80.0,
                        22.0,
                    ),
                    cx,
                );

                // Provider row
                let prov_y = inner.origin.y + 62.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(inner.origin.x, prov_y, inner.size.width, 56.0))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                // Provider name + reputation
                let prov_name = cx.text.layout(
                    "compute-provider-1",
                    Point::new(inner.origin.x + 8.0, prov_y + 8.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(prov_name);

                let mut rep = ReputationBadge::new(TrustTier::Trusted).success_rate(0.97);
                rep.paint(
                    Bounds::new(inner.origin.x + 140.0, prov_y + 6.0, 100.0, 22.0),
                    cx,
                );

                // Job in progress
                let mut job = JobStatusBadge::new(JobStatus::Processing);
                job.paint(
                    Bounds::new(inner.origin.x + 8.0, prov_y + 32.0, 90.0, 22.0),
                    cx,
                );

                let job_info = cx.text.layout(
                    "llama3 • 1.2K tokens",
                    Point::new(inner.origin.x + 106.0, prov_y + 36.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(job_info);

                // Trajectory contribution section
                let traj_y = prov_y + 68.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(inner.origin.x, traj_y, inner.size.width, 100.0))
                        .with_background(theme::bg::SURFACE)
                        .with_border(theme::border::DEFAULT, 1.0),
                );

                let traj_title = cx.text.layout(
                    "Trajectory Contributions",
                    Point::new(inner.origin.x + 8.0, traj_y + 6.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(traj_title);

                // Source badges row
                let source_y = traj_y + 28.0;
                let mut x = inner.origin.x + 8.0;
                let sources = [
                    (TrajectorySource::Claude, ContributionStatus::Accepted, 42),
                    (TrajectorySource::Cursor, ContributionStatus::Pending, 15),
                ];
                for (source, status, count) in &sources {
                    let mut badge = TrajectorySourceBadge::new(*source)
                        .status(*status)
                        .session_count(*count);
                    badge.paint(Bounds::new(x, source_y, 170.0, 22.0), cx);
                    x += 180.0;
                }

                // Earnings row
                let earn_y = traj_y + 56.0;
                let earn_label = cx.text.layout(
                    "Trajectory earnings:",
                    Point::new(inner.origin.x + 8.0, earn_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(earn_label);

                let mut traj_earn = EarningsBadge::new(EarningsType::Trajectories, 150_000);
                traj_earn.paint(Bounds::new(inner.origin.x + 120.0, earn_y, 160.0, 22.0), cx);

                // Total earnings bar at bottom
                let total_y = traj_y + 80.0;
                let total_label = cx.text.layout(
                    "Total today:",
                    Point::new(inner.origin.x + 8.0, total_y + 4.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(total_label);

                let mut today_earn = EarningsBadge::new(EarningsType::Total, 25_000);
                today_earn.paint(Bounds::new(inner.origin.x + 80.0, total_y, 150.0, 22.0), cx);
            },
        );
    }

    fn paint_autopilot(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Session Status Badges ==========
        let session_height = panel_height(180.0);
        let session_bounds = Bounds::new(bounds.origin.x, y, width, session_height);
        draw_panel("Session Status Badges", session_bounds, cx, |inner, cx| {
            let statuses = [
                (SessionStatus::Pending, None, None, "Pending"),
                (SessionStatus::Running, Some(125), Some(8), "Running"),
                (SessionStatus::Paused, Some(340), Some(12), "Paused"),
                (SessionStatus::Completed, Some(1800), Some(45), "Completed"),
                (SessionStatus::Failed, Some(65), Some(3), "Failed"),
                (SessionStatus::Aborted, Some(200), Some(5), "Aborted"),
            ];

            let tile_w = 150.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (status, duration, tasks, label)) in statuses.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = SessionStatusBadge::new(*status);
                if let Some(secs) = duration {
                    badge = badge.duration(*secs);
                }
                if let Some(count) = tasks {
                    badge = badge.task_count(*count);
                }
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 140.0, 22.0), cx);
            }
        });
        y += session_height + SECTION_GAP;

        // ========== Panel 2: APM Gauges ==========
        let apm_height = panel_height(160.0);
        let apm_bounds = Bounds::new(bounds.origin.x, y, width, apm_height);
        draw_panel(
            "APM (Actions Per Minute) Gauges",
            apm_bounds,
            cx,
            |inner, cx| {
                let apms = [
                    (0.0, "Idle"),
                    (5.0, "Low"),
                    (22.0, "Normal"),
                    (45.0, "High"),
                    (80.0, "Intense"),
                ];

                let tile_w = 160.0;
                let tile_h = 55.0;
                let gap = 12.0;
                let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

                for (idx, (apm, label)) in apms.iter().enumerate() {
                    let row = idx / cols;
                    let col = idx % cols;
                    let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                    let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                    // Label
                    let label_run = cx.text.layout(
                        *label,
                        Point::new(tile_x, tile_y),
                        theme::font_size::XS,
                        theme::text::MUTED,
                    );
                    cx.scene.draw_text(label_run);

                    // Gauge
                    let mut gauge = ApmGauge::new(*apm);
                    gauge.paint(Bounds::new(tile_x, tile_y + 18.0, 150.0, 22.0), cx);
                }
            },
        );
        y += apm_height + SECTION_GAP;

        // ========== Panel 3: Resource Usage Bars ==========
        let resource_height = panel_height(180.0);
        let resource_bounds = Bounds::new(bounds.origin.x, y, width, resource_height);
        draw_panel("Resource Usage Bars", resource_bounds, cx, |inner, cx| {
            let resources = [
                (ResourceType::Memory, 35.0, "Normal Memory (35%)"),
                (ResourceType::Memory, 65.0, "Warning Memory (65%)"),
                (ResourceType::Memory, 92.0, "Critical Memory (92%)"),
                (ResourceType::Cpu, 28.0, "Normal CPU (28%)"),
                (ResourceType::Cpu, 75.0, "Warning CPU (75%)"),
                (ResourceType::Cpu, 95.0, "Critical CPU (95%)"),
            ];

            let tile_w = 200.0;
            let tile_h = 50.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (rtype, pct, label)) in resources.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Bar
                let mut bar = ResourceUsageBar::new(*rtype, *pct);
                bar.paint(Bounds::new(tile_x, tile_y + 18.0, 180.0, 22.0), cx);
            }
        });
        y += resource_height + SECTION_GAP;

        // ========== Panel 4: Daemon Status Badges ==========
        let daemon_height = panel_height(160.0);
        let daemon_bounds = Bounds::new(bounds.origin.x, y, width, daemon_height);
        draw_panel("Daemon Status Badges", daemon_bounds, cx, |inner, cx| {
            let statuses = [
                (DaemonStatus::Offline, None, None, "Offline"),
                (DaemonStatus::Starting, None, None, "Starting"),
                (
                    DaemonStatus::Online,
                    Some(86400),
                    Some(3),
                    "Online (1d, 3 workers)",
                ),
                (DaemonStatus::Restarting, None, None, "Restarting"),
                (DaemonStatus::Error, None, None, "Error"),
                (DaemonStatus::Stopping, None, None, "Stopping"),
            ];

            let tile_w = 170.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (status, uptime, workers, label)) in statuses.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = DaemonStatusBadge::new(*status);
                if let Some(secs) = uptime {
                    badge = badge.uptime(*secs);
                }
                if let Some(count) = workers {
                    badge = badge.worker_count(*count);
                }
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 160.0, 22.0), cx);
            }
        });
        y += daemon_height + SECTION_GAP;

        // ========== Panel 5: Parallel Agent Badges ==========
        let parallel_height = panel_height(180.0);
        let parallel_bounds = Bounds::new(bounds.origin.x, y, width, parallel_height);
        draw_panel("Parallel Agent Badges", parallel_bounds, cx, |inner, cx| {
            let agents = [
                (0, ParallelAgentStatus::Idle, None, "Agent 0: Idle"),
                (
                    1,
                    ParallelAgentStatus::Running,
                    Some("Building tests"),
                    "Agent 1: Running",
                ),
                (
                    2,
                    ParallelAgentStatus::Waiting,
                    Some("Awaiting input"),
                    "Agent 2: Waiting",
                ),
                (3, ParallelAgentStatus::Completed, None, "Agent 3: Done"),
                (
                    4,
                    ParallelAgentStatus::Failed,
                    Some("Build error"),
                    "Agent 4: Failed",
                ),
                (5, ParallelAgentStatus::Initializing, None, "Agent 5: Init"),
            ];

            let tile_w = 220.0;
            let tile_h = 55.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (agent_idx, status, task, label)) in agents.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // Badge
                let mut badge = ParallelAgentBadge::new(*agent_idx, *status);
                if let Some(t) = task {
                    badge = badge.current_task(*t);
                }
                badge.paint(Bounds::new(tile_x, tile_y + 18.0, 200.0, 22.0), cx);
            }
        });
        y += parallel_height + SECTION_GAP;

        // ========== Panel 6: Complete Autopilot Dashboard ==========
        let dashboard_height = panel_height(400.0);
        let dashboard_bounds = Bounds::new(bounds.origin.x, y, width, dashboard_height);
        draw_panel(
            "Autopilot Dashboard Preview",
            dashboard_bounds,
            cx,
            |inner, cx| {
                // Header bar
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width,
                        50.0,
                    ))
                    .with_background(theme::bg::ELEVATED)
                    .with_border(theme::accent::PRIMARY, 1.0),
                );

                // Title
                let title = cx.text.layout(
                    "Autopilot Control",
                    Point::new(inner.origin.x + 12.0, inner.origin.y + 8.0),
                    theme::font_size::BASE,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(title);

                // Daemon status on right
                let mut daemon = DaemonStatusBadge::new(DaemonStatus::Online)
                    .uptime(86400)
                    .worker_count(3);
                daemon.paint(
                    Bounds::new(
                        inner.origin.x + inner.size.width - 180.0,
                        inner.origin.y + 10.0,
                        170.0,
                        22.0,
                    ),
                    cx,
                );

                // APM gauge
                let mut apm = ApmGauge::new(28.5);
                apm.paint(
                    Bounds::new(inner.origin.x + 12.0, inner.origin.y + 32.0, 140.0, 22.0),
                    cx,
                );

                // Active session row
                let session_y = inner.origin.y + 62.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        session_y,
                        inner.size.width,
                        56.0,
                    ))
                    .with_background(theme::bg::SURFACE)
                    .with_border(theme::border::DEFAULT, 1.0),
                );

                // Session info
                let session_title = cx.text.layout(
                    "Active Session #1234",
                    Point::new(inner.origin.x + 8.0, session_y + 8.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(session_title);

                let mut session = SessionStatusBadge::new(SessionStatus::Running)
                    .duration(325)
                    .task_count(12);
                session.paint(
                    Bounds::new(inner.origin.x + 160.0, session_y + 6.0, 200.0, 22.0),
                    cx,
                );

                // Task info
                let task_info = cx.text.layout(
                    "Current: Building component tests",
                    Point::new(inner.origin.x + 8.0, session_y + 32.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(task_info);

                // Parallel agents section
                let agents_y = session_y + 68.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        agents_y,
                        inner.size.width,
                        100.0,
                    ))
                    .with_background(theme::bg::SURFACE)
                    .with_border(theme::border::DEFAULT, 1.0),
                );

                let agents_label = cx.text.layout(
                    "Parallel Agents",
                    Point::new(inner.origin.x + 8.0, agents_y + 8.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(agents_label);

                // Agent badges in a row
                let mut x = inner.origin.x + 8.0;
                for (idx, status) in [
                    ParallelAgentStatus::Running,
                    ParallelAgentStatus::Running,
                    ParallelAgentStatus::Waiting,
                ]
                .iter()
                .enumerate()
                {
                    let mut agent = ParallelAgentBadge::new(idx as u8, *status).compact(true);
                    agent.paint(Bounds::new(x, agents_y + 32.0, 50.0, 22.0), cx);
                    x += 60.0;
                }

                // Resource bars
                let res_y = agents_y + 60.0;
                let mut mem = ResourceUsageBar::new(ResourceType::Memory, 45.0).bar_width(80.0);
                mem.paint(Bounds::new(inner.origin.x + 8.0, res_y, 160.0, 22.0), cx);

                let mut cpu = ResourceUsageBar::new(ResourceType::Cpu, 62.0).bar_width(80.0);
                cpu.paint(Bounds::new(inner.origin.x + 180.0, res_y, 160.0, 22.0), cx);

                // Session history section
                let history_y = agents_y + 112.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        inner.origin.x,
                        history_y,
                        inner.size.width,
                        80.0,
                    ))
                    .with_background(theme::bg::SURFACE)
                    .with_border(theme::border::DEFAULT, 1.0),
                );

                let history_label = cx.text.layout(
                    "Recent Sessions",
                    Point::new(inner.origin.x + 8.0, history_y + 8.0),
                    theme::font_size::SM,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(history_label);

                // Completed sessions
                let mut completed1 = SessionStatusBadge::new(SessionStatus::Completed)
                    .duration(1800)
                    .task_count(45)
                    .compact(true);
                completed1.paint(
                    Bounds::new(inner.origin.x + 8.0, history_y + 32.0, 28.0, 22.0),
                    cx,
                );
                let c1_label = cx.text.layout(
                    "#1233 - 45 tasks",
                    Point::new(inner.origin.x + 42.0, history_y + 36.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(c1_label);

                let mut completed2 = SessionStatusBadge::new(SessionStatus::Failed).compact(true);
                completed2.paint(
                    Bounds::new(inner.origin.x + 8.0, history_y + 56.0, 28.0, 22.0),
                    cx,
                );
                let c2_label = cx.text.layout(
                    "#1232 - Build error",
                    Point::new(inner.origin.x + 42.0, history_y + 60.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(c2_label);
            },
        );
    }

    fn paint_thread_components(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let mut y = bounds.origin.y;
        let width = bounds.size.width;

        // ========== Panel 1: Thread Headers ==========
        let header_height = panel_height(160.0);
        let header_bounds = Bounds::new(bounds.origin.x, y, width, header_height);
        draw_panel("Thread Headers", header_bounds, cx, |inner, cx| {
            let variants = [
                ("Full header", true, true, Some("3 messages")),
                ("No back button", false, true, None),
                ("No menu button", true, false, None),
                ("Minimal", false, false, Some("subtitle only")),
            ];

            let tile_w = 280.0;
            let tile_h = 60.0;
            let gap = 16.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (label, show_back, show_menu, subtitle)) in variants.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // ThreadHeader
                let mut header = ThreadHeader::new("Conversation")
                    .show_back_button(*show_back)
                    .show_menu_button(*show_menu);
                if let Some(sub) = subtitle {
                    header = header.subtitle(*sub);
                }
                header.paint(Bounds::new(tile_x, tile_y + 14.0, tile_w, 48.0), cx);
            }
        });
        y += header_height + SECTION_GAP;

        // ========== Panel 2: Message Editor States ==========
        let editor_height = panel_height(180.0);
        let editor_bounds = Bounds::new(bounds.origin.x, y, width, editor_height);
        draw_panel("Message Editor States", editor_bounds, cx, |inner, cx| {
            let states = [
                ("Normal mode", Mode::Normal, false, "Type a message..."),
                ("Plan mode", Mode::Plan, false, "Describe your plan..."),
                ("Streaming", Mode::Normal, true, ""),
            ];

            let tile_w = 320.0;
            let tile_h = 70.0;
            let gap = 16.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (label, mode, streaming, placeholder)) in states.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // MessageEditor
                let mut editor = MessageEditor::new().mode(*mode).streaming(*streaming);
                if !placeholder.is_empty() {
                    editor = editor.placeholder(*placeholder);
                }
                editor.paint(Bounds::new(tile_x, tile_y + 14.0, tile_w, 64.0), cx);
            }
        });
        y += editor_height + SECTION_GAP;

        // ========== Panel 3: Thread Feedback ==========
        let feedback_height = panel_height(200.0);
        let feedback_bounds = Bounds::new(bounds.origin.x, y, width, feedback_height);
        draw_panel("Thread Feedback", feedback_bounds, cx, |inner, cx| {
            let tile_w = 280.0;
            let gap = 16.0;

            // Default state
            let label_run = cx.text.layout(
                "Default (no rating)",
                Point::new(inner.origin.x, inner.origin.y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label_run);

            let mut feedback1 = ThreadFeedback::new();
            feedback1.paint(
                Bounds::new(inner.origin.x, inner.origin.y + 14.0, tile_w, 80.0),
                cx,
            );

            // Second column - with comment shown (simulated by larger height)
            let label_run2 = cx.text.layout(
                "Rating selected",
                Point::new(inner.origin.x + tile_w + gap, inner.origin.y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(label_run2);

            // Show a description of what would happen
            let info = cx.text.layout(
                "Click thumbs up/down to rate",
                Point::new(inner.origin.x + tile_w + gap, inner.origin.y + 50.0),
                theme::font_size::XS,
                theme::text::DISABLED,
            );
            cx.scene.draw_text(info);
        });
        y += feedback_height + SECTION_GAP;

        // ========== Panel 4: Entry Actions ==========
        let actions_height = panel_height(140.0);
        let actions_bounds = Bounds::new(bounds.origin.x, y, width, actions_height);
        draw_panel("Entry Actions", actions_bounds, cx, |inner, cx| {
            let variants = [
                ("Default (feedback + copy)", true, true, false, false, false),
                ("With retry", true, true, true, false, false),
                ("With edit/delete", true, true, false, true, true),
                ("All actions", true, true, true, true, true),
                ("No feedback", false, true, true, false, false),
            ];

            let tile_w = 200.0;
            let tile_h = 45.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (label, feedback, copy, retry, edit, delete)) in variants.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // EntryActions
                let mut actions = EntryActions::new()
                    .show_feedback(*feedback)
                    .show_copy(*copy)
                    .show_retry(*retry)
                    .show_edit(*edit)
                    .show_delete(*delete);
                actions.paint(Bounds::new(tile_x, tile_y + 16.0, tile_w, 24.0), cx);
            }
        });
        y += actions_height + SECTION_GAP;

        // ========== Panel 5: Terminal Headers ==========
        let terminal_height = panel_height(140.0);
        let terminal_bounds = Bounds::new(bounds.origin.x, y, width, terminal_height);
        draw_panel("Terminal Headers", terminal_bounds, cx, |inner, cx| {
            let variants = [
                ("Pending", "cargo build", ToolStatus::Pending, None),
                ("Running", "npm install", ToolStatus::Running, None),
                ("Success", "cargo test", ToolStatus::Success, Some(0)),
                ("Error", "rm -rf /", ToolStatus::Error, Some(1)),
            ];

            let tile_w = 280.0;
            let tile_h = 45.0;
            let gap = 12.0;
            let cols = ((inner.size.width + gap) / (tile_w + gap)).floor().max(1.0) as usize;

            for (idx, (label, cmd, status, exit_code)) in variants.iter().enumerate() {
                let row = idx / cols;
                let col = idx % cols;
                let tile_x = inner.origin.x + col as f32 * (tile_w + gap);
                let tile_y = inner.origin.y + row as f32 * (tile_h + gap);

                // Label
                let label_run = cx.text.layout(
                    *label,
                    Point::new(tile_x, tile_y),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_run);

                // TerminalHeader
                let mut header = TerminalHeader::new(*cmd).status(*status);
                if let Some(code) = exit_code {
                    header = header.exit_code(*code);
                }
                header.paint(Bounds::new(tile_x, tile_y + 14.0, tile_w, 32.0), cx);
            }
        });
        y += terminal_height + SECTION_GAP;

        // ========== Panel 6: Complete Thread Layout ==========
        let layout_height = panel_height(400.0);
        let layout_bounds = Bounds::new(bounds.origin.x, y, width, layout_height);
        draw_panel("Complete Thread Layout", layout_bounds, cx, |inner, cx| {
            // ThreadHeader at top
            let mut header = ThreadHeader::new("Code Review Session")
                .subtitle("5 messages")
                .show_back_button(true)
                .show_menu_button(true);
            header.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 48.0),
                cx,
            );

            // Thread content area
            let content_y = inner.origin.y + 56.0;
            let content_h = inner.size.height - 56.0 - 72.0;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    inner.origin.x,
                    content_y,
                    inner.size.width,
                    content_h,
                ))
                .with_background(theme::bg::APP)
                .with_border(theme::border::DEFAULT, 1.0),
            );

            // Sample messages
            let msg1 = cx.text.layout(
                "User: Can you review this code?",
                Point::new(inner.origin.x + 12.0, content_y + 12.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(msg1);

            let msg2 = cx.text.layout(
                "Assistant: I'll analyze the code structure...",
                Point::new(inner.origin.x + 12.0, content_y + 36.0),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(msg2);

            // Entry actions for a message
            let mut actions = EntryActions::new()
                .show_feedback(true)
                .show_copy(true)
                .show_retry(true);
            actions.paint(
                Bounds::new(inner.origin.x + 12.0, content_y + 60.0, 180.0, 24.0),
                cx,
            );

            // Terminal header in content
            let mut terminal = TerminalHeader::new("cargo clippy")
                .status(ToolStatus::Success)
                .exit_code(0);
            terminal.paint(
                Bounds::new(inner.origin.x + 12.0, content_y + 92.0, 300.0, 32.0),
                cx,
            );

            // MessageEditor at bottom
            let editor_y = inner.origin.y + inner.size.height - 64.0;
            let mut editor = MessageEditor::new()
                .mode(Mode::Normal)
                .placeholder("Continue the conversation...");
            editor.paint(
                Bounds::new(inner.origin.x, editor_y, inner.size.width, 64.0),
                cx,
            );
        });
        y += layout_height + SECTION_GAP;

        // ========== Panel 7: Trajectory View ==========
        let trajectory_height = panel_height(220.0);
        let trajectory_bounds = Bounds::new(bounds.origin.x, y, width, trajectory_height);
        draw_panel("Trajectory View", trajectory_bounds, cx, |inner, cx| {
            let entries = vec![
                TrajectoryEntry::new("Load workspace")
                    .detail("Open repository state")
                    .timestamp("00:12")
                    .status(TrajectoryStatus::Verified),
                TrajectoryEntry::new("Analyze failing tests")
                    .detail("Unit tests: 3 failed")
                    .timestamp("00:32")
                    .status(TrajectoryStatus::Partial),
                TrajectoryEntry::new("Apply fix")
                    .detail("Update parser edge cases")
                    .timestamp("01:05")
                    .status(TrajectoryStatus::Verified),
                TrajectoryEntry::new("Re-run suite")
                    .detail("All green")
                    .timestamp("01:42")
                    .status(TrajectoryStatus::Verified),
            ];

            let mut view = TrajectoryView::new().entries(entries);
            view.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width,
                    inner.size.height,
                ),
                cx,
            );
        });
    }

    fn paint_sessions(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let width = bounds.size.width;
        let mut y = bounds.origin.y;

        // ========== Panel 1: Session Cards ==========
        let cards_height = panel_height(280.0);
        let cards_bounds = Bounds::new(bounds.origin.x, y, width, cards_height);
        draw_panel("Session Cards", cards_bounds, cx, |inner, cx| {
            let card_w = (inner.size.width - 24.0) / 3.0;

            // Running session
            let running_info = SessionInfo::new("sess-001", "Implement auth flow")
                .status(SessionStatus::Running)
                .timestamp("10:30 AM")
                .duration(1847)
                .task_count(12)
                .model("sonnet");
            let mut running = SessionCard::new(running_info);
            running.paint(
                Bounds::new(inner.origin.x, inner.origin.y, card_w, 160.0),
                cx,
            );

            // Completed session
            let completed_info = SessionInfo::new("sess-002", "Fix CI pipeline")
                .status(SessionStatus::Completed)
                .timestamp("Yesterday")
                .duration(3621)
                .task_count(8)
                .model("opus");
            let mut completed = SessionCard::new(completed_info);
            completed.paint(
                Bounds::new(
                    inner.origin.x + card_w + 12.0,
                    inner.origin.y,
                    card_w,
                    160.0,
                ),
                cx,
            );

            // Failed session
            let failed_info = SessionInfo::new("sess-003", "Migrate database")
                .status(SessionStatus::Failed)
                .timestamp("2 days ago")
                .duration(892)
                .task_count(5)
                .model("sonnet");
            let mut failed = SessionCard::new(failed_info);
            failed.paint(
                Bounds::new(
                    inner.origin.x + (card_w + 12.0) * 2.0,
                    inner.origin.y,
                    card_w,
                    160.0,
                ),
                cx,
            );

            // Second row - more states
            let row2_y = inner.origin.y + 172.0;

            let paused_info = SessionInfo::new("sess-004", "Refactor components")
                .status(SessionStatus::Paused)
                .timestamp("1 hour ago")
                .duration(1200)
                .task_count(15)
                .model("sonnet");
            let mut paused = SessionCard::new(paused_info);
            paused.paint(Bounds::new(inner.origin.x, row2_y, card_w, 160.0), cx);

            let aborted_info = SessionInfo::new("sess-005", "Update dependencies")
                .status(SessionStatus::Aborted)
                .timestamp("3 hours ago")
                .duration(456)
                .task_count(3)
                .model("haiku");
            let mut aborted = SessionCard::new(aborted_info);
            aborted.paint(
                Bounds::new(inner.origin.x + card_w + 12.0, row2_y, card_w, 160.0),
                cx,
            );

            let pending_info = SessionInfo::new("sess-006", "Write tests")
                .status(SessionStatus::Pending)
                .timestamp("Queued")
                .model("sonnet");
            let mut pending = SessionCard::new(pending_info);
            pending.paint(
                Bounds::new(
                    inner.origin.x + (card_w + 12.0) * 2.0,
                    row2_y,
                    card_w,
                    160.0,
                ),
                cx,
            );
        });
        y += cards_height + SECTION_GAP;

        // ========== Panel 2: Session Breadcrumbs ==========
        let breadcrumb_height = panel_height(120.0);
        let breadcrumb_bounds = Bounds::new(bounds.origin.x, y, width, breadcrumb_height);
        draw_panel("Session Breadcrumbs", breadcrumb_bounds, cx, |inner, cx| {
            // Simple breadcrumb
            let mut bc1 = SessionBreadcrumb::new().items(vec![
                BreadcrumbItem::new("sess-001", "Main Session"),
                BreadcrumbItem::new("sess-002", "Fork: Auth").current(true),
            ]);
            bc1.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 28.0),
                cx,
            );

            // Deep breadcrumb
            let mut bc2 = SessionBreadcrumb::new().items(vec![
                BreadcrumbItem::new("root", "Root Session"),
                BreadcrumbItem::new("fork-1", "API Changes"),
                BreadcrumbItem::new("fork-2", "Error Handling"),
                BreadcrumbItem::new("current", "Final Polish").current(true),
            ]);
            bc2.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 36.0,
                    inner.size.width,
                    28.0,
                ),
                cx,
            );

            // Single item
            let mut bc3 = SessionBreadcrumb::new()
                .push_item(BreadcrumbItem::new("standalone", "Standalone Session").current(true));
            bc3.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 72.0,
                    inner.size.width,
                    28.0,
                ),
                cx,
            );
        });
        y += breadcrumb_height + SECTION_GAP;

        // ========== Panel 3: Session Search ==========
        let search_height = panel_height(180.0);
        let search_bounds = Bounds::new(bounds.origin.x, y, width, search_height);
        draw_panel(
            "Session Search & Filters",
            search_bounds,
            cx,
            |inner, cx| {
                // Empty search bar
                let mut search1 = SessionSearchBar::new();
                search1.paint(
                    Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 44.0),
                    cx,
                );

                // Search bar with placeholder
                let mut search2 = SessionSearchBar::new().placeholder("Search auth sessions...");
                search2.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + 52.0,
                        inner.size.width,
                        44.0,
                    ),
                    cx,
                );

                // With active filter
                let mut search3 = SessionSearchBar::new();
                search3.set_filter(SessionStatus::Running, true);
                search3.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + 104.0,
                        inner.size.width,
                        44.0,
                    ),
                    cx,
                );
            },
        );
        y += search_height + SECTION_GAP;

        // ========== Panel 4: Session Actions ==========
        let actions_height = panel_height(160.0);
        let actions_bounds = Bounds::new(bounds.origin.x, y, width, actions_height);
        draw_panel("Session Actions", actions_bounds, cx, |inner, cx| {
            let label_x = inner.origin.x;
            let badge_x = inner.origin.x + 200.0;
            let mut row_y = inner.origin.y;

            // Resumable session (paused)
            let paused_label = cx.text.layout(
                "Paused → Can Resume:",
                Point::new(label_x, row_y + 4.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(paused_label);
            let mut paused_badge = SessionStatusBadge::new(SessionStatus::Paused);
            paused_badge.paint(Bounds::new(badge_x, row_y, 100.0, 24.0), cx);
            row_y += 32.0;

            // Forkable sessions
            let completed_label = cx.text.layout(
                "Completed → Can Fork:",
                Point::new(label_x, row_y + 4.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(completed_label);
            let mut completed_badge = SessionStatusBadge::new(SessionStatus::Completed);
            completed_badge.paint(Bounds::new(badge_x, row_y, 100.0, 24.0), cx);
            row_y += 32.0;

            let failed_label = cx.text.layout(
                "Failed → Can Fork:",
                Point::new(label_x, row_y + 4.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(failed_label);
            let mut failed_badge = SessionStatusBadge::new(SessionStatus::Failed);
            failed_badge.paint(Bounds::new(badge_x, row_y, 100.0, 24.0), cx);
            row_y += 32.0;

            // Active session
            let running_label = cx.text.layout(
                "Running → Active:",
                Point::new(label_x, row_y + 4.0),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(running_label);
            let mut running_badge = SessionStatusBadge::new(SessionStatus::Running)
                .duration(3621)
                .task_count(8);
            running_badge.paint(Bounds::new(badge_x, row_y, 200.0, 24.0), cx);
        });
        y += actions_height + SECTION_GAP;

        // ========== Panel 5: Complete Session List ==========
        let list_height = panel_height(320.0);
        let list_bounds = Bounds::new(bounds.origin.x, y, width, list_height);
        draw_panel("Complete Session List", list_bounds, cx, |inner, cx| {
            // Search bar at top
            let mut search = SessionSearchBar::new();
            search.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 44.0),
                cx,
            );

            // Breadcrumb showing current path
            let mut breadcrumb = SessionBreadcrumb::new().items(vec![
                BreadcrumbItem::new("all", "All Sessions"),
                BreadcrumbItem::new("today", "Today").current(true),
            ]);
            breadcrumb.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 52.0,
                    inner.size.width,
                    28.0,
                ),
                cx,
            );

            // Session cards in a grid
            let cards_y = inner.origin.y + 80.0;
            let card_w = (inner.size.width - 12.0) / 2.0;

            let sessions = [
                ("Current Task", SessionStatus::Running, 1847u64, 12u32),
                ("Yesterday's Work", SessionStatus::Completed, 7200, 15),
                ("Blocked Task", SessionStatus::Paused, 2400, 10),
                ("Failed Migration", SessionStatus::Failed, 600, 8),
            ];

            for (i, (title, status, dur, total)) in sessions.iter().enumerate() {
                let col = i % 2;
                let row = i / 2;
                let x = inner.origin.x + col as f32 * (card_w + 12.0);
                let y = cards_y + row as f32 * 112.0;

                let info = SessionInfo::new(format!("sess-{}", i), *title)
                    .status(*status)
                    .timestamp("Today")
                    .duration(*dur)
                    .task_count(*total)
                    .model("sonnet");
                let mut card = SessionCard::new(info);
                card.paint(Bounds::new(x, y, card_w, 100.0), cx);
            }
        });
    }

    fn paint_permissions(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let width = bounds.size.width;
        let mut y = bounds.origin.y;

        // ========== Panel 1: Permission Decisions ==========
        let decisions_height = panel_height(160.0);
        let decisions_bounds = Bounds::new(bounds.origin.x, y, width, decisions_height);
        draw_panel("Permission Decisions", decisions_bounds, cx, |inner, cx| {
            let decisions = [
                (PermissionDecision::Ask, "Ask every time"),
                (PermissionDecision::AllowOnce, "Allow once"),
                (PermissionDecision::AllowAlways, "Allow always"),
                (PermissionDecision::Deny, "Deny"),
            ];

            for (i, (decision, desc)) in decisions.iter().enumerate() {
                let x = inner.origin.x + (i as f32 * 140.0);

                // Decision badge
                let color = decision.color();
                let badge_bounds = Bounds::new(x, inner.origin.y, 120.0, 28.0);
                cx.scene.draw_quad(
                    Quad::new(badge_bounds)
                        .with_background(color.with_alpha(0.2))
                        .with_border(color, 1.0),
                );
                let label = cx.text.layout(
                    decision.label(),
                    Point::new(x + 8.0, inner.origin.y + 6.0),
                    theme::font_size::SM,
                    color,
                );
                cx.scene.draw_text(label);

                // Description
                let desc_text = cx.text.layout(
                    desc,
                    Point::new(x, inner.origin.y + 40.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(desc_text);
            }

            // Short labels row
            let short_y = inner.origin.y + 72.0;
            let short_label = cx.text.layout(
                "Short labels:",
                Point::new(inner.origin.x, short_y),
                theme::font_size::XS,
                theme::text::MUTED,
            );
            cx.scene.draw_text(short_label);

            for (i, (decision, _)) in decisions.iter().enumerate() {
                let x = inner.origin.x + 80.0 + (i as f32 * 60.0);
                let short = cx.text.layout(
                    decision.short_label(),
                    Point::new(x, short_y),
                    theme::font_size::SM,
                    decision.color(),
                );
                cx.scene.draw_text(short);
            }
        });
        y += decisions_height + SECTION_GAP;

        // ========== Panel 2: Permission Rules ==========
        let rules_height = panel_height(240.0);
        let rules_bounds = Bounds::new(bounds.origin.x, y, width, rules_height);
        draw_panel("Permission Rules", rules_bounds, cx, |inner, cx| {
            let rules = [
                PermissionRule::new("rule-1", ToolType::Bash, "Bash")
                    .scope(PermissionScope::Session)
                    .pattern("cargo *")
                    .decision(PermissionDecision::AllowAlways),
                PermissionRule::new("rule-2", ToolType::Write, "Write")
                    .scope(PermissionScope::Project)
                    .pattern("src/*")
                    .decision(PermissionDecision::AllowAlways),
                PermissionRule::new("rule-3", ToolType::Read, "Read")
                    .scope(PermissionScope::Global)
                    .decision(PermissionDecision::AllowAlways),
                PermissionRule::new("rule-4", ToolType::Edit, "Edit")
                    .scope(PermissionScope::Session)
                    .decision(PermissionDecision::Ask),
                PermissionRule::new("rule-5", ToolType::Bash, "Bash")
                    .scope(PermissionScope::Global)
                    .pattern("sudo *")
                    .decision(PermissionDecision::Deny),
            ];

            for (i, rule) in rules.iter().enumerate() {
                let mut row = PermissionRuleRow::new(rule.clone());
                row.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 44.0,
                        inner.size.width,
                        40.0,
                    ),
                    cx,
                );
            }
        });
        y += rules_height + SECTION_GAP;

        // ========== Panel 3: Permission History ==========
        let history_height = panel_height(280.0);
        let history_bounds = Bounds::new(bounds.origin.x, y, width, history_height);
        draw_panel("Permission History", history_bounds, cx, |inner, cx| {
            let histories = [
                PermissionHistory::new("h-1", ToolType::Bash, "Bash", "cargo build --release")
                    .decision(PermissionDecision::AllowOnce)
                    .timestamp("2 min ago")
                    .session("sess-001"),
                PermissionHistory::new("h-2", ToolType::Write, "Write", "src/lib.rs")
                    .decision(PermissionDecision::AllowAlways)
                    .timestamp("5 min ago")
                    .session("sess-001"),
                PermissionHistory::new("h-3", ToolType::Bash, "Bash", "rm -rf node_modules/")
                    .decision(PermissionDecision::Deny)
                    .timestamp("10 min ago")
                    .session("sess-001"),
                PermissionHistory::new("h-4", ToolType::Read, "Read", "/etc/passwd")
                    .decision(PermissionDecision::Deny)
                    .timestamp("15 min ago")
                    .session("sess-002"),
            ];

            for (i, history) in histories.iter().enumerate() {
                let mut item = PermissionHistoryItem::new(history.clone());
                item.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 64.0,
                        inner.size.width,
                        56.0,
                    ),
                    cx,
                );
            }
        });
        y += history_height + SECTION_GAP;

        // ========== Panel 4: Permission Bar Variants ==========
        let bar_height = panel_height(200.0);
        let bar_bounds = Bounds::new(bounds.origin.x, y, width, bar_height);
        draw_panel("Permission Bar Variants", bar_bounds, cx, |inner, cx| {
            // Standard permission bar
            let mut bar1 = PermissionBar::new("Bash wants to execute: cargo test");
            bar1.paint(
                Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, 48.0),
                cx,
            );

            // File write permission
            let mut bar2 = PermissionBar::new("Write wants to create: src/new_module.rs");
            bar2.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 56.0,
                    inner.size.width,
                    48.0,
                ),
                cx,
            );

            // Dangerous operation
            let mut bar3 = PermissionBar::new("Bash wants to execute: git push --force");
            bar3.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 112.0,
                    inner.size.width,
                    48.0,
                ),
                cx,
            );
        });
        y += bar_height + SECTION_GAP;

        // ========== Panel 5: Permission Statistics ==========
        let stats_height = panel_height(140.0);
        let stats_bounds = Bounds::new(bounds.origin.x, y, width, stats_height);
        draw_panel("Permission Statistics", stats_bounds, cx, |inner, cx| {
            let stats = [
                ("Total Requests", "247", theme::text::PRIMARY),
                ("Allowed", "189", Hsla::new(120.0, 0.7, 0.45, 1.0)),
                ("Denied", "42", Hsla::new(0.0, 0.8, 0.5, 1.0)),
                ("Pending", "16", Hsla::new(45.0, 0.7, 0.5, 1.0)),
            ];

            let stat_w = inner.size.width / 4.0;
            for (i, (label, value, color)) in stats.iter().enumerate() {
                let x = inner.origin.x + i as f32 * stat_w;

                // Value (large)
                let value_text = cx.text.layout(
                    value,
                    Point::new(x + 12.0, inner.origin.y + 8.0),
                    24.0,
                    *color,
                );
                cx.scene.draw_text(value_text);

                // Label
                let label_text = cx.text.layout(
                    label,
                    Point::new(x + 12.0, inner.origin.y + 44.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_text);
            }

            // Rule counts
            let rule_y = inner.origin.y + 80.0;
            let rule_label = cx.text.layout(
                "Active Rules:",
                Point::new(inner.origin.x + 12.0, rule_y),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(rule_label);

            let rule_counts = [("Global", 5), ("Project", 12), ("Session", 8)];

            let mut rx = inner.origin.x + 120.0;
            for (scope, count) in rule_counts {
                let scope_text = format!("{}: {}", scope, count);
                let scope_run = cx.text.layout(
                    &scope_text,
                    Point::new(rx, rule_y),
                    theme::font_size::SM,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(scope_run);
                rx += 100.0;
            }
        });
    }

    fn paint_apm_metrics(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let width = bounds.size.width;
        let mut y = bounds.origin.y;

        // ========== Panel 1: APM Gauge Variations ==========
        let gauge_height = panel_height(200.0);
        let gauge_bounds = Bounds::new(bounds.origin.x, y, width, gauge_height);
        draw_panel("APM Gauge Variations", gauge_bounds, cx, |inner, cx| {
            let apms = [
                (0.0, "Idle"),
                (25.0, "Slow"),
                (50.0, "Moderate"),
                (75.0, "Fast"),
                (95.0, "Expert"),
                (120.0, "Elite"),
            ];

            let gauge_w = 100.0;
            let gauge_h = 60.0;
            let gap = 20.0;

            for (i, (apm, label)) in apms.iter().enumerate() {
                let x = inner.origin.x + (i as f32 * (gauge_w + gap));

                // APM Gauge
                let mut gauge = ApmGauge::new(*apm);
                gauge.paint(Bounds::new(x, inner.origin.y, gauge_w, gauge_h), cx);

                // Label
                let label_text = cx.text.layout(
                    *label,
                    Point::new(x + 10.0, inner.origin.y + gauge_h + 8.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_text);

                // APM Value
                let level = ApmLevel::from_apm(*apm);
                let apm_text = cx.text.layout(
                    &format!("{:.0} APM", apm),
                    Point::new(x + 10.0, inner.origin.y + gauge_h + 24.0),
                    theme::font_size::SM,
                    level.color(),
                );
                cx.scene.draw_text(apm_text);

                // Tier label
                let tier_text = cx.text.layout(
                    level.label(),
                    Point::new(x + 10.0, inner.origin.y + gauge_h + 42.0),
                    theme::font_size::XS,
                    level.color(),
                );
                cx.scene.draw_text(tier_text);
            }
        });
        y += gauge_height + SECTION_GAP;

        // ========== Panel 2: APM Session Rows ==========
        let rows_height = panel_height(220.0);
        let rows_bounds = Bounds::new(bounds.origin.x, y, width, rows_height);
        draw_panel("APM Session Rows", rows_bounds, cx, |inner, cx| {
            let sessions = [
                ApmSessionData::new("sess-1", "Build feature authentication", 92.0)
                    .status(SessionStatus::Completed)
                    .duration(1800)
                    .rank(1),
                ApmSessionData::new("sess-2", "Fix database query bug", 78.5)
                    .status(SessionStatus::Completed)
                    .duration(2400)
                    .rank(2),
                ApmSessionData::new("sess-3", "Refactor API endpoints", 65.0)
                    .status(SessionStatus::Running)
                    .duration(900)
                    .rank(3),
                ApmSessionData::new("sess-4", "Add unit tests", 45.2)
                    .status(SessionStatus::Paused)
                    .duration(600)
                    .rank(4),
            ];

            for (i, session) in sessions.iter().enumerate() {
                let mut row = ApmSessionRow::new(session.clone());
                row.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 48.0,
                        inner.size.width,
                        44.0,
                    ),
                    cx,
                );
            }
        });
        y += rows_height + SECTION_GAP;

        // ========== Panel 3: Session Comparison ==========
        let comparison_height = panel_height(280.0);
        let comparison_bounds = Bounds::new(bounds.origin.x, y, width, comparison_height);
        draw_panel("Session Comparison", comparison_bounds, cx, |inner, cx| {
            let session_a = ComparisonSession::new("sess-a", "Monday Session", 68.5)
                .messages(120)
                .tool_calls(85)
                .duration(3600);

            let session_b = ComparisonSession::new("sess-b", "Tuesday Session", 82.3)
                .messages(95)
                .tool_calls(110)
                .duration(2800);

            let mut card = ApmComparisonCard::new(session_a, session_b);
            card.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(500.0),
                    220.0,
                ),
                cx,
            );
        });
        y += comparison_height + SECTION_GAP;

        // ========== Panel 4: APM Leaderboard ==========
        let leaderboard_height = panel_height(320.0);
        let leaderboard_bounds = Bounds::new(bounds.origin.x, y, width, leaderboard_height);
        draw_panel("APM Leaderboard", leaderboard_bounds, cx, |inner, cx| {
            let entries = vec![
                LeaderboardEntry::new("1", "Implement OAuth2 flow", 98.5)
                    .status(SessionStatus::Completed)
                    .messages(150)
                    .tool_calls(120),
                LeaderboardEntry::new("2", "Build payment integration", 92.0)
                    .status(SessionStatus::Completed)
                    .messages(180)
                    .tool_calls(95),
                LeaderboardEntry::new("3", "Create dashboard UI", 85.5)
                    .status(SessionStatus::Completed)
                    .messages(200)
                    .tool_calls(75),
                LeaderboardEntry::new("4", "Add real-time sync", 78.0)
                    .status(SessionStatus::Completed)
                    .messages(90)
                    .tool_calls(60),
                LeaderboardEntry::new("5", "Fix memory leak", 65.0)
                    .status(SessionStatus::Completed)
                    .messages(50)
                    .tool_calls(35),
                LeaderboardEntry::new("6", "Write documentation", 45.0)
                    .status(SessionStatus::Completed)
                    .messages(80)
                    .tool_calls(10),
            ];

            let mut leaderboard = ApmLeaderboard::new()
                .title("Top Sessions This Week")
                .entries(entries);
            leaderboard.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(600.0),
                    260.0,
                ),
                cx,
            );
        });
        y += leaderboard_height + SECTION_GAP;

        // ========== Panel 5: APM Trends Summary ==========
        let trends_height = panel_height(200.0);
        let trends_bounds = Bounds::new(bounds.origin.x, y, width, trends_height);
        draw_panel("APM Trends Summary", trends_bounds, cx, |inner, cx| {
            let metrics = [
                ("Avg APM", "72.4", "+5.2%", Hsla::new(120.0, 0.7, 0.45, 1.0)),
                (
                    "Peak APM",
                    "98.5",
                    "+12.1%",
                    Hsla::new(120.0, 0.7, 0.45, 1.0),
                ),
                ("Sessions", "24", "+3", Hsla::new(200.0, 0.7, 0.5, 1.0)),
                (
                    "Tool Calls",
                    "1,847",
                    "-2.3%",
                    Hsla::new(0.0, 0.7, 0.5, 1.0),
                ),
            ];

            let metric_w = inner.size.width / 4.0;
            for (i, (label, value, change, change_color)) in metrics.iter().enumerate() {
                let x = inner.origin.x + i as f32 * metric_w;

                // Value (large)
                let value_text = cx.text.layout(
                    value,
                    Point::new(x + 12.0, inner.origin.y + 8.0),
                    24.0,
                    theme::text::PRIMARY,
                );
                cx.scene.draw_text(value_text);

                // Change indicator
                let change_text = cx.text.layout(
                    change,
                    Point::new(x + 12.0, inner.origin.y + 40.0),
                    theme::font_size::SM,
                    *change_color,
                );
                cx.scene.draw_text(change_text);

                // Label
                let label_text = cx.text.layout(
                    label,
                    Point::new(x + 12.0, inner.origin.y + 60.0),
                    theme::font_size::XS,
                    theme::text::MUTED,
                );
                cx.scene.draw_text(label_text);
            }

            // Period selector hint
            let period_y = inner.origin.y + 100.0;
            let periods = ["1h", "24h", "7d", "30d"];
            let period_label = cx.text.layout(
                "Time Period:",
                Point::new(inner.origin.x + 12.0, period_y),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(period_label);

            for (i, period) in periods.iter().enumerate() {
                let x = inner.origin.x + 100.0 + i as f32 * 60.0;
                let is_selected = i == 2; // 7d selected

                let bg = if is_selected {
                    theme::accent::PRIMARY.with_alpha(0.3)
                } else {
                    theme::bg::HOVER
                };

                let btn_bounds = Bounds::new(x, period_y - 4.0, 48.0, 24.0);
                cx.scene
                    .draw_quad(Quad::new(btn_bounds).with_background(bg).with_border(
                        if is_selected {
                            theme::accent::PRIMARY
                        } else {
                            theme::border::DEFAULT
                        },
                        1.0,
                    ));

                let period_text = cx.text.layout(
                    period,
                    Point::new(x + 14.0, period_y),
                    theme::font_size::SM,
                    if is_selected {
                        theme::accent::PRIMARY
                    } else {
                        theme::text::MUTED
                    },
                );
                cx.scene.draw_text(period_text);
            }
        });
    }

    fn paint_wallet_flows(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let width = bounds.size.width;
        let mut y = bounds.origin.y;

        // ========== Panel 1: Mnemonic Display ==========
        let mnemonic_height = panel_height(260.0);
        let mnemonic_bounds = Bounds::new(bounds.origin.x, y, width, mnemonic_height);
        draw_panel("Mnemonic Display", mnemonic_bounds, cx, |inner, cx| {
            // Sample 12-word mnemonic
            let words = vec![
                "abandon".to_string(),
                "ability".to_string(),
                "able".to_string(),
                "about".to_string(),
                "above".to_string(),
                "absent".to_string(),
                "absorb".to_string(),
                "abstract".to_string(),
                "absurd".to_string(),
                "abuse".to_string(),
                "access".to_string(),
                "accident".to_string(),
            ];

            let mut mnemonic = MnemonicDisplay::new(words).revealed(true);
            mnemonic.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(500.0),
                    200.0,
                ),
                cx,
            );
        });
        y += mnemonic_height + SECTION_GAP;

        // ========== Panel 2: Address Cards ==========
        let address_height = panel_height(180.0);
        let address_bounds = Bounds::new(bounds.origin.x, y, width, address_height);
        draw_panel("Address Cards", address_bounds, cx, |inner, cx| {
            // Bitcoin address
            let mut btc_card = AddressCard::new(
                "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq",
                AddressType::Bitcoin,
            )
            .label("Primary Wallet");
            btc_card.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(400.0),
                    70.0,
                ),
                cx,
            );

            // Lightning address
            let mut ln_card =
                AddressCard::new("lnbc1500n1pj9nr6mpp5argz38...", AddressType::Lightning)
                    .label("Lightning Invoice");
            ln_card.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y + 80.0,
                    inner.size.width.min(400.0),
                    70.0,
                ),
                cx,
            );
        });
        y += address_height + SECTION_GAP;

        // ========== Panel 3: Transaction History ==========
        let tx_height = panel_height(280.0);
        let tx_bounds = Bounds::new(bounds.origin.x, y, width, tx_height);
        draw_panel("Transaction History", tx_bounds, cx, |inner, cx| {
            let transactions = [
                TransactionInfo::new("tx-1", 150000, TransactionDirection::Incoming)
                    .timestamp("2 hours ago")
                    .description("Payment from Alice"),
                TransactionInfo::new("tx-2", 50000, TransactionDirection::Outgoing)
                    .timestamp("Yesterday")
                    .description("Coffee shop")
                    .fee(500),
                TransactionInfo::new("tx-3", 1000000, TransactionDirection::Incoming)
                    .timestamp("3 days ago")
                    .description("Freelance payment"),
                TransactionInfo::new("tx-4", 25000, TransactionDirection::Outgoing)
                    .timestamp("1 week ago")
                    .description("Subscription")
                    .fee(250),
            ];

            for (i, tx) in transactions.iter().enumerate() {
                let mut row = TransactionRow::new(tx.clone());
                row.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 60.0,
                        inner.size.width,
                        56.0,
                    ),
                    cx,
                );
            }
        });
        y += tx_height + SECTION_GAP;

        // ========== Panel 4: Send Flow ==========
        let send_height = panel_height(360.0);
        let send_bounds = Bounds::new(bounds.origin.x, y, width, send_height);
        draw_panel("Send Flow Wizard", send_bounds, cx, |inner, cx| {
            let mut send_flow = SendFlow::new()
                .step(SendStep::Review)
                .address("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq")
                .amount(50000)
                .fee(500);
            send_flow.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(500.0),
                    320.0,
                ),
                cx,
            );
        });
        y += send_height + SECTION_GAP;

        // ========== Panel 5: Receive Flow ==========
        let receive_height = panel_height(420.0);
        let receive_bounds = Bounds::new(bounds.origin.x, y, width, receive_height);
        draw_panel("Receive Flow Wizard", receive_bounds, cx, |inner, cx| {
            let mut receive_flow = ReceiveFlow::new()
                .step(ReceiveStep::ShowInvoice)
                .receive_type(ReceiveType::Lightning)
                .amount(25000)
                .invoice("lnbc250u1pjxxx...")
                .expires_in(3600);
            receive_flow.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(500.0),
                    380.0,
                ),
                cx,
            );
        });
    }

    fn paint_gitafter_flows(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let width = bounds.size.width;
        let mut y = bounds.origin.y;

        // ========== Panel 1: Repository Cards ==========
        let repo_height = panel_height(240.0);
        let repo_bounds = Bounds::new(bounds.origin.x, y, width, repo_height);
        draw_panel("Repository Cards", repo_bounds, cx, |inner, cx| {
            let repos = [
                RepoInfo::new("repo-1", "openagents")
                    .description("An open source AI agent framework for autonomous workflows")
                    .visibility(RepoVisibility::Public)
                    .stars(1250)
                    .forks(180)
                    .issues(42)
                    .language("Rust")
                    .updated_at("2 hours ago"),
                RepoInfo::new("repo-2", "wgpui")
                    .description("GPU-accelerated native UI framework")
                    .visibility(RepoVisibility::Public)
                    .stars(340)
                    .forks(28)
                    .issues(15)
                    .language("Rust")
                    .updated_at("Yesterday"),
            ];

            for (i, repo) in repos.iter().enumerate() {
                let mut card = RepoCard::new(repo.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 110.0,
                        inner.size.width.min(500.0),
                        100.0,
                    ),
                    cx,
                );
            }
        });
        y += repo_height + SECTION_GAP;

        // ========== Panel 2: Issue List ==========
        let issue_height = panel_height(320.0);
        let issue_bounds = Bounds::new(bounds.origin.x, y, width, issue_height);
        draw_panel("Issue List with Bounties", issue_bounds, cx, |inner, cx| {
            let issues = [
                IssueInfo::new("issue-1", 42, "Memory leak in event processing loop")
                    .status(IssueStatus::Open)
                    .label(IssueLabel::bug())
                    .label(IssueLabel::help_wanted())
                    .author("alice")
                    .bounty(50000)
                    .comments(12)
                    .created_at("3 days ago"),
                IssueInfo::new("issue-2", 43, "Add dark mode toggle to settings")
                    .status(IssueStatus::Open)
                    .label(IssueLabel::enhancement())
                    .label(IssueLabel::good_first_issue())
                    .author("bob")
                    .bounty(25000)
                    .comments(5)
                    .created_at("1 week ago"),
                IssueInfo::new("issue-3", 44, "Update documentation for v2.0 release")
                    .status(IssueStatus::Closed)
                    .label(IssueLabel::new("docs", Hsla::new(190.0, 0.6, 0.5, 1.0)))
                    .author("charlie")
                    .comments(8)
                    .created_at("2 weeks ago"),
            ];

            for (i, issue) in issues.iter().enumerate() {
                let mut row = IssueRow::new(issue.clone());
                row.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 90.0,
                        inner.size.width,
                        80.0,
                    ),
                    cx,
                );
            }
        });
        y += issue_height + SECTION_GAP;

        // ========== Panel 3: PR Timeline ==========
        let pr_height = panel_height(280.0);
        let pr_bounds = Bounds::new(bounds.origin.x, y, width, pr_height);
        draw_panel("PR Timeline", pr_bounds, cx, |inner, cx| {
            let events = [
                PrEvent::new("ev-1", PrEventType::Commit, "alice")
                    .message("Initial implementation of feature X")
                    .commit_sha("abc1234def")
                    .timestamp("3 hours ago"),
                PrEvent::new("ev-2", PrEventType::Review, "bob")
                    .review_state(ReviewState::Approved)
                    .timestamp("2 hours ago"),
                PrEvent::new("ev-3", PrEventType::Comment, "charlie")
                    .message("Looks good! Just one minor suggestion...")
                    .timestamp("1 hour ago"),
                PrEvent::new("ev-4", PrEventType::Merge, "alice")
                    .message("Merged into main")
                    .timestamp("30 minutes ago"),
            ];

            for (i, event) in events.iter().enumerate() {
                let is_last = i == events.len() - 1;
                let mut item = PrTimelineItem::new(event.clone()).is_last(is_last);
                item.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 60.0,
                        inner.size.width.min(500.0),
                        60.0,
                    ),
                    cx,
                );
            }
        });
        y += pr_height + SECTION_GAP;

        // ========== Panel 4: Issue Labels & Status Variants ==========
        let labels_height = panel_height(200.0);
        let labels_bounds = Bounds::new(bounds.origin.x, y, width, labels_height);
        draw_panel(
            "Issue Labels & PR Events",
            labels_bounds,
            cx,
            |inner, cx| {
                // Draw predefined labels
                let labels = [
                    IssueLabel::bug(),
                    IssueLabel::enhancement(),
                    IssueLabel::good_first_issue(),
                    IssueLabel::help_wanted(),
                    IssueLabel::new("security", Hsla::new(0.0, 0.8, 0.5, 1.0)),
                    IssueLabel::new("performance", Hsla::new(280.0, 0.6, 0.5, 1.0)),
                ];

                let mut label_x = inner.origin.x;
                for label in &labels {
                    let label_w = (label.name.len() as f32 * 7.0) + 16.0;
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(label_x, inner.origin.y, label_w, 20.0))
                            .with_background(label.color.with_alpha(0.2))
                            .with_border(label.color, 1.0),
                    );
                    let text = cx.text.layout(
                        &label.name,
                        Point::new(label_x + 6.0, inner.origin.y + 4.0),
                        theme::font_size::XS,
                        label.color,
                    );
                    cx.scene.draw_text(text);
                    label_x += label_w + 8.0;
                }

                // Draw PR event types
                let mut event_y = inner.origin.y + 40.0;
                let events = [
                    PrEventType::Commit,
                    PrEventType::Review,
                    PrEventType::Comment,
                    PrEventType::StatusChange,
                    PrEventType::Merge,
                    PrEventType::Close,
                    PrEventType::Reopen,
                ];

                let mut event_x = inner.origin.x;
                for event in &events {
                    let icon_text = format!("{} {}", event.icon(), event.label());
                    let text = cx.text.layout(
                        &icon_text,
                        Point::new(event_x, event_y),
                        theme::font_size::SM,
                        event.color(),
                    );
                    cx.scene.draw_text(text);
                    event_x += 120.0;
                    if event_x > inner.origin.x + inner.size.width - 120.0 {
                        event_x = inner.origin.x;
                        event_y += 24.0;
                    }
                }

                // Draw review states
                let review_y = event_y + 40.0;
                let states = [
                    ReviewState::Approved,
                    ReviewState::RequestChanges,
                    ReviewState::Commented,
                    ReviewState::Pending,
                ];

                let mut state_x = inner.origin.x;
                for state in &states {
                    let state_w = 120.0;
                    cx.scene.draw_quad(
                        Quad::new(Bounds::new(state_x, review_y, state_w, 24.0))
                            .with_background(state.color().with_alpha(0.2))
                            .with_border(state.color(), 1.0),
                    );
                    let text = cx.text.layout(
                        state.label(),
                        Point::new(state_x + 8.0, review_y + 5.0),
                        theme::font_size::XS,
                        state.color(),
                    );
                    cx.scene.draw_text(text);
                    state_x += state_w + 12.0;
                }
            },
        );
    }

    fn paint_marketplace_flows(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let width = bounds.size.width;
        let mut y = bounds.origin.y;

        // ========== Panel 1: Compute Providers ==========
        let provider_height = panel_height(260.0);
        let provider_bounds = Bounds::new(bounds.origin.x, y, width, provider_height);
        draw_panel("Compute Providers", provider_bounds, cx, |inner, cx| {
            let providers = [
                ProviderInfo::new(
                    "p1",
                    "FastCompute Pro",
                    ProviderSpecs::new(32, 128, 2000).gpu("NVIDIA A100"),
                )
                .status(ProviderStatus::Online)
                .price(15000)
                .rating(4.9)
                .jobs(1250)
                .location("US-East"),
                ProviderInfo::new("p2", "Budget Runner", ProviderSpecs::new(8, 32, 500))
                    .status(ProviderStatus::Busy)
                    .price(2000)
                    .rating(4.5)
                    .jobs(340)
                    .location("EU-West"),
            ];

            for (i, provider) in providers.iter().enumerate() {
                let mut card = ProviderCard::new(provider.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 115.0,
                        inner.size.width.min(500.0),
                        110.0,
                    ),
                    cx,
                );
            }
        });
        y += provider_height + SECTION_GAP;

        // ========== Panel 2: Skills Marketplace ==========
        let skills_height = panel_height(280.0);
        let skills_bounds = Bounds::new(bounds.origin.x, y, width, skills_height);
        draw_panel("Skills Marketplace", skills_bounds, cx, |inner, cx| {
            let skills = [
                SkillInfo::new(
                    "s1",
                    "Code Review Pro",
                    "AI-powered code review with security analysis",
                )
                .category(SkillCategory::CodeGeneration)
                .author("openagents")
                .version("2.1.0")
                .status(SkillInstallStatus::Installed)
                .downloads(45000)
                .rating(4.8),
                SkillInfo::new(
                    "s2",
                    "Data Transformer",
                    "Transform and clean datasets automatically",
                )
                .category(SkillCategory::DataAnalysis)
                .author("datacraft")
                .version("1.5.2")
                .status(SkillInstallStatus::Available)
                .price(5000)
                .downloads(12000)
                .rating(4.6),
            ];

            for (i, skill) in skills.iter().enumerate() {
                let mut card = SkillCard::new(skill.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 120.0,
                        inner.size.width.min(500.0),
                        110.0,
                    ),
                    cx,
                );
            }
        });
        y += skills_height + SECTION_GAP;

        // ========== Panel 3: Data Marketplace ==========
        let data_height = panel_height(280.0);
        let data_bounds = Bounds::new(bounds.origin.x, y, width, data_height);
        draw_panel("Data Marketplace", data_bounds, cx, |inner, cx| {
            let datasets = [
                DatasetInfo::new(
                    "d1",
                    "LLM Training Corpus",
                    "High-quality text corpus for language model training",
                )
                .format(DataFormat::Parquet)
                .license(DataLicense::OpenSource)
                .size(10_737_418_240) // 10 GB
                .rows(50_000_000)
                .author("opendata")
                .downloads(2500)
                .updated_at("2 days ago"),
                DatasetInfo::new(
                    "d2",
                    "Code Embeddings",
                    "Pre-computed embeddings for 100+ programming languages",
                )
                .format(DataFormat::Arrow)
                .license(DataLicense::Commercial)
                .size(5_368_709_120) // 5 GB
                .rows(25_000_000)
                .author("codebase")
                .price(25000)
                .downloads(850)
                .updated_at("1 week ago"),
            ];

            for (i, dataset) in datasets.iter().enumerate() {
                let mut card = DatasetCard::new(dataset.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 115.0,
                        inner.size.width.min(550.0),
                        105.0,
                    ),
                    cx,
                );
            }
        });
        y += data_height + SECTION_GAP;

        // ========== Panel 4: Categories & Formats Reference ==========
        let ref_height = panel_height(180.0);
        let ref_bounds = Bounds::new(bounds.origin.x, y, width, ref_height);
        draw_panel("Categories & Formats", ref_bounds, cx, |inner, cx| {
            // Skill categories
            let mut cat_x = inner.origin.x;
            let categories = [
                SkillCategory::CodeGeneration,
                SkillCategory::DataAnalysis,
                SkillCategory::WebAutomation,
                SkillCategory::FileProcessing,
                SkillCategory::ApiIntegration,
            ];

            for cat in &categories {
                let cat_w = (cat.label().len() as f32 * 6.0) + 12.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(cat_x, inner.origin.y, cat_w, 18.0))
                        .with_background(cat.color().with_alpha(0.2))
                        .with_border(cat.color(), 1.0),
                );
                let text = cx.text.layout(
                    cat.label(),
                    Point::new(cat_x + 4.0, inner.origin.y + 3.0),
                    theme::font_size::XS,
                    cat.color(),
                );
                cx.scene.draw_text(text);
                cat_x += cat_w + 8.0;
            }

            // Data formats
            let mut fmt_x = inner.origin.x;
            let formats = [
                DataFormat::Json,
                DataFormat::Csv,
                DataFormat::Parquet,
                DataFormat::Arrow,
                DataFormat::Sqlite,
            ];

            for fmt in &formats {
                let fmt_w = 60.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(fmt_x, inner.origin.y + 30.0, fmt_w, 18.0))
                        .with_background(fmt.color().with_alpha(0.2))
                        .with_border(fmt.color(), 1.0),
                );
                let text = cx.text.layout(
                    fmt.label(),
                    Point::new(fmt_x + 6.0, inner.origin.y + 33.0),
                    theme::font_size::XS,
                    fmt.color(),
                );
                cx.scene.draw_text(text);
                fmt_x += fmt_w + 8.0;
            }

            // Provider statuses
            let mut status_x = inner.origin.x;
            let statuses = [
                ProviderStatus::Online,
                ProviderStatus::Busy,
                ProviderStatus::Offline,
                ProviderStatus::Maintenance,
            ];

            for status in &statuses {
                let status_w = 90.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(status_x, inner.origin.y + 60.0, status_w, 18.0))
                        .with_background(status.color().with_alpha(0.2))
                        .with_border(status.color(), 1.0),
                );
                let text = cx.text.layout(
                    status.label(),
                    Point::new(status_x + 6.0, inner.origin.y + 63.0),
                    theme::font_size::XS,
                    status.color(),
                );
                cx.scene.draw_text(text);
                status_x += status_w + 8.0;
            }

            // Install statuses
            let mut install_x = inner.origin.x;
            let install_statuses = [
                SkillInstallStatus::Available,
                SkillInstallStatus::Installed,
                SkillInstallStatus::UpdateAvailable,
                SkillInstallStatus::Installing,
            ];

            for status in &install_statuses {
                let status_w = 90.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        install_x,
                        inner.origin.y + 90.0,
                        status_w,
                        18.0,
                    ))
                    .with_background(status.color().with_alpha(0.2))
                    .with_border(status.color(), 1.0),
                );
                let text = cx.text.layout(
                    status.label(),
                    Point::new(install_x + 6.0, inner.origin.y + 93.0),
                    theme::font_size::XS,
                    status.color(),
                );
                cx.scene.draw_text(text);
                install_x += status_w + 8.0;
            }
        });
    }

    fn paint_nostr_flows(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let width = bounds.size.width;
        let mut y = bounds.origin.y;

        // ========== Panel 1: Contact Cards ==========
        let contacts_height = panel_height(320.0);
        let contacts_bounds = Bounds::new(bounds.origin.x, y, width, contacts_height);
        draw_panel("Contact Management", contacts_bounds, cx, |inner, cx| {
            let contacts = [
                ContactInfo::new("npub1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq")
                    .display_name("Alice Developer")
                    .nip05("alice@openagents.com")
                    .about("Building the future of decentralized AI")
                    .verification(ContactVerification::Verified)
                    .following(true)
                    .mutual(true),
                ContactInfo::new("npub1zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz")
                    .display_name("Bob Builder")
                    .nip05("bob@nostr.dev")
                    .about("Open source contributor")
                    .verification(ContactVerification::WebOfTrust)
                    .following(true)
                    .mutual(false),
                ContactInfo::new("npub1yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy")
                    .display_name("Anonymous")
                    .verification(ContactVerification::Unknown)
                    .following(false)
                    .mutual(false),
            ];

            for (i, contact) in contacts.iter().enumerate() {
                let mut card = ContactCard::new(contact.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 95.0,
                        inner.size.width.min(500.0),
                        90.0,
                    ),
                    cx,
                );
            }
        });
        y += contacts_height + SECTION_GAP;

        // ========== Panel 2: DM Conversations ==========
        let dm_height = panel_height(380.0);
        let dm_bounds = Bounds::new(bounds.origin.x, y, width, dm_height);
        draw_panel("Direct Messages", dm_bounds, cx, |inner, cx| {
            let messages = [
                DmMessage::new(
                    "m1",
                    "Hey! Just saw your PR, looks great!",
                    DmDirection::Incoming,
                )
                .sender("Alice")
                .timestamp("2 min ago")
                .encryption(EncryptionStatus::Encrypted)
                .read(true),
                DmMessage::new(
                    "m2",
                    "Thanks! Working on the review comments now.",
                    DmDirection::Outgoing,
                )
                .timestamp("1 min ago")
                .encryption(EncryptionStatus::Encrypted)
                .read(true),
                DmMessage::new(
                    "m3",
                    "Let me know when you push the updates. I'll review it tonight.",
                    DmDirection::Incoming,
                )
                .sender("Alice")
                .timestamp("Just now")
                .encryption(EncryptionStatus::Encrypted)
                .read(false),
                DmMessage::new(
                    "m4",
                    "[Encrypted message - decryption failed]",
                    DmDirection::Incoming,
                )
                .sender("Unknown")
                .timestamp("5 min ago")
                .encryption(EncryptionStatus::Failed)
                .read(false),
            ];

            for (i, msg) in messages.iter().enumerate() {
                let mut bubble = DmBubble::new(msg.clone());
                bubble.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 80.0,
                        inner.size.width.min(500.0),
                        75.0,
                    ),
                    cx,
                );
            }
        });
        y += dm_height + SECTION_GAP;

        // ========== Panel 3: Zaps & Lightning ==========
        let zaps_height = panel_height(280.0);
        let zaps_bounds = Bounds::new(bounds.origin.x, y, width, zaps_height);
        draw_panel("Zaps & Lightning", zaps_bounds, cx, |inner, cx| {
            let zaps = [
                ZapInfo::new("z1", 21000, "npub1alice...")
                    .sender_name("Alice")
                    .message("Great thread!")
                    .timestamp("5 min ago"),
                ZapInfo::new("z2", 1000000, "npub1bob...")
                    .sender_name("Bob")
                    .message("Thanks for the amazing tutorial!")
                    .timestamp("1 hour ago"),
                ZapInfo::new("z3", 500, "npub1anon...").timestamp("2 hours ago"),
            ];

            for (i, zap) in zaps.iter().enumerate() {
                let mut card = ZapCard::new(zap.clone());
                card.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y + i as f32 * 85.0,
                        inner.size.width.min(450.0),
                        80.0,
                    ),
                    cx,
                );
            }
        });
        y += zaps_height + SECTION_GAP;

        // ========== Panel 4: Relay Manager Organism ==========
        let relay_mgr_height = panel_height(420.0);
        let relay_mgr_bounds = Bounds::new(bounds.origin.x, y, width, relay_mgr_height);
        draw_panel(
            "Relay Manager (Organism)",
            relay_mgr_bounds,
            cx,
            |inner, cx| {
                let relays = vec![
                    RelayInfo::new("wss://relay.damus.io").status(RelayStatus::Connected),
                    RelayInfo::new("wss://nos.lol").status(RelayStatus::Connecting),
                    RelayInfo::new("wss://relay.nostr.band").status(RelayStatus::Connected),
                    RelayInfo::new("wss://relay.snort.social").status(RelayStatus::Disconnected),
                ];
                let mut manager = RelayManager::new(relays);
                manager.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width.min(500.0),
                        380.0,
                    ),
                    cx,
                );
            },
        );
        y += relay_mgr_height + SECTION_GAP;

        // ========== Panel 5: DM Thread Organism ==========
        let dm_thread_height = panel_height(450.0);
        let dm_thread_bounds = Bounds::new(bounds.origin.x, y, width, dm_thread_height);
        draw_panel("DM Thread (Organism)", dm_thread_bounds, cx, |inner, cx| {
            let messages = vec![
                DmMessage::new(
                    "m1",
                    "Hey! Just saw your PR, looks great!",
                    DmDirection::Incoming,
                )
                .sender("Alice")
                .timestamp("2 min ago")
                .encryption(EncryptionStatus::Encrypted)
                .read(true),
                DmMessage::new(
                    "m2",
                    "Thanks! Working on the review comments now.",
                    DmDirection::Outgoing,
                )
                .timestamp("1 min ago")
                .encryption(EncryptionStatus::Encrypted)
                .read(true),
                DmMessage::new(
                    "m3",
                    "Let me know when you push the updates.",
                    DmDirection::Incoming,
                )
                .sender("Alice")
                .timestamp("Just now")
                .encryption(EncryptionStatus::Encrypted)
                .read(false),
            ];
            let mut thread =
                DmThread::new("Alice Developer", "npub1abc123xyz789").messages(messages);
            thread.paint(
                Bounds::new(
                    inner.origin.x,
                    inner.origin.y,
                    inner.size.width.min(500.0),
                    400.0,
                ),
                cx,
            );
        });
        y += dm_thread_height + SECTION_GAP;

        // ========== Panel 6: Zap Flow Organism ==========
        let zap_flow_height = panel_height(420.0);
        let zap_flow_bounds = Bounds::new(bounds.origin.x, y, width, zap_flow_height);
        draw_panel(
            "Zap Flow Wizard (Organism)",
            zap_flow_bounds,
            cx,
            |inner, cx| {
                let mut flow = ZapFlow::new("Alice Developer", "npub1abc123xyz789...");
                flow.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width.min(400.0),
                        380.0,
                    ),
                    cx,
                );
            },
        );
        y += zap_flow_height + SECTION_GAP;

        // ========== Panel 7: Event Inspector Organism ==========
        let event_inspector_height = panel_height(400.0);
        let event_inspector_bounds = Bounds::new(bounds.origin.x, y, width, event_inspector_height);
        draw_panel(
            "Event Inspector (Organism)",
            event_inspector_bounds,
            cx,
            |inner, cx| {
                let event_data = EventData::new(
                    "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd",
                    "npub1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
                    1,
                )
                .content("GM! Building the future of decentralized AI. #OpenAgents #Nostr")
                .created_at(1700000000)
                .tags(vec![
                    TagData::new("t", vec!["OpenAgents".to_string()]),
                    TagData::new("t", vec!["Nostr".to_string()]),
                    TagData::new("p", vec!["npub1alice...".to_string()]),
                ])
                .sig("abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234")
                .verified(true);

                let mut inspector = EventInspector::new(event_data);
                inspector.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width.min(450.0),
                        350.0,
                    ),
                    cx,
                );
            },
        );
        y += event_inspector_height + SECTION_GAP;

        // ========== Panel 8: Status Reference ==========
        let ref_height = panel_height(180.0);
        let ref_bounds = Bounds::new(bounds.origin.x, y, width, ref_height);
        draw_panel("Nostr Status Reference", ref_bounds, cx, |inner, cx| {
            // Verification statuses
            let mut ver_x = inner.origin.x;
            let verifications = [
                ContactVerification::Verified,
                ContactVerification::WebOfTrust,
                ContactVerification::Unknown,
            ];

            for ver in &verifications {
                let ver_w = (ver.label().len() as f32 * 7.0) + 16.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(ver_x, inner.origin.y, ver_w, 20.0))
                        .with_background(ver.color().with_alpha(0.2))
                        .with_border(ver.color(), 1.0),
                );
                let text = cx.text.layout(
                    ver.label(),
                    Point::new(ver_x + 6.0, inner.origin.y + 4.0),
                    theme::font_size::XS,
                    ver.color(),
                );
                cx.scene.draw_text(text);
                ver_x += ver_w + 12.0;
            }

            // Encryption statuses
            let mut enc_x = inner.origin.x;
            let encryptions = [
                EncryptionStatus::Encrypted,
                EncryptionStatus::Decrypted,
                EncryptionStatus::Failed,
            ];

            for enc in &encryptions {
                let enc_w = 80.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(enc_x, inner.origin.y + 35.0, enc_w, 20.0))
                        .with_background(enc.color().with_alpha(0.2))
                        .with_border(enc.color(), 1.0),
                );
                let label = format!(
                    "{} {}",
                    enc.icon(),
                    match enc {
                        EncryptionStatus::Encrypted => "Encrypted",
                        EncryptionStatus::Decrypted => "Decrypted",
                        EncryptionStatus::Failed => "Failed",
                    }
                );
                let text = cx.text.layout(
                    &label,
                    Point::new(enc_x + 6.0, inner.origin.y + 39.0),
                    theme::font_size::XS,
                    enc.color(),
                );
                cx.scene.draw_text(text);
                enc_x += enc_w + 12.0;
            }

            // DM directions
            let mut dir_x = inner.origin.x;
            let directions = [
                ("Incoming", Hsla::new(200.0, 0.6, 0.5, 1.0)),
                ("Outgoing", theme::accent::PRIMARY),
            ];

            for (label, color) in &directions {
                let dir_w = 80.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(dir_x, inner.origin.y + 70.0, dir_w, 20.0))
                        .with_background(color.with_alpha(0.2))
                        .with_border(*color, 1.0),
                );
                let text = cx.text.layout(
                    label,
                    Point::new(dir_x + 6.0, inner.origin.y + 74.0),
                    theme::font_size::XS,
                    *color,
                );
                cx.scene.draw_text(text);
                dir_x += dir_w + 12.0;
            }
        });
    }

    fn paint_sovereign_agent_flows(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let width = bounds.size.width;
        let mut y = bounds.origin.y;

        // ========== Panel 1: Agent Profiles ==========
        let profiles_height = panel_height(340.0);
        let profiles_bounds = Bounds::new(bounds.origin.x, y, width, profiles_height);
        draw_panel(
            "Sovereign Agent Profiles",
            profiles_bounds,
            cx,
            |inner, cx| {
                let agents = [
                    AgentProfileInfo::new("agent-1", "CodeReviewer", AgentType::Sovereign)
                        .status(AgentStatus::Busy)
                        .npub("npub1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq")
                        .description("AI-powered code review with security analysis")
                        .capabilities(vec![
                            "code_review".to_string(),
                            "testing".to_string(),
                            "security".to_string(),
                        ])
                        .created_at("2 weeks ago")
                        .last_active("Just now"),
                    AgentProfileInfo::new("agent-2", "DataProcessor", AgentType::Custodial)
                        .status(AgentStatus::Idle)
                        .description("Processes and transforms data pipelines")
                        .capabilities(vec!["data_transform".to_string(), "etl".to_string()])
                        .created_at("1 month ago")
                        .last_active("5 min ago"),
                    AgentProfileInfo::new("agent-3", "MarketWatch", AgentType::Sovereign)
                        .status(AgentStatus::Online)
                        .description("Monitors market conditions and sends alerts")
                        .capabilities(vec!["monitoring".to_string(), "alerts".to_string()])
                        .created_at("3 days ago"),
                ];

                for (i, agent) in agents.iter().enumerate() {
                    let mut card = AgentProfileCard::new(agent.clone());
                    card.paint(
                        Bounds::new(
                            inner.origin.x,
                            inner.origin.y + i as f32 * 105.0,
                            inner.size.width.min(520.0),
                            100.0,
                        ),
                        cx,
                    );
                }
            },
        );
        y += profiles_height + SECTION_GAP;

        // ========== Panel 2: Signing Requests (FROSTR) ==========
        let signing_height = panel_height(400.0);
        let signing_bounds = Bounds::new(bounds.origin.x, y, width, signing_height);
        draw_panel(
            "Threshold Signing Requests",
            signing_bounds,
            cx,
            |inner, cx| {
                let requests = [
                    SigningRequestInfo::new(
                        "sr1",
                        SigningType::Transaction,
                        "Send 0.05 BTC to bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh",
                        "Agent-CodeReviewer",
                    )
                    .urgency(SigningUrgency::Urgent)
                    .threshold(1, 2)
                    .expires_in("5 minutes")
                    .created_at("2 min ago"),
                    SigningRequestInfo::new(
                        "sr2",
                        SigningType::Event,
                        "Publish NIP-90 job result event to nostr relays",
                        "Agent-DataProcessor",
                    )
                    .urgency(SigningUrgency::Normal)
                    .threshold(0, 3)
                    .expires_in("1 hour")
                    .created_at("10 min ago"),
                    SigningRequestInfo::new(
                        "sr3",
                        SigningType::Message,
                        "Sign DM reply to npub1alice...",
                        "Agent-MarketWatch",
                    )
                    .urgency(SigningUrgency::Normal)
                    .threshold(2, 2)
                    .created_at("1 hour ago"),
                    SigningRequestInfo::new(
                        "sr4",
                        SigningType::KeyRotation,
                        "Rotate threshold key shares - quarterly rotation",
                        "System",
                    )
                    .urgency(SigningUrgency::Expired)
                    .threshold(1, 3)
                    .expires_in("expired")
                    .created_at("2 days ago"),
                ];

                for (i, req) in requests.iter().enumerate() {
                    let mut card = SigningRequestCard::new(req.clone());
                    card.paint(
                        Bounds::new(
                            inner.origin.x,
                            inner.origin.y + i as f32 * 100.0,
                            inner.size.width.min(550.0),
                            95.0,
                        ),
                        cx,
                    );
                }
            },
        );
        y += signing_height + SECTION_GAP;

        // ========== Panel 3: Agent Status Matrix ==========
        let matrix_height = panel_height(280.0);
        let matrix_bounds = Bounds::new(bounds.origin.x, y, width, matrix_height);
        draw_panel("Agent Status Overview", matrix_bounds, cx, |inner, cx| {
            // Status summary header
            let statuses = [
                (AgentStatus::Online, 3),
                (AgentStatus::Busy, 2),
                (AgentStatus::Idle, 5),
                (AgentStatus::Error, 1),
                (AgentStatus::Offline, 0),
            ];

            let mut status_x = inner.origin.x;
            for (status, count) in &statuses {
                let status_w = 100.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(status_x, inner.origin.y, status_w, 50.0))
                        .with_background(status.color().with_alpha(0.1))
                        .with_border(status.color().with_alpha(0.5), 1.0),
                );
                let count_run = cx.text.layout(
                    &count.to_string(),
                    Point::new(status_x + 40.0, inner.origin.y + 8.0),
                    theme::font_size::LG,
                    status.color(),
                );
                cx.scene.draw_text(count_run);
                let label_run = cx.text.layout(
                    status.label(),
                    Point::new(status_x + 10.0, inner.origin.y + 32.0),
                    theme::font_size::XS,
                    status.color(),
                );
                cx.scene.draw_text(label_run);
                status_x += status_w + 8.0;
            }

            // Threshold key status
            let key_y = inner.origin.y + 70.0;
            let key_text = cx.text.layout(
                "Threshold Keys: 2-of-3 active",
                Point::new(inner.origin.x, key_y),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(key_text);

            // Key share indicators
            let shares = [
                ("Share 1", true, "Local"),
                ("Share 2", true, "Hardware Key"),
                ("Share 3", false, "Cloud Backup"),
            ];

            let mut share_x = inner.origin.x;
            for (label, active, location) in &shares {
                let share_w = 140.0;
                let color = if *active {
                    Hsla::new(120.0, 0.6, 0.45, 1.0)
                } else {
                    theme::text::MUTED
                };
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(share_x, key_y + 25.0, share_w, 40.0))
                        .with_background(color.with_alpha(0.1))
                        .with_border(color, 1.0),
                );
                let share_run = cx.text.layout(
                    label,
                    Point::new(share_x + 8.0, key_y + 30.0),
                    theme::font_size::XS,
                    color,
                );
                cx.scene.draw_text(share_run);
                let loc_run = cx.text.layout(
                    location,
                    Point::new(share_x + 8.0, key_y + 46.0),
                    10.0,
                    theme::text::DISABLED,
                );
                cx.scene.draw_text(loc_run);
                share_x += share_w + 10.0;
            }

            // Pending signatures counter
            let pending_y = key_y + 85.0;
            let pending_run = cx.text.layout(
                "Pending Signatures: 4",
                Point::new(inner.origin.x, pending_y),
                theme::font_size::SM,
                Hsla::new(30.0, 0.8, 0.5, 1.0),
            );
            cx.scene.draw_text(pending_run);
        });
        y += matrix_height + SECTION_GAP;

        // ========== Panel 4: Agent State Inspector Organism ==========
        let inspector_height = panel_height(450.0);
        let inspector_bounds = Bounds::new(bounds.origin.x, y, width, inspector_height);
        draw_panel(
            "Agent State Inspector (Organism)",
            inspector_bounds,
            cx,
            |inner, cx| {
                let goals = vec![
                    AgentGoal::new("g1", "Complete code review for PR #123")
                        .progress(0.75)
                        .status(AgentGoalStatus::Active),
                    AgentGoal::new("g2", "Run security scan on dependencies")
                        .progress(1.0)
                        .status(AgentGoalStatus::Completed),
                    AgentGoal::new("g3", "Waiting for API rate limit reset")
                        .progress(0.3)
                        .status(AgentGoalStatus::Blocked),
                ];
                let actions = vec![
                    AgentAction::new("Read", "Reading src/main.rs").timestamp("12:34"),
                    AgentAction::new("Edit", "Modified config.toml").timestamp("12:35"),
                    AgentAction::new("Bash", "Running tests...")
                        .timestamp("12:36")
                        .success(false),
                ];
                let resources = ResourceUsage {
                    tokens_used: 45000,
                    tokens_limit: 100000,
                    actions_count: 47,
                    runtime_seconds: 384,
                };
                let mut inspector = AgentStateInspector::new("CodeReviewer", "agent-123")
                    .goals(goals)
                    .actions(actions)
                    .memory(vec![
                        ("current_file".to_string(), "src/main.rs".to_string()),
                        ("branch".to_string(), "feature/auth".to_string()),
                    ])
                    .resources(resources);
                inspector.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width.min(500.0),
                        400.0,
                    ),
                    cx,
                );
            },
        );
        y += inspector_height + SECTION_GAP;

        // ========== Panel 5: Threshold Key Manager Organism ==========
        let key_mgr_height = panel_height(450.0);
        let key_mgr_bounds = Bounds::new(bounds.origin.x, y, width, key_mgr_height);
        draw_panel(
            "FROSTR Key Manager (Organism)",
            key_mgr_bounds,
            cx,
            |inner, cx| {
                let key_share = KeyShare::new("key-001", 1, 2, 3)
                    .created_at("2024-01-15")
                    .backed_up(true);
                let peers = vec![
                    ThresholdPeer::new("npub1alice...", "Alice (Local)", 1)
                        .status(PeerStatus::Online)
                        .last_seen("Now"),
                    ThresholdPeer::new("npub1bob...", "Bob (Hardware)", 2)
                        .status(PeerStatus::Signing)
                        .last_seen("Just now"),
                    ThresholdPeer::new("npub1carol...", "Carol (Cloud)", 3)
                        .status(PeerStatus::Offline)
                        .last_seen("5 min ago"),
                ];
                let requests = vec![
                    SigningRequest::new("req-1", "Sign Bitcoin transaction: 0.05 BTC")
                        .requester("CodeReviewer Agent")
                        .timestamp("2 min ago")
                        .progress(1, 2),
                ];
                let mut key_manager = ThresholdKeyManager::new()
                    .key_share(key_share)
                    .peers(peers)
                    .requests(requests);
                key_manager.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width.min(500.0),
                        400.0,
                    ),
                    cx,
                );
            },
        );
        y += key_mgr_height + SECTION_GAP;

        // ========== Panel 6: Schedule Configuration Organism ==========
        let schedule_height = panel_height(400.0);
        let schedule_bounds = Bounds::new(bounds.origin.x, y, width, schedule_height);
        draw_panel(
            "Schedule Configuration (Organism)",
            schedule_bounds,
            cx,
            |inner, cx| {
                let config = ScheduleData::new(ScheduleType::Continuous)
                    .heartbeat(30, IntervalUnit::Seconds)
                    .tick(5, IntervalUnit::Minutes)
                    .enabled(true)
                    .next_run(1700050000)
                    .last_run(1700000000);

                let mut schedule = ScheduleConfig::new(config);
                schedule.paint(
                    Bounds::new(
                        inner.origin.x,
                        inner.origin.y,
                        inner.size.width.min(450.0),
                        360.0,
                    ),
                    cx,
                );
            },
        );
        y += schedule_height + SECTION_GAP;

        // ========== Panel 7: Type & Status Reference ==========
        let ref_height = panel_height(180.0);
        let ref_bounds = Bounds::new(bounds.origin.x, y, width, ref_height);
        draw_panel("Agent Types & Statuses", ref_bounds, cx, |inner, cx| {
            // Agent types
            let mut type_x = inner.origin.x;
            let types = [AgentType::Human, AgentType::Sovereign, AgentType::Custodial];

            for agent_type in &types {
                let type_w = (agent_type.label().len() as f32 * 7.0) + 24.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(type_x, inner.origin.y, type_w, 22.0))
                        .with_background(agent_type.color().with_alpha(0.2))
                        .with_border(agent_type.color(), 1.0),
                );
                let icon = agent_type.icon();
                let icon_run = cx.text.layout(
                    icon,
                    Point::new(type_x + 4.0, inner.origin.y + 4.0),
                    theme::font_size::XS,
                    agent_type.color(),
                );
                cx.scene.draw_text(icon_run);
                let label_run = cx.text.layout(
                    agent_type.label(),
                    Point::new(type_x + 18.0, inner.origin.y + 4.0),
                    theme::font_size::XS,
                    agent_type.color(),
                );
                cx.scene.draw_text(label_run);
                type_x += type_w + 10.0;
            }

            // Agent statuses
            let mut status_x = inner.origin.x;
            let statuses = [
                AgentStatus::Online,
                AgentStatus::Busy,
                AgentStatus::Idle,
                AgentStatus::Error,
                AgentStatus::Offline,
            ];

            for status in &statuses {
                let status_w = (status.label().len() as f32 * 6.0) + 14.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(status_x, inner.origin.y + 35.0, status_w, 20.0))
                        .with_background(status.color().with_alpha(0.2))
                        .with_border(status.color(), 1.0),
                );
                let status_run = cx.text.layout(
                    status.label(),
                    Point::new(status_x + 6.0, inner.origin.y + 39.0),
                    theme::font_size::XS,
                    status.color(),
                );
                cx.scene.draw_text(status_run);
                status_x += status_w + 8.0;
            }

            // Signing types
            let mut sig_x = inner.origin.x;
            let sig_types = [
                SigningType::Transaction,
                SigningType::Message,
                SigningType::Event,
                SigningType::KeyRotation,
            ];

            for sig_type in &sig_types {
                let sig_w = (sig_type.label().len() as f32 * 6.0) + 22.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(sig_x, inner.origin.y + 70.0, sig_w, 20.0))
                        .with_background(sig_type.color().with_alpha(0.2))
                        .with_border(sig_type.color(), 1.0),
                );
                let icon_run = cx.text.layout(
                    sig_type.icon(),
                    Point::new(sig_x + 4.0, inner.origin.y + 74.0),
                    theme::font_size::XS,
                    sig_type.color(),
                );
                cx.scene.draw_text(icon_run);
                let sig_run = cx.text.layout(
                    sig_type.label(),
                    Point::new(sig_x + 16.0, inner.origin.y + 74.0),
                    theme::font_size::XS,
                    sig_type.color(),
                );
                cx.scene.draw_text(sig_run);
                sig_x += sig_w + 8.0;
            }

            // Urgency levels
            let mut urg_x = inner.origin.x;
            let urgencies = [
                SigningUrgency::Normal,
                SigningUrgency::Urgent,
                SigningUrgency::Expired,
            ];

            for urgency in &urgencies {
                let urg_w = (urgency.label().len() as f32 * 6.0) + 12.0;
                cx.scene.draw_quad(
                    Quad::new(Bounds::new(urg_x, inner.origin.y + 105.0, urg_w, 20.0))
                        .with_background(urgency.color().with_alpha(0.2))
                        .with_border(urgency.color(), 1.0),
                );
                let urg_run = cx.text.layout(
                    urgency.label(),
                    Point::new(urg_x + 5.0, inner.origin.y + 109.0),
                    theme::font_size::XS,
                    urgency.color(),
                );
                cx.scene.draw_text(urg_run);
                urg_x += urg_w + 8.0;
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
            Bounds::new(
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                hint_height,
            ),
            cx,
        );

        let items_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + hint_height + 10.0,
            bounds.size.width,
            bounds.size.height - hint_height - 10.0,
        );
        let gap = 12.0;
        let item_width = ((items_bounds.size.width - gap * (self.items.len() as f32 - 1.0))
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
        let item_width = ((items_bounds.size.width - gap * (self.items.len() as f32 - 1.0))
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
            InputEvent::KeyDown { key, modifiers } => match key {
                Key::Named(NamedKey::Tab) => {
                    if modifiers.shift {
                        self.focused = (self.focused + self.items.len() - 1) % self.items.len();
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
            },
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
        let x_anim = Animation::new(x, x, Duration::from_millis(500)).easing(Easing::EaseOutCubic);
        let y_anim = Animation::new(y, y, Duration::from_millis(500)).easing(Easing::EaseOutCubic);
        let w_anim = Animation::new(w, w, Duration::from_millis(300)).easing(Easing::EaseOutCubic);
        let h_anim = Animation::new(h, h, Duration::from_millis(300)).easing(Easing::EaseOutCubic);
        let mut alpha_anim =
            Animation::new(0.0, 1.0, Duration::from_millis(400)).easing(Easing::EaseOut);
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
            shake: SpringAnimation::new(0.0, 0.0)
                .stiffness(300.0)
                .damping(10.0),
            shake_target: 0.0,
            shake_phase: 0,
            content_type: "generic".to_string(),
        }
    }

    fn move_to(&mut self, x: f32, y: f32, animate: bool) {
        self.target_x = x;
        self.target_y = y;
        if animate {
            self.x_anim =
                Animation::new(self.x_anim.current_value(), x, Duration::from_millis(400))
                    .easing(Easing::EaseInOutCubic);
            self.y_anim =
                Animation::new(self.y_anim.current_value(), y, Duration::from_millis(400))
                    .easing(Easing::EaseInOutCubic);
            self.x_anim.start();
            self.y_anim.start();
        }
    }

    fn resize_to(&mut self, w: f32, h: f32, animate: bool) {
        self.target_w = w;
        self.target_h = h;
        if animate {
            self.w_anim =
                Animation::new(self.w_anim.current_value(), w, Duration::from_millis(300))
                    .easing(Easing::EaseInOutCubic);
            self.h_anim =
                Animation::new(self.h_anim.current_value(), h, Duration::from_millis(300))
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
        self.h_anim = Animation::new(
            self.h_anim.current_value(),
            30.0,
            Duration::from_millis(300),
        )
        .easing(Easing::EaseInOutCubic);
        self.h_anim.start();
    }

    fn close(&mut self) {
        self.state = PaneState::Closing;
        self.alpha_anim =
            Animation::new(1.0, 0.0, Duration::from_millis(300)).easing(Easing::EaseIn);
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
        let shake_offset = if self.shake_phase > 0 {
            self.shake.current()
        } else {
            0.0
        };
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
        self.panes
            .retain(|_, pane| pane.is_visible() || pane.state != PaneState::Closing);
    }

    fn paint(&self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.push_clip(bounds);
        cx.scene
            .draw_quad(Quad::new(bounds).with_background(theme::bg::APP));

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
            cx.scene
                .draw_quad(Quad::new(log_bounds).with_background(Hsla::new(0.0, 0.0, 0.05, 0.95)));
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
            self.create_pane(
                "terminal", "Terminal", 50.0, 340.0, 450.0, 180.0, "terminal",
            );
            self.scenario_index = 2;
        }
        if self.scenario_index == 2 && t >= 1.5 {
            self.create_pane("chat", "AI Assistant", 540.0, 60.0, 340.0, 230.0, "chat");
            self.scenario_index = 3;
        }
        if self.scenario_index == 3 && t >= 2.0 {
            self.create_pane(
                "diagnostics",
                "Diagnostics",
                540.0,
                320.0,
                340.0,
                200.0,
                "diagnostics",
            );
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
            self.tool_log.add(
                t,
                "Animate { id: \"diagnostics\", animation: \"Pulse\" }".to_string(),
            );
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
            .map(|c| {
                format!(
                    "#{:02x}{:02x}{:02x}",
                    (c.l * 255.0) as u8,
                    (c.s * 255.0) as u8,
                    (c.h as u8)
                )
            })
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

#[allow(dead_code)]
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

    scene.draw_quad(
        Quad::new(Bounds::new(bar_x1, y - bar_h + 2.0, bar_w, bar_h)).with_background(color),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(bar_x2, y - bar_h + 2.0, bar_w, bar_h)).with_background(color),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(bar_x1, y + font_size - 4.0, bar_w, bar_h)).with_background(color),
    );
    scene.draw_quad(
        Quad::new(Bounds::new(bar_x2, y + font_size - 4.0, bar_w, bar_h)).with_background(color),
    );

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

fn atoms_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(140.0), // Tool & Status Atoms
        panel_height(160.0), // Mode & Model Atoms
        panel_height(180.0), // Agent Status Badges
        panel_height(180.0), // Bitcoin & Payment Atoms
        panel_height(180.0), // Nostr Protocol Atoms
        panel_height(180.0), // GitAfter Atoms
        panel_height(180.0), // Marketplace Atoms
        panel_height(180.0), // Autopilot Atoms
        panel_height(160.0), // Interactive Atoms
    ];
    stacked_height(&panels)
}

fn arwes_frames_height(bounds: Bounds) -> f32 {
    let available = (bounds.size.width - PANEL_PADDING * 2.0).max(0.0);
    let permutations = FRAME_STYLES.len() * FRAME_ANIMATIONS.len() * FRAME_DIRECTIONS.len();
    let glow_palette = FRAME_STYLES.len() * FRAME_ANIMATIONS.len() * GLOW_PRESETS.len();
    let panels = [
        panel_height(
            grid_metrics(
                available,
                permutations,
                FRAME_TILE_W,
                FRAME_TILE_H,
                FRAME_TILE_GAP,
            )
            .height,
        ),
        panel_height(
            grid_metrics(
                available,
                FRAME_STYLES.len() * 2,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            )
            .height,
        ),
        panel_height(
            grid_metrics(
                available,
                FRAME_STYLES.len() * 2,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            )
            .height,
        ),
        panel_height(
            grid_metrics(
                available,
                glow_palette,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            )
            .height,
        ),
        panel_height(
            grid_metrics(
                available,
                16,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            )
            .height,
        ),
        panel_height(
            grid_metrics(
                available,
                2,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            )
            .height,
        ),
        panel_height(
            grid_metrics(
                available,
                4,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            )
            .height,
        ),
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
        panel_height(
            grid_metrics(available, moving_count, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height,
        ),
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
        panel_height(
            grid_metrics(
                available,
                8,
                ILLUMINATOR_TILE_W,
                ILLUMINATOR_TILE_H,
                ILLUMINATOR_TILE_GAP,
            )
            .height,
        ),
        panel_height(
            grid_metrics(
                available,
                4,
                ILLUMINATOR_TILE_W,
                ILLUMINATOR_TILE_H,
                ILLUMINATOR_TILE_GAP,
            )
            .height,
        ),
    ];
    stacked_height(&panels)
}

fn hud_widgets_height(bounds: Bounds) -> f32 {
    let available = (bounds.size.width - PANEL_PADDING * 2.0).max(0.0);
    let panels = [
        panel_height(grid_metrics(available, 6, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height), // Scanlines
        panel_height(grid_metrics(available, 6, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height), // Signal meters
        panel_height(grid_metrics(available, 6, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height), // Reticles
        panel_height(grid_metrics(available, 6, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height), // Resizable panes
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
        panel_height(180.0), // Tooltip demos
        panel_height(120.0), // StatusBar demos
        panel_height(260.0), // Notifications demos
        panel_height(200.0), // ContextMenu demo
        panel_height(240.0), // CommandPalette demo
    ];
    stacked_height(&panels)
}

fn chat_threads_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(480.0), // Simple Conversation
        panel_height(600.0), // Multi-Tool Workflow
        panel_height(520.0), // Code Editing Session
        panel_height(440.0), // Search & Navigation
        panel_height(320.0), // Streaming Response
        panel_height(800.0), // Complex Agent Session
        panel_height(280.0), // Error Handling
    ];
    stacked_height(&panels)
}

fn bitcoin_wallet_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(200.0), // Payment Method Icons
        panel_height(180.0), // Payment Status Badges
        panel_height(160.0), // Network Badges
        panel_height(200.0), // Bitcoin Amounts
        panel_height(220.0), // Balance Cards
        panel_height(300.0), // Payment Rows (Transaction History)
        panel_height(320.0), // Invoice Displays
        panel_height(400.0), // Complete Wallet Dashboard
    ];
    stacked_height(&panels)
}

fn nostr_protocol_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(160.0), // Relay Status Indicators
        panel_height(280.0), // Event Kind Badges
        panel_height(200.0), // Bech32 Entities
        panel_height(300.0), // Relay Connection List
        panel_height(320.0), // Complete Relay Dashboard
    ];
    stacked_height(&panels)
}

fn gitafter_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(160.0), // Issue Status Badges
        panel_height(180.0), // PR Status Badges
        panel_height(140.0), // Bounty Badges
        panel_height(160.0), // Stack Layer Indicators
        panel_height(180.0), // Agent Status + Type Badges
        panel_height(160.0), // Trajectory Status Badges
        panel_height(360.0), // Complete GitAfter Dashboard
    ];
    stacked_height(&panels)
}

fn sovereign_agents_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(160.0), // Threshold Key Badges
        panel_height(180.0), // Agent Schedule Badges
        panel_height(160.0), // Goal Progress Badges
        panel_height(180.0), // Tick Event Badges
        panel_height(180.0), // Skill License Badges
        panel_height(400.0), // Complete Agent Dashboard Preview
    ];
    stacked_height(&panels)
}

fn marketplace_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(140.0), // Market Type Badges
        panel_height(180.0), // Job Status Badges
        panel_height(160.0), // Reputation Badges
        panel_height(180.0), // Trajectory Source Badges
        panel_height(180.0), // Earnings Badges
        panel_height(400.0), // Complete Marketplace Dashboard
    ];
    stacked_height(&panels)
}

fn autopilot_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(180.0), // Session Status Badges
        panel_height(160.0), // APM Gauges
        panel_height(180.0), // Resource Usage Bars
        panel_height(160.0), // Daemon Status Badges
        panel_height(180.0), // Parallel Agent Badges
        panel_height(400.0), // Complete Autopilot Dashboard
    ];
    stacked_height(&panels)
}

fn thread_components_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(160.0), // Thread Headers
        panel_height(180.0), // Message Editor States
        panel_height(200.0), // Thread Feedback
        panel_height(140.0), // Entry Actions
        panel_height(140.0), // Terminal Headers
        panel_height(400.0), // Complete Thread Layout
    ];
    stacked_height(&panels)
}

fn sessions_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(280.0), // Session Cards (2 rows of 3)
        panel_height(120.0), // Session Breadcrumbs
        panel_height(180.0), // Session Search & Filters
        panel_height(160.0), // Session Actions
        panel_height(320.0), // Complete Session List
    ];
    stacked_height(&panels)
}

fn permissions_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(160.0), // Permission Decisions
        panel_height(240.0), // Permission Rules
        panel_height(280.0), // Permission History
        panel_height(200.0), // Permission Bar Variants
        panel_height(140.0), // Permission Statistics
    ];
    stacked_height(&panels)
}

fn apm_metrics_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(200.0), // APM Gauge Variations
        panel_height(220.0), // APM Session Rows
        panel_height(280.0), // Session Comparison
        panel_height(320.0), // APM Leaderboard
        panel_height(200.0), // APM Trends Summary
    ];
    stacked_height(&panels)
}

fn wallet_flows_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(260.0), // Mnemonic Display
        panel_height(180.0), // Address Cards
        panel_height(280.0), // Transaction History
        panel_height(360.0), // Send Flow
        panel_height(420.0), // Receive Flow
    ];
    stacked_height(&panels)
}

fn gitafter_flows_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(240.0), // Repository Cards
        panel_height(320.0), // Issue List
        panel_height(280.0), // PR Timeline
        panel_height(200.0), // Issue Labels & Statuses
    ];
    stacked_height(&panels)
}

fn marketplace_flows_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(260.0), // Compute Providers
        panel_height(280.0), // Skills Marketplace
        panel_height(280.0), // Data Marketplace
        panel_height(180.0), // Categories & Formats Reference
    ];
    stacked_height(&panels)
}

fn nostr_flows_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(320.0), // Contact Cards
        panel_height(380.0), // DM Conversations
        panel_height(280.0), // Zaps & Lightning
        panel_height(420.0), // Relay Manager Organism
        panel_height(450.0), // DM Thread Organism
        panel_height(420.0), // Zap Flow Organism
        panel_height(400.0), // Event Inspector Organism
        panel_height(180.0), // Status Reference
    ];
    stacked_height(&panels)
}

fn sovereign_agent_flows_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(340.0), // Agent Profiles
        panel_height(400.0), // Signing Requests
        panel_height(280.0), // Agent Status Matrix
        panel_height(450.0), // Agent State Inspector Organism
        panel_height(450.0), // Threshold Key Manager Organism
        panel_height(400.0), // Schedule Configuration Organism
        panel_height(180.0), // Type & Status Reference
    ];
    stacked_height(&panels)
}
