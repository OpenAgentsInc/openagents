use crate::deck::model::{Deck, DeckTheme, Slide, SlideLayout};
use crate::deck::parser::presentation_markdown_config;
use crate::state::DeckState;
use wgpui::components::Component;
use wgpui::components::hud::{DotShape, DotsGrid, DotsOrigin};
use wgpui::markdown::MarkdownRenderer;
use wgpui::{
    Bounds, Cursor, Hsla, InputEvent, Key, MouseButton, NamedKey, PaintContext, Point, Quad, theme,
};

pub struct DeckApp {
    state: DeckState,
    markdown_renderer: MarkdownRenderer,
}

impl DeckApp {
    pub fn new(deck: Deck) -> Result<Self, String> {
        Ok(Self {
            state: DeckState::new(deck)?,
            markdown_renderer: MarkdownRenderer::with_config(presentation_markdown_config()),
        })
    }

    pub fn request_redraw(&mut self) {
        self.state.request_redraw();
    }

    pub fn take_redraw_request(&mut self) -> bool {
        self.state.take_redraw_request()
    }

    pub fn handle_input(&mut self, event: &InputEvent, viewport: Bounds) -> bool {
        let changed = match event {
            InputEvent::KeyDown { key, .. } => self.handle_key_down(key),
            InputEvent::MouseDown {
                button: MouseButton::Left,
                x,
                y,
                ..
            } => self.handle_click(Point::new(*x, *y), viewport),
            InputEvent::Scroll { dy, .. } if *dy >= 36.0 => self.state.advance(),
            InputEvent::Scroll { dy, .. } if *dy <= -36.0 => self.state.retreat(),
            _ => false,
        };

        if changed {
            self.state.request_redraw();
        }

        changed
    }

    pub fn cursor_for_point(&self, point: Point, viewport: Bounds) -> Cursor {
        if self.click_target(point, viewport).is_some() {
            Cursor::Pointer
        } else {
            Cursor::Default
        }
    }

    pub fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let slide = self.state.current_slide().cloned();
        let theme = slide
            .as_ref()
            .map(|slide| &slide.theme)
            .unwrap_or(&self.state.deck().metadata.theme);
        let palette = palette_for_theme(theme);
        let content_bounds = content_bounds(bounds);

        cx.scene
            .draw_quad(Quad::new(bounds).with_background(Hsla::from_hex(0x020203)));
        paint_background_wash(bounds, &palette, cx);

        let mut dots = DotsGrid::new()
            .color(palette.grid)
            .shape(DotShape::Cross)
            .distance(30.0)
            .size(3.5)
            .cross_thickness(1.0)
            .opacity(0.12)
            .origin(DotsOrigin::Center);
        dots.paint(bounds, cx);

