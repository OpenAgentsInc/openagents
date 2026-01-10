use web_time::Instant;

use wgpui::components::atoms::{Mode, Model, StreamingIndicator};
use wgpui::components::molecules::{AssistantMessage, CheckpointRestore, ModeSelector, ModelSelector, PermissionBar, ThinkingBlock};
use wgpui::components::organisms::PermissionDialog;
use wgpui::components::organisms::ThreadControls;
use wgpui::{Animation, Bounds, EventContext, Easing, Hsla, InputEvent, MouseButton, PaintContext, Point, Quad, Text, theme};

use crate::constants::{
    GAP, HEADER_HEIGHT, MARGIN, NAV_ITEM_HEIGHT, NAV_WIDTH, PANEL_PADDING, SECTION_APM_METRICS,
    SECTION_ARWES_BACKGROUNDS, SECTION_ARWES_FRAMES, SECTION_ARWES_ILLUMINATOR, SECTION_ARWES_TEXT,
    SECTION_ATOMS, SECTION_AUTOPILOT, SECTION_BITCOIN_WALLET, SECTION_CHAT_THREADS, SECTION_GITAFTER,
    SECTION_GITAFTER_FLOWS, SECTION_HUD_WIDGETS, SECTION_INTERACTIONS, SECTION_LIGHT_DEMO,
    SECTION_MARKETPLACE, SECTION_MARKETPLACE_FLOWS, SECTION_MOLECULES, SECTION_NOSTR_FLOWS,
    SECTION_NOSTR_PROTOCOL, SECTION_ORGANISMS, SECTION_OVERVIEW, SECTION_PERMISSIONS,
    SECTION_SESSIONS, SECTION_SOVEREIGN_AGENT_FLOWS, SECTION_SOVEREIGN_AGENTS, SECTION_SYSTEM_UI,
    SECTION_THREAD_COMPONENTS, SECTION_TOOLCALL_DEMO, SECTION_WALLET_FLOWS,
};
use crate::demos::{FocusDemo, ToolcallDemo};
use crate::sections::heights::{
    apm_metrics_height, arwes_backgrounds_height, arwes_frames_height, arwes_illuminator_height,
    arwes_text_effects_height, atoms_height, autopilot_height, bitcoin_wallet_height,
    chat_threads_height, gitafter_flows_height, gitafter_height, hud_widgets_height,
    light_demo_height, marketplace_flows_height, marketplace_height, nostr_flows_height,
    nostr_protocol_height, permissions_height, sessions_height, sovereign_agent_flows_height,
    sovereign_agents_height, system_ui_height, thread_components_height, toolcall_demo_height,
    wallet_flows_height,
};

pub(crate) struct StoryLayout {
    header: Bounds,
    nav: Bounds,
    content: Bounds,
}

pub(crate) struct Storybook {
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
    pub(crate) fn new() -> Self {
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

        let mut light_frame_anim = Animation::new(0.0_f32, 1.0, std::time::Duration::from_millis(2400))
            .easing(Easing::EaseInOutCubic)
            .iterations(0)
            .alternate();
        light_frame_anim.start();

        let mut glow_pulse_anim = Animation::new(0.4_f32, 1.0, std::time::Duration::from_millis(1800))
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

    pub(crate) fn tick(&mut self) {
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

    pub(crate) fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
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

    pub(crate) fn handle_input(&mut self, event: &InputEvent, bounds: Bounds) -> bool {
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
            InputEvent::MouseDown { button, x, y, .. } => {
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
}
