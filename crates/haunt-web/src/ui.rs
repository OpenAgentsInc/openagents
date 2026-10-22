//! The words over the view, drawn into an offscreen 2D canvas that the
//! shell lays over the frame as a texture: no page elements. The title
//! card, a hint line, a short note when something changes (a volume, the
//! gentle setting, a new version), the menu, the ending card, and the
//! touch joystick.

use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

const PAPER: &str = "#F1E4C8";
const LICORICE: &str = "#1B1714";
const PUMPKIN: &str = "#E07A2E";
const PLUM: &str = "#4A3F5E";
const MUSTARD: &str = "#C99A3B";

/// A clickable spot on the UI, in CSS pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    pub action: &'static str,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Button {
    #[must_use]
    pub fn hit(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.w && y >= self.y && y <= self.y + self.h
    }
}

/// What the UI shows this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    pub title: Option<(String, String)>,
    pub hint: String,
    pub note: String,
    pub menu: Option<Vec<(String, &'static str)>>,
    pub ending: Option<(String, Vec<(String, &'static str)>)>,
    /// The joystick's centre and the thumb, CSS pixels.
    pub stick: Option<([f64; 2], [f64; 2])>,
    pub mic: Option<bool>,
    pub loading: Option<String>,
}

pub struct Ui {
    pub canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    shown: Option<View>,
    size: (u32, u32),
    pub buttons: Vec<Button>,
    /// The canvas changed since the last upload.
    pub dirty: bool,
}

impl Ui {
    pub fn new(document: &web_sys::Document) -> Option<Self> {
        let canvas: HtmlCanvasElement = document.create_element("canvas").ok()?.dyn_into().ok()?;
        let ctx: CanvasRenderingContext2d = canvas.get_context("2d").ok()??.dyn_into().ok()?;
        Some(Self {
            canvas,
            ctx,
            shown: None,
            size: (0, 0),
            buttons: Vec::new(),
            dirty: true,
        })
    }

    fn text(&self, text: &str, x: f64, y: f64, size: f64, fill: &str, weight: &str) {
        let ctx = &self.ctx;
        ctx.set_font(&format!(
            "{weight} {size}px 'Trebuchet MS', 'Arial Rounded MT Bold', ui-rounded, system-ui, sans-serif"
        ));
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");
        ctx.set_line_join("round");
        ctx.set_line_width(size * 0.18);
        ctx.set_stroke_style_str(LICORICE);
        let _ = ctx.stroke_text(text, x, y);
        ctx.set_fill_style_str(fill);
        let _ = ctx.fill_text(text, x, y);
    }

    fn panel(&self, x: f64, y: f64, w: f64, h: f64) {
        let ctx = &self.ctx;
        ctx.set_fill_style_str(PAPER);
        ctx.set_stroke_style_str(LICORICE);
        ctx.set_line_width(5.0);
        ctx.begin_path();
        ctx.rect(x, y, w, h);
        ctx.fill();
        ctx.stroke();
        ctx.set_stroke_style_str(PUMPKIN);
        ctx.set_line_width(2.0);
        ctx.stroke_rect(x + 8.0, y + 8.0, w - 16.0, h - 16.0);
    }

    fn button(&mut self, label: &str, action: &'static str, cx: f64, y: f64, w: f64) {
        let h = 44.0;
        let x = cx - w / 2.0;
        let ctx = &self.ctx;
        ctx.set_fill_style_str(PUMPKIN);
        ctx.set_stroke_style_str(LICORICE);
        ctx.set_line_width(4.0);
        ctx.begin_path();
        ctx.rect(x, y, w, h);
        ctx.fill();
        ctx.stroke();
        self.ctx
            .set_font("800 19px 'Trebuchet MS', ui-rounded, system-ui, sans-serif");
        self.ctx.set_text_align("center");
        self.ctx.set_text_baseline("middle");
        self.ctx.set_fill_style_str(LICORICE);
        let _ = self.ctx.fill_text(label, cx, y + h / 2.0);
        self.buttons.push(Button { action, x, y, w, h });
    }

    /// Redraws when `view` or the size changed. `width` and `height` are
    /// CSS pixels; `scale` is the device pixel ratio.
    pub fn draw(&mut self, view: &View, width: f64, height: f64, scale: f64) {
        let size = ((width * scale) as u32, (height * scale) as u32);
        if self.shown.as_ref() == Some(view) && self.size == size {
            return;
        }
        self.size = size;
        self.shown = Some(view.clone());
        self.dirty = true;
        self.buttons.clear();
        self.canvas.set_width(size.0.max(1));
        self.canvas.set_height(size.1.max(1));
        let ctx = self.ctx.clone();
        let _ = ctx.set_transform(scale, 0.0, 0.0, scale, 0.0, 0.0);
        ctx.clear_rect(0.0, 0.0, width, height);
        let cx = width / 2.0;
        let big = (width.min(height) / 9.0).clamp(30.0, 64.0);
        if let Some(loading) = &view.loading {
            self.text(loading, cx, height / 2.0, 22.0, PAPER, "700");
        }
        if let Some((title, line)) = &view.title {
            ctx.set_fill_style_str("rgba(27,23,20,0.55)");
            ctx.fill_rect(0.0, 0.0, width, height);
            self.text(title, cx, height * 0.4, big, PUMPKIN, "900");
            self.text(line, cx, height * 0.4 + big * 1.1, 21.0, PAPER, "700");
        }
        if !view.hint.is_empty() {
            self.text(&view.hint, cx, height - 28.0, 16.0, PAPER, "700");
        }
        if !view.note.is_empty() {
            self.text(&view.note, cx, 38.0, 20.0, MUSTARD, "800");
        }
        if let Some(items) = &view.menu {
            let w = 340.0_f64.min(width - 32.0);
            let h = 96.0 + items.len() as f64 * 56.0;
            let y = (height - h) / 2.0;
            self.panel(cx - w / 2.0, y, w, h);
            self.text("Paused", cx, y + 44.0, 30.0, PUMPKIN, "900");
            for (i, (label, action)) in items.iter().enumerate() {
                self.button(label, action, cx, y + 80.0 + i as f64 * 56.0, w - 60.0);
            }
        }
        if let Some((line, items)) = &view.ending {
            ctx.set_fill_style_str("rgba(74,63,94,0.6)");
            ctx.fill_rect(0.0, 0.0, width, height);
            let w = 380.0_f64.min(width - 32.0);
            let h = 120.0 + items.len() as f64 * 56.0;
            let y = (height - h) / 2.0;
            self.panel(cx - w / 2.0, y, w, h);
            self.text(line, cx, y + 52.0, 28.0, PUMPKIN, "900");
            for (i, (label, action)) in items.iter().enumerate() {
                self.button(label, action, cx, y + 96.0 + i as f64 * 56.0, w - 60.0);
            }
        }
        if let Some((centre, thumb)) = view.stick {
            ctx.set_stroke_style_str(PAPER);
            ctx.set_line_width(3.0);
            ctx.begin_path();
            let _ = ctx.arc(centre[0], centre[1], 52.0, 0.0, std::f64::consts::TAU);
            ctx.stroke();
            ctx.set_fill_style_str("rgba(241,228,200,0.6)");
            ctx.begin_path();
            let _ = ctx.arc(thumb[0], thumb[1], 24.0, 0.0, std::f64::consts::TAU);
            ctx.fill();
        }
        if let Some(listening) = view.mic {
            // The mic button, bottom right: it sends your words to the
            // house's chat.
            let (x, y, r) = (width - 44.0, height - 44.0, 26.0);
            ctx.set_fill_style_str(if listening { PUMPKIN } else { PLUM });
            ctx.set_stroke_style_str(LICORICE);
            ctx.set_line_width(4.0);
            ctx.begin_path();
            let _ = ctx.arc(x, y, r, 0.0, std::f64::consts::TAU);
            ctx.fill();
            ctx.stroke();
            ctx.set_fill_style_str(PAPER);
            ctx.fill_rect(x - 6.0, y - 13.0, 12.0, 18.0);
            ctx.fill_rect(x - 1.5, y + 5.0, 3.0, 8.0);
            self.buttons.push(Button {
                action: "mic",
                x: x - r,
                y: y - r,
                w: r * 2.0,
                h: r * 2.0,
            });
        }
    }
}