        if let Some(slide) = slide.as_ref() {
            self.paint_slide(slide, content_bounds, &palette, cx);
        }
    }

    fn handle_key_down(&mut self, key: &Key) -> bool {
        match key {
            Key::Named(NamedKey::ArrowRight)
            | Key::Named(NamedKey::PageDown)
            | Key::Named(NamedKey::Enter)
            | Key::Named(NamedKey::Space) => self.state.advance(),
            Key::Named(NamedKey::ArrowLeft)
            | Key::Named(NamedKey::PageUp)
            | Key::Named(NamedKey::Backspace) => self.state.retreat(),
            Key::Named(NamedKey::Home) => self.state.jump_to(0),
            Key::Named(NamedKey::End) => self.state.jump_to(self.state.slide_count() - 1),
            _ => false,
        }
    }

    fn handle_click(&mut self, point: Point, viewport: Bounds) -> bool {
        match self.click_target(point, viewport) {
            Some(ClickTarget::Previous) => self.state.retreat(),
            Some(ClickTarget::Next) => self.state.advance(),
            None => false,
        }
    }

    fn click_target(&self, point: Point, viewport: Bounds) -> Option<ClickTarget> {
        if !viewport.contains(point) {
            return None;
        }

        let left_zone = viewport.min_x() + viewport.width() * 0.18;
        let right_zone = viewport.max_x() - viewport.width() * 0.18;
        let can_go_back = self.state.current_slide_index() > 0;
        let can_go_forward = self.state.current_slide_index() + 1 < self.state.slide_count();

        if point.x <= left_zone && can_go_back {
            Some(ClickTarget::Previous)
        } else if point.x >= right_zone && can_go_forward {
            Some(ClickTarget::Next)
        } else {
            None
        }
    }

    fn paint_slide(
        &self,
        slide: &Slide,
        content_bounds: Bounds,
        palette: &DeckPalette,
        cx: &mut PaintContext,
    ) {
        match slide.layout {
            SlideLayout::Title => self.paint_title_slide(slide, content_bounds, palette, cx),
            SlideLayout::Body | SlideLayout::TwoColumn | SlideLayout::Code => {
                self.paint_market_slide(slide, content_bounds, palette, cx)
            }
        }
    }

    fn paint_title_slide(
        &self,
        slide: &Slide,
        content_bounds: Bounds,
        palette: &DeckPalette,
        cx: &mut PaintContext,
    ) {
        if is_minimal_title_slide(slide) {
            self.paint_minimal_title_slide(slide, content_bounds, cx);
            return;
        }

        let body_top = self.paint_slide_header(slide, content_bounds, palette, 42.0, cx);
        let panel_bounds = Bounds::new(
            content_bounds.min_x(),
            body_top,
            content_bounds.width(),
            (content_bounds.max_y() - body_top).max(1.0),
        );

        draw_panel(
            cx,
            panel_bounds,
            palette.panel_bg,
            palette.panel_border,
            18.0,
        );
        self.paint_markdown_panel(
            slide,
            panel_bounds.inset(22.0),
            Point::new(panel_bounds.min_x() + 22.0, panel_bounds.min_y() + 24.0),
            cx,
        );
    }

    fn paint_minimal_title_slide(
        &self,
        slide: &Slide,
        content_bounds: Bounds,
        cx: &mut PaintContext,
    ) {
        let title_width = (content_bounds.width() * 0.72).clamp(320.0, 860.0);
        let title_y = content_bounds.min_y() + content_bounds.height() * 0.32;
        let title_bounds = Bounds::new(
            content_bounds.min_x() + (content_bounds.width() - title_width) * 0.5,
            title_y,
            title_width,
            (content_bounds.height() * 0.28).max(120.0),
        );

        let title_height = draw_wrapped_text(
            cx,
            slide.title.as_str(),
            title_bounds,
            58.0,
            theme::text::PRIMARY,
            TextAlign::Center,
            false,
        );

        if let Some(summary) = slide.summary.as_deref() {
            draw_wrapped_text(
                cx,
                summary,
                Bounds::new(
                    title_bounds.min_x(),
                    title_y + title_height + 20.0,
                    title_bounds.width(),
                    40.0,
                ),
                18.0,
                theme::text::SECONDARY,
                TextAlign::Center,
                false,
            );
        }
    }

    fn paint_market_slide(
        &self,
        slide: &Slide,
        content_bounds: Bounds,
        palette: &DeckPalette,
        cx: &mut PaintContext,
    ) {
        let body_top = self.paint_slide_header(slide, content_bounds, palette, 34.0, cx);
        let panel_bounds = Bounds::new(
            content_bounds.min_x(),
            body_top,
            content_bounds.width(),
            (content_bounds.max_y() - body_top).max(1.0),
        );

        draw_panel(
            cx,
            panel_bounds,
            palette.panel_bg,
            palette.panel_border,
            16.0,
        );
        draw_mono_text(
            cx,
            "CORE POINTS",
            Point::new(panel_bounds.min_x() + 20.0, panel_bounds.min_y() + 18.0),
            11.0,
            palette.accent,
        );

        self.paint_markdown_panel(
            slide,
            Bounds::new(
                panel_bounds.min_x() + 22.0,
                panel_bounds.min_y() + 38.0,
                (panel_bounds.width() - 44.0).max(1.0),
                (panel_bounds.height() - 54.0).max(1.0),
            ),
            Point::new(panel_bounds.min_x() + 22.0, panel_bounds.min_y() + 38.0),
            cx,
        );
    }

    fn paint_slide_header(
        &self,
        slide: &Slide,
        content_bounds: Bounds,
        palette: &DeckPalette,
        title_size: f32,
        cx: &mut PaintContext,
    ) -> f32 {
        let mut cursor_y = content_bounds.min_y();
        if let Some(eyebrow) = slide.eyebrow.as_deref() {
            draw_mono_text(
                cx,
                eyebrow,
                Point::new(content_bounds.min_x(), cursor_y),
                12.0,
                palette.accent,
            );
            cursor_y += 20.0;
        }

        let title_height = draw_wrapped_text(
            cx,
            slide.title.as_str(),
            Bounds::new(
                content_bounds.min_x(),
                cursor_y,
                content_bounds.width(),
                title_size * 2.4,
            ),
            title_size,
            theme::text::PRIMARY,
            TextAlign::Left,
            false,
        );
        cursor_y += title_height + 10.0;

        if let Some(summary) = slide.summary.as_deref() {
            let summary_width = if slide.layout == SlideLayout::Title {
                content_bounds.width() * 0.9
            } else {
                content_bounds.width()
            };
            let summary_height = draw_wrapped_text(
                cx,
                summary,
                Bounds::new(content_bounds.min_x(), cursor_y, summary_width, 90.0),
                16.5,
                theme::text::SECONDARY,
                TextAlign::Left,
                false,
            );
            cursor_y += summary_height + 18.0;
        }

        cursor_y
    }

    fn paint_markdown_panel(
        &self,
        slide: &Slide,
        panel_bounds: Bounds,
        origin: Point,
        cx: &mut PaintContext,
    ) {
        let Some(markdown) = slide.markdown() else {
            return;
        };

        let _ = self.markdown_renderer.render(
            &markdown.document,
            origin,
            panel_bounds.width(),
            cx.text,
            cx.scene,
        );
    }
}

