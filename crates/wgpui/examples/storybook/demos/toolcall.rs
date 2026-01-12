use std::collections::HashMap;
use std::time::Duration;

use wgpui::components::hud::{
    DotShape, DotsGrid, DotsOrigin, DrawDirection, FrameAnimation, FrameStyle,
};
use wgpui::{
    Animation, Bounds, Component, Easing, Hsla, PaintContext, Point, Quad, SpringAnimation, theme,
};

use crate::helpers::demo_frame;

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

pub(crate) struct ToolcallDemo {
    panes: HashMap<String, ToolcallPane>,
    z_counter: i32,
    tool_log: ToolCallLog,
    elapsed: f32,
    scenario_index: usize,
    dots_anim: Animation<f32>,
    frame_anim: Animation<f32>,
}

impl ToolcallDemo {
    pub(crate) fn new() -> Self {
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

    pub(crate) fn tick(&mut self, dt: Duration) {
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

    pub(crate) fn paint(&self, bounds: Bounds, cx: &mut PaintContext) {
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
