use std::time::{Duration, Instant};
use wgpui::components::atoms::{
    EntryType, Mode, ModeBadge, Model, ModelBadge, Status, StatusDot, StreamingIndicator,
};
use wgpui::components::hud::{Command, CommandPalette, Notifications, StatusBar, StatusItem};
use wgpui::components::molecules::{MessageHeader, ModeSelector, ModelSelector};
use wgpui::{
    Animation, Bounds, Button, ButtonVariant, Component, ContextMenu, Easing, MenuItem,
    PaintContext, Point, Quad, Scene, SpringAnimation, TextInput, Tooltip, TooltipPosition,
    VirtualList, theme,
};

pub const DEFAULT_COMPONENT_SHOWCASE_WIDTH: f64 = 1200.0;
pub const DEFAULT_COMPONENT_SHOWCASE_HEIGHT: f64 = 900.0;

pub struct ComponentShowcaseState {
    #[allow(dead_code)]
    start_time: Instant,
    position_anim: Animation<f32>,
    color_anim: Animation<wgpui::Hsla>,
    spring: SpringAnimation<f32>,
    #[allow(dead_code)]
    tooltip: Tooltip,
    #[allow(dead_code)]
    context_menu: ContextMenu,
    #[allow(dead_code)]
    command_palette: CommandPalette,
    status_bar: StatusBar,
    notifications: Notifications,
    text_input: TextInput,
    selected_mode: Mode,
    selected_model: Model,
    #[allow(dead_code)]
    hover_button: Option<usize>,
    message_count: usize,
}

impl ComponentShowcaseState {
    pub fn tick(&mut self, delta: Duration) {
        self.position_anim.tick(delta);
        self.color_anim.tick(delta);
        self.spring.tick(delta);
    }
}

impl Default for ComponentShowcaseState {
    fn default() -> Self {
        let mut position_anim = Animation::new(0.0_f32, 200.0, Duration::from_millis(2000))
            .easing(Easing::EaseInOutCubic)
            .iterations(0)
            .alternate();
        position_anim.start();

        let mut color_anim = Animation::new(
            theme::accent::PRIMARY,
            theme::accent::GREEN,
            Duration::from_millis(3000),
        )
        .easing(Easing::EaseInOut)
        .iterations(0)
        .alternate();
        color_anim.start();

        let spring = SpringAnimation::new(0.0, 100.0)
            .stiffness(80.0)
            .damping(8.0);

        Self {
            start_time: Instant::now(),
            position_anim,
            color_anim,
            spring,
            tooltip: Tooltip::new("This button does something amazing!")
                .position(TooltipPosition::Top)
                .delay(20),
            context_menu: ContextMenu::new().items(vec![
                MenuItem::new("new", "New File").shortcut("Cmd+N"),
                MenuItem::new("open", "Open...").shortcut("Cmd+O"),
                MenuItem::separator(),
                MenuItem::new("save", "Save").shortcut("Cmd+S"),
                MenuItem::new("saveas", "Save As...").shortcut("Cmd+Shift+S"),
                MenuItem::separator(),
                MenuItem::new("close", "Close").shortcut("Cmd+W"),
            ]),
            command_palette: CommandPalette::new().commands(vec![
                Command::new("file.new", "New File").keybinding("Cmd+N"),
                Command::new("file.open", "Open File").keybinding("Cmd+O"),
                Command::new("file.save", "Save").keybinding("Cmd+S"),
                Command::new("edit.undo", "Undo").keybinding("Cmd+Z"),
                Command::new("edit.redo", "Redo").keybinding("Cmd+Shift+Z"),
                Command::new("view.zoom", "Zoom In").keybinding("Cmd++"),
            ]),
            status_bar: StatusBar::new().items(vec![
                StatusItem::mode("mode", Mode::Normal).left(),
                StatusItem::text("file", "component_showcase.rs").center(),
                StatusItem::model("model", Model::Codex).right(),
                StatusItem::status("status", Status::Online).right(),
            ]),
            notifications: Notifications::new(),
            text_input: TextInput::new().placeholder("Type something..."),
            selected_mode: Mode::Normal,
            selected_model: Model::Codex,
            hover_button: None,
            message_count: 10000,
        }
    }
}