fn paint_background_wash(bounds: Bounds, palette: &DeckPalette, cx: &mut PaintContext) {
    let top_wash = Bounds::new(
        bounds.min_x() + bounds.width() * 0.05,
        bounds.min_y() + bounds.height() * 0.08,
        bounds.width() * 0.46,
        bounds.height() * 0.18,
    );
    let side_wash = Bounds::new(
        bounds.max_x() - bounds.width() * 0.34,
        bounds.min_y() + bounds.height() * 0.2,
        bounds.width() * 0.24,
        bounds.height() * 0.44,
    );
    let footer_wash = Bounds::new(
        bounds.min_x() + bounds.width() * 0.16,
        bounds.max_y() - bounds.height() * 0.16,
        bounds.width() * 0.42,
        bounds.height() * 0.08,
    );

    cx.scene.draw_quad(
        Quad::new(top_wash)
            .with_background(palette.wash.with_alpha(0.18))
            .with_corner_radius(36.0),
    );
    cx.scene.draw_quad(
        Quad::new(side_wash)
            .with_background(palette.wash.with_alpha(0.12))
            .with_corner_radius(42.0),
    );
    cx.scene.draw_quad(
        Quad::new(footer_wash)
            .with_background(palette.wash.with_alpha(0.08))
            .with_corner_radius(28.0),
    );
}

fn is_minimal_title_slide(slide: &Slide) -> bool {
    slide.layout == SlideLayout::Title
        && slide.eyebrow.is_none()
        && slide.diagram.is_none()
        && slide
            .markdown()
            .map(|markdown| markdown.markdown.trim().is_empty())
            .unwrap_or(true)
}

fn content_bounds(bounds: Bounds) -> Bounds {
    let inset_x = (bounds.width() * 0.08).clamp(28.0, 88.0);
    let inset_y = (bounds.height() * 0.1).clamp(22.0, 72.0);
    Bounds::new(
        bounds.min_x() + inset_x,
        bounds.min_y() + inset_y,
        (bounds.width() - inset_x * 2.0).max(320.0),
        (bounds.height() - inset_y * 2.0).max(220.0),
    )
}

fn draw_text(cx: &mut PaintContext, text: &str, origin: Point, size: f32, color: Hsla) {
    let run = cx.text.layout(text, origin, size, color);
    cx.scene.draw_text(run);
}

fn draw_mono_text(cx: &mut PaintContext, text: &str, origin: Point, size: f32, color: Hsla) {
    let run = cx.text.layout_mono(text, origin, size, color);
    cx.scene.draw_text(run);
}

fn draw_panel(cx: &mut PaintContext, bounds: Bounds, bg: Hsla, border: Hsla, radius: f32) {
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(bg)
            .with_border(border, 1.0)
            .with_corner_radius(radius),
    );
}

fn draw_wrapped_text(
    cx: &mut PaintContext,
    text: &str,
    bounds: Bounds,
    size: f32,
    color: Hsla,
    align: TextAlign,
    mono: bool,
) -> f32 {
    if text.trim().is_empty() {
        return 0.0;
    }

    let line_height = size * 1.25;
    let lines = wrap_text_lines(cx, text, bounds.width(), size, color, mono);
    let mut y = bounds.min_y();
    for line in &lines {
        let width = measure_text_width(cx, line, size, color, mono);
        let x = match align {
            TextAlign::Left => bounds.min_x(),
            TextAlign::Center => bounds.min_x() + (bounds.width() - width).max(0.0) * 0.5,
        };
        if mono {
            draw_mono_text(cx, line, Point::new(x, y), size, color);
        } else {
            draw_text(cx, line, Point::new(x, y), size, color);
        }
        y += line_height;
    }

    (lines.len() as f32) * line_height
}

fn wrap_text_lines(
    cx: &mut PaintContext,
    text: &str,
    max_width: f32,
    size: f32,
    color: Hsla,
    mono: bool,
) -> Vec<String> {
    let mut lines = Vec::new();

    for paragraph in text.lines() {
        let paragraph = paragraph.trim();
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if current.is_empty() {
                word.to_string()
            } else {
                format!("{current} {word}")
            };
            if !current.is_empty()
                && measure_text_width(cx, &candidate, size, color, mono) > max_width
            {
                lines.push(current);
                current = word.to_string();
            } else {
                current = candidate;
            }
        }

        if !current.is_empty() {
            lines.push(current);
        }
    }

    lines
}

fn measure_text_width(
    cx: &mut PaintContext,
    text: &str,
    size: f32,
    color: Hsla,
    mono: bool,
) -> f32 {
    let run = if mono {
        cx.text.layout_mono(text, Point::ZERO, size, color)
    } else {
        cx.text.layout(text, Point::ZERO, size, color)
    };
    run.bounds().size.width
}

#[allow(dead_code)]
fn paint_market_map(bounds: Bounds, palette: &DeckPalette, cx: &mut PaintContext) {
    let inner = bounds.inset(6.0);
    let autopilot_height = (inner.height() * 0.18).clamp(40.0, 56.0);
    let market_height = (inner.height() * 0.16).clamp(34.0, 46.0);
    let kernel_height = (inner.height() * 0.24).clamp(52.0, 68.0);
    let free = (inner.height() - autopilot_height - market_height - kernel_height).max(18.0);
    let top_gap = free * 0.18;
    let gap_one = free * 0.28;
    let gap_two = free * 0.54;
    let autopilot_bounds = Bounds::new(
        inner.min_x() + inner.width() * 0.28,
        inner.min_y() + top_gap,
        inner.width() * 0.44,
        autopilot_height,
    );
    let market_y = autopilot_bounds.max_y() + gap_one;
    let gap = 10.0;
    let card_width = ((inner.width() - gap * 4.0) / 5.0).max(58.0);
    let kernel_bounds = Bounds::new(
        inner.min_x() + 8.0,
        market_y + market_height + gap_two,
        inner.width() - 16.0,
        kernel_height,
    );

    draw_flow_card(
        cx,
        autopilot_bounds,
        "Autopilot",
        Some("agent / wallet / runtime"),
        theme::accent::PRIMARY,
    );

    let market_labels = [
        ("Compute", market_color("compute")),
        ("Data", market_color("data")),
        ("Labor", market_color("labor")),
        ("Liquidity", market_color("liquidity")),
        ("Risk", market_color("risk")),
    ];

    let row_start_x = inner.min_x();
    let row_end_x = row_start_x + card_width * 5.0 + gap * 4.0;
    draw_v_connector(
        cx,
        center_x(autopilot_bounds),
        autopilot_bounds.max_y(),
        market_y - 14.0,
        palette.panel_border,
    );
    draw_h_connector(
        cx,
        row_start_x + card_width * 0.5,
        row_end_x - card_width * 0.5,
        market_y - 14.0,
        palette.panel_border,
    );

    for (index, (label, color)) in market_labels.iter().enumerate() {
        let x = inner.min_x() + (card_width + gap) * index as f32;
        let card_bounds = Bounds::new(x, market_y, card_width, market_height);
        draw_v_connector(
            cx,
            center_x(card_bounds),
            market_y - 14.0,
            card_bounds.min_y(),
            palette.panel_border,
        );
        draw_v_connector(
            cx,
            center_x(card_bounds),
            card_bounds.max_y(),
            kernel_bounds.min_y() - 12.0,
            color.with_alpha(0.7),
        );
        draw_flow_card(cx, card_bounds, label, None, *color);
    }

    draw_flow_card(
        cx,
        kernel_bounds,
        "Economy Kernel",
        Some("contracts / verification / liability / settlement / policy / receipts"),
        palette.accent,
    );
}