pub fn build_component_showcase(
    scene: &mut Scene,
    text_system: &mut wgpui::TextSystem,
    demo: &mut ComponentShowcaseState,
    width: f32,
    height: f32,
) {
    let margin = 24.0;
    let col_width = (width - margin * 3.0) / 2.0;

    scene
        .draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    let mut y = margin;
    draw_header(scene, text_system, margin, &mut y, width);

    let left_x = margin;
    let right_x = margin * 2.0 + col_width;

    let mut left_y = y;
    let mut right_y = y;

    draw_animation_section(scene, text_system, demo, left_x, col_width, &mut left_y);
    draw_atoms_section(scene, text_system, right_x, col_width, &mut right_y);

    left_y += 32.0;
    right_y += 32.0;

    draw_buttons_section(scene, text_system, left_x, col_width, &mut left_y);
    draw_inputs_section(scene, text_system, demo, right_x, col_width, &mut right_y);

    left_y += 32.0;
    right_y += 32.0;

    draw_virtual_list_section(scene, text_system, demo, left_x, col_width, &mut left_y);
    draw_selectors_section(scene, text_system, demo, right_x, col_width, &mut right_y);

    demo.status_bar.paint(
        Bounds::new(0.0, 0.0, width, height),
        &mut PaintContext::new(scene, text_system, 1.0),
    );
    demo.notifications.paint(
        Bounds::new(0.0, 0.0, width, height),
        &mut PaintContext::new(scene, text_system, 1.0),
    );
}