#[allow(dead_code)]
fn paint_compute_flow(bounds: Bounds, palette: &DeckPalette, cx: &mut PaintContext) {
    let inner = bounds.inset(8.0);
    let top_height = (inner.height() * 0.28).clamp(52.0, 74.0);
    let kernel_height = (inner.height() * 0.22).clamp(48.0, 64.0);
    let free = (inner.height() - top_height - kernel_height).max(20.0);
    let top_y = inner.min_y() + free * 0.22;
    let kernel_y = top_y + top_height + free * 0.78;
    let left = Bounds::new(inner.min_x() + 6.0, top_y, inner.width() * 0.23, top_height);
    let center = Bounds::new(
        inner.min_x() + inner.width() * 0.31,
        top_y - top_height * 0.08,
        inner.width() * 0.38,
        top_height * 1.12,
    );
    let right = Bounds::new(
        inner.max_x() - inner.width() * 0.23 - 6.0,
        top_y,
        inner.width() * 0.23,
        top_height,
    );
    let kernel = Bounds::new(
        inner.min_x() + inner.width() * 0.18,
        kernel_y,
        inner.width() * 0.64,
        kernel_height,
    );

    draw_h_connector(
        cx,
        center_x(left),
        center.min_x(),
        center_y(left),
        palette.panel_border,
    );
    draw_h_connector(
        cx,
        center.max_x(),
        center_x(right),
        center_y(right),
        palette.panel_border,
    );
    draw_v_connector(
        cx,
        center_x(center),
        center.max_y(),
        kernel.min_y() - 10.0,
        palette.panel_border,
    );

    draw_flow_card(
        cx,
        left,
        "Buyer",
        Some("capacity demand"),
        theme::accent::BLUE,
    );
    draw_flow_card(
        cx,
        center,
        "Compute Market",
        Some("pricing / matching / delivery proofs"),
        theme::accent::BLUE,
    );
    draw_flow_card(
        cx,
        right,
        "Provider",
        Some("spare CPU / GPU"),
        theme::accent::BLUE,
    );
    draw_flow_card(
        cx,
        kernel,
        "Kernel receipts + settlement",
        Some("the MVP makes earning legible"),
        palette.accent,
    );
}

#[allow(dead_code)]
fn paint_access_grant(bounds: Bounds, palette: &DeckPalette, cx: &mut PaintContext) {
    let inner = bounds.inset(8.0);
    let top_height = (inner.height() * 0.28).clamp(52.0, 72.0);
    let kernel_height = (inner.height() * 0.22).clamp(48.0, 64.0);
    let free = (inner.height() - top_height - kernel_height).max(20.0);
    let top_y = inner.min_y() + free * 0.22;
    let kernel_y = top_y + top_height + free * 0.78;
    let card_w = inner.width() * 0.25;
    let left = Bounds::new(inner.min_x() + 2.0, top_y, card_w, top_height);
    let center = Bounds::new(
        inner.min_x() + inner.width() * 0.37,
        top_y,
        card_w,
        top_height,
    );
    let right = Bounds::new(inner.max_x() - card_w - 2.0, top_y, card_w, top_height);
    let kernel = Bounds::new(
        inner.min_x() + inner.width() * 0.28,
        kernel_y,
        inner.width() * 0.44,
        kernel_height,
    );

    draw_h_connector(
        cx,
        center_x(left),
        center.min_x(),
        center_y(left),
        palette.panel_border,
    );
    draw_h_connector(
        cx,
        center.max_x(),
        center_x(right),
        center_y(right),
        palette.panel_border,
    );
    draw_v_connector(
        cx,
        center_x(center),
        center.max_y(),
        kernel.min_y() - 12.0,
        palette.panel_border,
    );

    draw_flow_card(
        cx,
        left,
        "Data Asset",
        Some("context / artifacts"),
        market_color("data"),
    );
    draw_flow_card(
        cx,
        center,
        "Access Grant",
        Some("policy / permission / revocation"),
        market_color("data"),
    );
    draw_flow_card(
        cx,
        right,
        "Agent / Work",
        Some("controlled context"),
        market_color("data"),
    );
    draw_flow_card(
        cx,
        kernel,
        "Kernel policy + receipts",
        Some("explicit access should stay explicit"),
        palette.accent,
    );
}

#[allow(dead_code)]
fn paint_contract_chain(bounds: Bounds, palette: &DeckPalette, cx: &mut PaintContext) {
    let inner = bounds.inset(8.0);
    let card_height = (inner.height() * 0.28).clamp(48.0, 64.0);
    let footer_height = (inner.height() * 0.24).clamp(50.0, 64.0);
    let free = (inner.height() - card_height - footer_height).max(18.0);
    let y = inner.min_y() + free * 0.26;
    let footer_y = y + card_height + free * 0.74;
    let gap = 10.0;
    let card_width = ((inner.width() - gap * 4.0) / 5.0).max(54.0);
    let labels = [
        "WorkUnit",
        "Contract",
        "Submission",
        "Verdict",
        "Settlement",
    ];
    let accent = market_color("labor");

    for (index, label) in labels.iter().enumerate() {
        let x = inner.min_x() + (card_width + gap) * index as f32;
        let card_bounds = Bounds::new(x, y, card_width, card_height);
        if index > 0 {
            let prev_x = inner.min_x() + (card_width + gap) * (index as f32 - 1.0);
            draw_h_connector(
                cx,
                prev_x + card_width,
                x,
                y + card_height * 0.5,
                accent.with_alpha(0.75),
            );
        }
        draw_flow_card(cx, card_bounds, label, None, accent);
    }

    let footer = Bounds::new(
        inner.min_x() + 18.0,
        footer_y,
        inner.width() - 36.0,
        footer_height,
    );
    draw_flow_card(
        cx,
        footer,
        "Verified outcomes unlock settlement",
        Some("define work / check work / assign responsibility / move money"),
        palette.accent,
    );
}

#[allow(dead_code)]
fn paint_liquidity_route(bounds: Bounds, palette: &DeckPalette, cx: &mut PaintContext) {
    let inner = bounds.inset(8.0);
    let card_height = (inner.height() * 0.28).clamp(52.0, 68.0);
    let footer_height = (inner.height() * 0.22).clamp(48.0, 60.0);
    let free = (inner.height() - card_height - footer_height).max(20.0);
    let y = inner.min_y() + free * 0.28;
    let footer_y = y + card_height + free * 0.72;
    let gap = 12.0;
    let card_width = ((inner.width() - gap * 3.0) / 4.0).max(62.0);
    let labels = [
        ("Intent", Some("needs value movement")),
        ("Quote / Route", Some("price + path")),
        ("Envelope", Some("bounded credit")),
        ("Settlement", Some("proofs on rails")),
    ];
    let accent = market_color("liquidity");

    for (index, (label, detail)) in labels.iter().enumerate() {
        let x = inner.min_x() + (card_width + gap) * index as f32;
        let card_bounds = Bounds::new(x, y, card_width, card_height);
        if index > 0 {
            let prev_x = inner.min_x() + (card_width + gap) * (index as f32 - 1.0);
            draw_h_connector(
                cx,
                prev_x + card_width,
                x,
                y + card_height * 0.5,
                accent.with_alpha(0.78),
            );
        }
        draw_flow_card(cx, card_bounds, label, *detail, accent);
    }

    let footer = Bounds::new(
        inner.min_x() + inner.width() * 0.28,
        footer_y,
        inner.width() * 0.44,
        footer_height,
    );
    draw_flow_card(
        cx,
        footer,
        "Kernel receipts",
        Some("bounded money movement, not hidden leverage"),
        palette.accent,
    );
}