fn draw_header(
    scene: &mut Scene,
    text_system: &mut wgpui::TextSystem,
    margin: f32,
    y: &mut f32,
    width: f32,
) {
    let title = "wgpui Component Showcase";
    let subtitle = "GPU-Accelerated UI • 40+ Components • Animation • Accessibility";

    let title_run = text_system.layout(
        title,
        Point::new(margin, *y + 24.0),
        28.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(title_run);

    let subtitle_run = text_system.layout(
        subtitle,
        Point::new(margin, *y + 52.0),
        14.0,
        theme::text::MUTED,
    );
    scene.draw_text(subtitle_run);

    scene.draw_quad(
        Quad::new(Bounds::new(margin, *y + 70.0, width - margin * 2.0, 2.0))
            .with_background(theme::accent::PRIMARY),
    );

    *y += 90.0;
}

fn draw_section_title(
    scene: &mut Scene,
    text_system: &mut wgpui::TextSystem,
    x: f32,
    y: &mut f32,
    title: &str,
) {
    let run = text_system.layout(title, Point::new(x, *y + 16.0), 18.0, theme::text::PRIMARY);
    scene.draw_text(run);
    *y += 32.0;
}

fn draw_animation_section(
    scene: &mut Scene,
    text_system: &mut wgpui::TextSystem,
    demo: &ComponentShowcaseState,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_title(scene, text_system, x, y, "Animation System");

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 120.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let anim_x = x + demo.position_anim.current_value();
    let anim_color = demo.color_anim.current_value();
    scene.draw_quad(
        Quad::new(Bounds::new(anim_x + 10.0, *y + 20.0, 40.0, 40.0)).with_background(anim_color),
    );

    let label = text_system.layout(
        "Position + Color Animation",
        Point::new(x + 10.0, *y + 80.0),
        12.0,
        theme::text::MUTED,
    );
    scene.draw_text(label);

    let spring_val = demo.spring.current();
    scene.draw_quad(
        Quad::new(Bounds::new(
            x + width - 60.0,
            *y + 20.0 + (100.0 - spring_val) * 0.5,
            40.0,
            40.0,
        ))
        .with_background(theme::accent::PURPLE),
    );

    let spring_label = text_system.layout(
        "Spring Physics",
        Point::new(x + width - 100.0, *y + 100.0),
        12.0,
        theme::text::MUTED,
    );
    scene.draw_text(spring_label);

    *y += 130.0;
}

fn draw_atoms_section(
    scene: &mut Scene,
    text_system: &mut wgpui::TextSystem,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_title(scene, text_system, x, y, "Atoms");

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 120.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    let mut atom_x = x + 16.0;
    let atom_y = *y + 20.0;

    StatusDot::new(Status::Online).paint(Bounds::new(atom_x, atom_y, 12.0, 12.0), &mut cx);
    atom_x += 30.0;

    StatusDot::new(Status::Busy).paint(Bounds::new(atom_x, atom_y, 12.0, 12.0), &mut cx);
    atom_x += 30.0;

    StatusDot::new(Status::Away).paint(Bounds::new(atom_x, atom_y, 12.0, 12.0), &mut cx);
    atom_x += 30.0;

    StatusDot::new(Status::Offline).paint(Bounds::new(atom_x, atom_y, 12.0, 12.0), &mut cx);
    atom_x += 50.0;

    ModeBadge::new(Mode::Normal).paint(Bounds::new(atom_x, atom_y - 4.0, 60.0, 20.0), &mut cx);
    atom_x += 70.0;

    ModeBadge::new(Mode::Plan).paint(Bounds::new(atom_x, atom_y - 4.0, 60.0, 20.0), &mut cx);
    atom_x += 70.0;

    ModeBadge::new(Mode::Act).paint(Bounds::new(atom_x, atom_y - 4.0, 60.0, 20.0), &mut cx);

    let badge_y = atom_y + 40.0;
    atom_x = x + 16.0;

    ModelBadge::new(Model::Codex).paint(Bounds::new(atom_x, badge_y, 80.0, 24.0), &mut cx);
    atom_x += 90.0;

    ModelBadge::new(Model::Gpt4).paint(Bounds::new(atom_x, badge_y, 80.0, 24.0), &mut cx);
    atom_x += 90.0;

    ModelBadge::new(Model::Gemini).paint(Bounds::new(atom_x, badge_y, 80.0, 24.0), &mut cx);
    atom_x += 100.0;

    StreamingIndicator::new().paint(Bounds::new(atom_x, badge_y + 4.0, 40.0, 16.0), &mut cx);

    *y += 130.0;
}

fn draw_buttons_section(
    scene: &mut Scene,
    text_system: &mut wgpui::TextSystem,
    x: f32,
    _width: f32,
    y: &mut f32,
) {
    draw_section_title(scene, text_system, x, y, "Button Variants");

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    let btn_height = 36.0;
    let btn_width = 100.0;
    let spacing = 12.0;
    let mut btn_x = x;

    Button::new("Primary").paint(Bounds::new(btn_x, *y, btn_width, btn_height), &mut cx);
    btn_x += btn_width + spacing;

    Button::new("Secondary")
        .variant(ButtonVariant::Secondary)
        .paint(Bounds::new(btn_x, *y, btn_width, btn_height), &mut cx);
    btn_x += btn_width + spacing;

    Button::new("Ghost")
        .variant(ButtonVariant::Ghost)
        .paint(Bounds::new(btn_x, *y, btn_width, btn_height), &mut cx);
    btn_x += btn_width + spacing;

    Button::new("Danger")
        .variant(ButtonVariant::Danger)
        .paint(Bounds::new(btn_x, *y, btn_width, btn_height), &mut cx);
    btn_x += btn_width + spacing;

    Button::new("Disabled")
        .disabled(true)
        .paint(Bounds::new(btn_x, *y, btn_width, btn_height), &mut cx);

    *y += btn_height + 16.0;
}

fn draw_inputs_section(
    scene: &mut Scene,
    text_system: &mut wgpui::TextSystem,
    demo: &mut ComponentShowcaseState,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_title(scene, text_system, x, y, "Text Input");

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    demo.text_input
        .paint(Bounds::new(x, *y, width - 20.0, 36.0), &mut cx);

    *y += 52.0;
}

fn draw_virtual_list_section(
    scene: &mut Scene,
    text_system: &mut wgpui::TextSystem,
    demo: &ComponentShowcaseState,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_title(
        scene,
        text_system,
        x,
        y,
        &format!("Virtual List ({} items)", demo.message_count),
    );

    scene.draw_quad(
        Quad::new(Bounds::new(x, *y, width, 200.0))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let items: Vec<String> = (0..20)
        .map(|i| format!("Message #{} - Virtualized for performance", i))
        .collect();

    let item_height = 32.0;
    let mut virtual_list = VirtualList::new(
        items,
        item_height,
        move |item: &String, idx: usize, bounds: Bounds, cx: &mut PaintContext| {
            let bg = if idx % 2 == 0 {
                theme::bg::SURFACE
            } else {
                theme::bg::MUTED
            };
            cx.scene.draw_quad(Quad::new(bounds).with_background(bg));

            let run = cx.text.layout(
                item,
                Point::new(
                    bounds.origin.x + 12.0,
                    bounds.origin.y + bounds.size.height * 0.6,
                ),
                theme::font_size::SM,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(run);
        },
    );

    let mut cx = PaintContext::new(scene, text_system, 1.0);
    virtual_list.paint(Bounds::new(x + 1.0, *y + 1.0, width - 2.0, 198.0), &mut cx);

    *y += 210.0;
}

fn draw_selectors_section(
    scene: &mut Scene,
    text_system: &mut wgpui::TextSystem,
    demo: &ComponentShowcaseState,
    x: f32,
    width: f32,
    y: &mut f32,
) {
    draw_section_title(scene, text_system, x, y, "Mode & Model Selectors");

    {
        let mut cx = PaintContext::new(scene, text_system, 1.0);
        ModeSelector::new(demo.selected_mode).paint(Bounds::new(x, *y, 150.0, 32.0), &mut cx);
        ModelSelector::new(demo.selected_model)
            .paint(Bounds::new(x + 170.0, *y, 150.0, 32.0), &mut cx);
    }

    *y += 48.0;

    draw_section_title(scene, text_system, x, y, "Message Header");

    {
        let mut cx = PaintContext::new(scene, text_system, 1.0);
        MessageHeader::new(EntryType::Assistant)
            .model(Model::Codex)
            .paint(Bounds::new(x, *y, width, 40.0), &mut cx);
    }

    *y += 56.0;
}