#[allow(dead_code)]
fn paint_risk_loop(bounds: Bounds, palette: &DeckPalette, cx: &mut PaintContext) {
    let inner = bounds.inset(8.0);
    let top_height = (inner.height() * 0.28).clamp(52.0, 72.0);
    let bottom_height = (inner.height() * 0.26).clamp(54.0, 72.0);
    let free = (inner.height() - top_height - bottom_height).max(20.0);
    let top_y = inner.min_y() + free * 0.24;
    let bottom_y = top_y + top_height + free * 0.76;
    let gap = 12.0;
    let card_width = ((inner.width() - gap * 2.0) / 3.0).max(72.0);
    let labels = [
        ("Prediction", Some("signals")),
        ("Coverage", Some("backstops")),
        ("Underwriting", Some("capital")),
    ];
    let accent = market_color("risk");

    for (index, (label, detail)) in labels.iter().enumerate() {
        let x = inner.min_x() + (card_width + gap) * index as f32;
        let card_bounds = Bounds::new(x, top_y, card_width, top_height);
        draw_flow_card(cx, card_bounds, label, *detail, accent);
        draw_v_connector(
            cx,
            center_x(card_bounds),
            card_bounds.max_y(),
            bottom_y - 10.0,
            accent.with_alpha(0.75),
        );
    }

    let bottom = Bounds::new(
        inner.min_x() + inner.width() * 0.18,
        bottom_y,
        inner.width() * 0.64,
        bottom_height,
    );
    draw_flow_card(
        cx,
        bottom,
        "Verification + policy + autonomy",
        Some("priced uncertainty feeds the control loop"),
        palette.accent,
    );
}

#[allow(dead_code)]
fn draw_flow_card(
    cx: &mut PaintContext,
    bounds: Bounds,
    title: &str,
    detail: Option<&str>,
    accent: Hsla,
) {
    let title_size = (bounds.height() * 0.24).clamp(12.0, 15.0);
    let detail_size = (bounds.height() * 0.16).clamp(9.5, 11.0);

    draw_panel(
        cx,
        bounds,
        accent.with_alpha(0.12),
        accent.with_alpha(0.7),
        14.0,
    );
    cx.scene.draw_quad(
        Quad::new(Bounds::new(
            bounds.min_x() + 14.0,
            bounds.min_y() + 12.0,
            (bounds.width() - 28.0).max(18.0),
            2.0,
        ))
        .with_background(accent.with_alpha(0.8))
        .with_corner_radius(1.0),
    );

    let title_height = draw_wrapped_text(
        cx,
        title,
        Bounds::new(
            bounds.min_x() + 12.0,
            bounds.min_y() + 22.0,
            (bounds.width() - 24.0).max(16.0),
            bounds.height() * 0.45,
        ),
        title_size,
        theme::text::PRIMARY,
        TextAlign::Center,
        false,
    );

    if let Some(detail) = detail {
        let _ = draw_wrapped_text(
            cx,
            detail,
            Bounds::new(
                bounds.min_x() + 12.0,
                bounds.min_y() + 28.0 + title_height,
                (bounds.width() - 24.0).max(16.0),
                bounds.height() - 34.0 - title_height,
            ),
            detail_size,
            theme::text::SECONDARY,
            TextAlign::Center,
            false,
        );
    }
}

#[allow(dead_code)]
fn draw_h_connector(cx: &mut PaintContext, x1: f32, x2: f32, y: f32, color: Hsla) {
    let left = x1.min(x2);
    let width = (x2 - x1).abs().max(2.0);
    cx.scene.draw_quad(
        Quad::new(Bounds::new(left, y - 1.0, width, 2.0))
            .with_background(color)
            .with_corner_radius(1.0),
    );
}

#[allow(dead_code)]
fn draw_v_connector(cx: &mut PaintContext, x: f32, y1: f32, y2: f32, color: Hsla) {
    let top = y1.min(y2);
    let height = (y2 - y1).abs().max(2.0);
    cx.scene.draw_quad(
        Quad::new(Bounds::new(x - 1.0, top, 2.0, height))
            .with_background(color)
            .with_corner_radius(1.0),
    );
}

#[allow(dead_code)]
fn center_x(bounds: Bounds) -> f32 {
    bounds.min_x() + bounds.width() * 0.5
}

#[allow(dead_code)]
fn center_y(bounds: Bounds) -> f32 {
    bounds.min_y() + bounds.height() * 0.5
}

#[derive(Clone, Copy)]
struct DeckPalette {
    grid: Hsla,
    accent: Hsla,
    wash: Hsla,
    panel_bg: Hsla,
    panel_border: Hsla,
}

fn palette_for_theme(theme: &DeckTheme) -> DeckPalette {
    let accent = match theme {
        DeckTheme::Minimal => Hsla::from_hex(0xD9D4C7),
        DeckTheme::Code => Hsla::from_hex(0x727C8D),
        DeckTheme::Diagram => Hsla::from_hex(0x8B866F),
        DeckTheme::Hud => Hsla::from_hex(0xC7C1B3),
        DeckTheme::Custom(label) => market_color(label),
    };

    DeckPalette {
        grid: Hsla::from_hex(0x3A3936).with_alpha(0.22),
        accent,
        wash: Hsla::from_hex(0x1A1713),
        panel_bg: Hsla::from_hex(0x090909).with_alpha(0.94),
        panel_border: accent.with_alpha(0.24),
    }
}

fn market_color(label: &str) -> Hsla {
    match label.trim().to_ascii_lowercase().as_str() {
        "compute" => Hsla::from_hex(0x7B8594),
        "data" => Hsla::from_hex(0x9B917E),
        "labor" => Hsla::from_hex(0x7D8662),
        "liquidity" => Hsla::from_hex(0xA77437),
        "risk" => Hsla::from_hex(0x9C5549),
        _ => Hsla::from_hex(0xC7C1B3),
    }
}

#[derive(Clone, Copy)]
enum ClickTarget {
    Previous,
    Next,
}

#[derive(Clone, Copy)]
enum TextAlign {
    Left,
    Center,
}

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use crate::input;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;
#[cfg(target_arch = "wasm32")]
use wgpui::Scene;
#[cfg(target_arch = "wasm32")]
use wgpui::{Platform, WebPlatform, run_animation_loop, setup_resize_observer};

#[cfg(target_arch = "wasm32")]
pub(crate) async fn boot_browser_app(deck: Deck) -> Result<(), String> {
    let platform = WebPlatform::init("deck-canvas").await?;
    let canvas = platform.canvas().clone();
    let runtime = Rc::new(RefCell::new(BrowserRuntime::new(platform, deck)?));

    setup_resize_observer(&canvas, {
        let runtime = runtime.clone();
        move || {
            runtime.borrow_mut().handle_resize();
        }
    });

    input::install_input_bridge(
        canvas,
        Rc::new({
            let runtime = runtime.clone();
            move |event| {
                runtime.borrow_mut().handle_input(event);
            }
        }),
    )?;

    input::clear_boot_status();
    runtime.borrow_mut().request_redraw();
    runtime.borrow_mut().render_if_needed()?;

    run_animation_loop(move || {
        if let Err(error) = runtime.borrow_mut().render_if_needed() {
            web_sys::console::error_1(&JsValue::from_str(&error));
        }
    });

    Ok(())
}

#[cfg(target_arch = "wasm32")]
struct BrowserRuntime {
    app: DeckApp,
    platform: WebPlatform,
    pointer: Point,
}

#[cfg(target_arch = "wasm32")]
impl BrowserRuntime {
    fn new(platform: WebPlatform, deck: Deck) -> Result<Self, String> {
        Ok(Self {
            app: DeckApp::new(deck)?,
            platform,
            pointer: Point::ZERO,
        })
    }

    fn handle_input(&mut self, event: InputEvent) {
        if let InputEvent::MouseMove { x, y }
        | InputEvent::MouseDown { x, y, .. }
        | InputEvent::MouseUp { x, y, .. } = &event
        {
            self.pointer = Point::new(*x, *y);
        }

        let viewport = self.viewport();
        let _ = self.app.handle_input(&event, viewport);
        self.refresh_cursor();
    }

    fn handle_resize(&mut self) {
        self.platform.handle_resize();
        self.app.request_redraw();
        self.refresh_cursor();
    }

    fn request_redraw(&mut self) {
        self.app.request_redraw();
    }

    fn render_if_needed(&mut self) -> Result<(), String> {
        if !self.app.take_redraw_request() {
            return Ok(());
        }

        let viewport = self.viewport();
        let mut scene = Scene::new();
        let scale_factor = self.platform.scale_factor();
        {
            let text_system = self.platform.text_system();
            let mut cx = PaintContext::new(&mut scene, text_system, scale_factor);
            self.app.paint(viewport, &mut cx);
        }

        self.platform.render(&scene)
    }

    fn refresh_cursor(&self) {
        let cursor = self.app.cursor_for_point(self.pointer, self.viewport());
        self.platform.set_cursor(cursor);
    }

    fn viewport(&self) -> Bounds {
        let logical_size = self.platform.logical_size();
        Bounds::new(0.0, 0.0, logical_size.width, logical_size.height)
    }
}
