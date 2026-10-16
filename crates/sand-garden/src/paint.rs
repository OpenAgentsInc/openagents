//! Canvas drawing in the studio's warm sand, sea glass, and ink palette.
use crate::{Action, Drawing, HEIGHT, Light, Point, Tool, WIDTH};
use wasm_bindgen::{Clamped, JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d as Context, HtmlCanvasElement, ImageData};

pub fn canvas() -> Result<(HtmlCanvasElement, Context), JsValue> {
    let c: HtmlCanvasElement = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .create_element("canvas")?
        .dyn_into()?;
    c.set_width(WIDTH as u32);
    c.set_height(HEIGHT as u32);
    let ctx = c.get_context("2d")?.unwrap().dyn_into()?;
    Ok((c, ctx))
}
pub struct Painter {
    pub ctx: Context,
    sand: std::cell::RefCell<crate::sand::Sand>,
    bed: HtmlCanvasElement,
    bed_ctx: Context,
    objects: std::cell::RefCell<Vec<Action>>,
    light: std::cell::Cell<Light>,
    cache: HtmlCanvasElement,
    cache_ctx: Context,
}
impl Painter {
    pub fn new(ctx: Context) -> Result<Self, JsValue> {
        let (bed, bed_ctx) = canvas()?;
        let (cache, cache_ctx) = canvas()?;
        let p = Self {
            ctx,
            bed,
            bed_ctx,
            sand: std::cell::RefCell::new(crate::sand::Sand::new()),
            objects: std::cell::RefCell::new(Vec::new()),
            light: std::cell::Cell::new(Light::Sun),
            cache,
            cache_ctx,
        };
        p.flush((0, 0, WIDTH as usize, HEIGHT as usize));
        Ok(p)
    }
    fn flush(&self, r: (usize, usize, usize, usize)) {
        let mut sand = self.sand.borrow_mut();
        sand.shade(r);
        // A dirty rectangle updates only the touched grains on the cached bed.
        let width = r.2 - r.0;
        let height = r.3 - r.1;
        let mut patch = Vec::with_capacity(width * height * 4);
        for y in r.1..r.3 {
            patch.extend_from_slice(
                &sand.pixels[(y * WIDTH as usize + r.0) * 4..(y * WIDTH as usize + r.2) * 4],
            );
        }
        if width > 0 && height > 0 {
            if let Ok(image) = ImageData::new_with_u8_clamped_array_and_sh(
                Clamped(&patch),
                width as u32,
                height as u32,
            ) {
                let _ = self.bed_ctx.put_image_data(&image, r.0 as f64, r.1 as f64);
            }
        }
        drop(sand);
        self.refresh();
    }
    fn refresh(&self) {
        let c = &self.ctx;
        let _ = c.draw_image_with_html_canvas_element(&self.bed, 0., 0.);
        for a in self.objects.borrow().iter() {
            self.object(a);
        }
        c.save();
        if self.light.get() == Light::Shade {
            c.set_fill_style_str("rgba(17,53,78,0.23)");
            c.fill_rect(0., 0., WIDTH, HEIGHT);
            c.set_stroke_style_str("rgba(11,36,57,0.09)");
            c.set_line_cap("round");
            for k in 0..7 {
                let y = -80. + k as f64 * 78.;
                c.set_line_width(18. + k as f64 * 3.);
                c.begin_path();
                c.move_to(WIDTH + 60., y);
                c.quadratic_curve_to(
                    WIDTH - 140.,
                    y + 60.,
                    WIDTH - 240. - k as f64 * 32.,
                    y + 140.,
                );
                c.stroke();
            }
        } else if let Ok(g) = c.create_radial_gradient(80., 40., 0., 80., 40., 1000.) {
            let _ = g.add_color_stop(0., "rgba(255,240,186,0.14)");
            let _ = g.add_color_stop(1., "rgba(255,240,186,0)");
            c.set_fill_style_canvas_gradient(&g);
            c.fill_rect(0., 0., WIDTH, HEIGHT);
        }
        c.restore();
        if let Some(canvas) = c.canvas() {
            let _ = self
                .cache_ctx
                .draw_image_with_html_canvas_element(&canvas, 0., 0.);
        }
    }
    pub fn update_objects(&self, d: &Drawing) {
        *self.objects.borrow_mut() = d
            .actions
            .iter()
            .filter(|a| a.tool.placeable())
            .cloned()
            .collect();
        self.light.set(d.light);
        self.refresh();
    }
    pub fn water_ratio(&self) -> f64 {
        self.sand.borrow().water_ratio()
    }
    pub fn is_water(&self, p: Point) -> bool {
        self.sand.borrow().is_water(p)
    }
    pub fn present(&self, guests: &[crate::visitors::Guest], selected: Option<&Action>) {
        let c = &self.ctx;
        let _ = c.draw_image_with_html_canvas_element(&self.cache, 0., 0.);
        for guest in guests {
            crate::creatures::draw(c, guest);
        }
        if let Some(a) = selected {
            if let Some(p) = a.points.first() {
                c.save();
                c.set_stroke_style_str("rgba(30,67,76,0.8)");
                c.set_line_width(1.5);
                let _ = c.set_line_dash(&js_sys::Array::of2(&4.into(), &5.into()));
                c.begin_path();
                let _ = c.arc(p.x, p.y, 36. * a.size, 0., std::f64::consts::TAU);
                c.stroke();
                c.restore();
            }
        }
    }
    pub fn redraw(&self, d: &Drawing) {
        self.light.set(d.light);
        self.sand.borrow_mut().clear();
        self.objects.borrow_mut().clear();
        for a in &d.actions {
            if a.tool.placeable() {
                self.objects.borrow_mut().push(a.clone());
                continue;
            }
            self.segment(a, a.points[0], a.points[0], (0., 1.), (0., 1.));
            let mut n = None;
            for p in a.points.windows(2) {
                let (old, next) = normals(p[0], p[1], n);
                self.segment(a, p[0], p[1], old, next);
                n = Some(next);
            }
        }
        self.flush((0, 0, WIDTH as usize, HEIGHT as usize));
    }
    pub fn action(&self, a: &Action) {
        if a.tool.draws() {
            let r = self.segment(a, a.points[0], a.points[0], (0., 1.), (0., 1.));
            self.flush(r);
        } else {
            self.objects.borrow_mut().push(a.clone());
            self.refresh();
        }
    }
    pub fn extend(
        &self,
        a: &Action,
        from: Point,
        to: Point,
        prev: Option<(f64, f64)>,
    ) -> (f64, f64) {
        let (old, n) = normals(from, to, prev);
        let r = self.segment(a, from, to, old, n);
        self.flush(r);
        n
    }
    fn line(&self, a: Point, b: Point, color: &str, width: f64, ox: f64, oy: f64) {
        self.ctx.set_stroke_style_str(color);
        self.ctx.set_line_width(width);
        self.ctx.set_line_cap("round");
        self.ctx.begin_path();
        self.ctx.move_to(a.x + ox, a.y + oy);
        self.ctx.line_to(b.x + ox, b.y + oy);
        self.ctx.stroke();
    }
    fn segment(
        &self,
        a: &Action,
        from: Point,
        to: Point,
        old: (f64, f64),
        n: (f64, f64),
    ) -> (usize, usize, usize, usize) {
        self.sand
            .borrow_mut()
            .stroke(a.tool, a.size, from, to, old, n, a.color.as_deref())
    }
    fn ellipse(&self, x: f64, y: f64, rx: f64, ry: f64, color: &str) {
        self.ctx.begin_path();
        let _ = self
            .ctx
            .ellipse(x, y, rx, ry, 0., 0., std::f64::consts::TAU);
        self.ctx.set_fill_style_str(color);
        self.ctx.fill();
    }
    pub fn object(&self, a: &Action) {
        let Some(p) = a.points.first() else { return };
        let c = &self.ctx;
        let base = a
            .color
            .as_deref()
            .filter(|v| crate::valid_color(v))
            .unwrap_or(a.tool.color());
        let light = tone(base, 0.4);
        let dark = tone(base, -0.4);
        c.save();
        let _ = c.translate(p.x, p.y);
        let _ = c.rotate(a.angle);
        let _ = c.scale(a.size, a.size);
        c.set_shadow_color("rgba(58,53,40,0.28)");
        c.set_shadow_blur(6.);
        c.set_shadow_offset_x(3.);
        c.set_shadow_offset_y(5.);
        c.set_line_join("round");
        c.set_line_width(1.4);
        c.set_stroke_style_str(&dark);
        match a.tool {
            Tool::Stone => {
                self.ellipse(0., 0., 25., 18., base);
                c.set_shadow_color("transparent");
                c.stroke();
                self.ellipse(-5., -5., 16., 9., &light);
                self.line(
                    Point { x: -15., y: 3. },
                    Point { x: 12., y: 7. },
                    "#b8bdb1",
                    1.5,
                    0.,
                    0.,
                );
                self.ellipse(9., -3., 2., 1.2, "#616f76");
            }
            Tool::Glass => {
                c.begin_path();
                c.move_to(-22., 8.);
                c.line_to(-12., -18.);
                c.quadratic_curve_to(-8., -24., -2., -17.);
                c.line_to(22., 6.);
                c.quadratic_curve_to(25., 13., 16., 15.);
                c.line_to(-15., 18.);
                c.close_path();
                c.set_fill_style_str(base);
                c.fill();
                c.set_shadow_color("transparent");
                c.set_stroke_style_str("#4f827f");
                c.stroke();
                self.line(
                    Point { x: -13., y: 5. },
                    Point { x: -6., y: -11. },
                    &light,
                    2.8,
                    0.,
                    0.,
                );
                self.line(
                    Point { x: -11., y: 12. },
                    Point { x: 15., y: 10. },
                    "#93bdad",
                    1.,
                    0.,
                    0.,
                );
            }
            Tool::Tooth => {
                c.begin_path();
                c.move_to(-19., -12.);
                c.quadratic_curve_to(-12., -22., -3., -14.);
                c.quadratic_curve_to(8., -21., 19., -11.);
                c.line_to(12., -4.);
                c.line_to(0., 25.);
                c.line_to(-11., -4.);
                c.close_path();
                c.set_fill_style_str(base);
                c.fill();
                c.set_shadow_color("transparent");
                c.stroke();
                c.begin_path();
                c.move_to(-19., -12.);
                c.quadratic_curve_to(-11., -21., -3., -14.);
                c.quadratic_curve_to(8., -21., 19., -11.);
                c.line_to(11., -5.);
                c.line_to(-11., -5.);
                c.close_path();
                c.set_fill_style_str("#8c8275");
                c.fill();
                self.line(
                    Point { x: 0., y: -3. },
                    Point { x: 0., y: 17. },
                    "#faf0cf",
                    2.,
                    0.,
                    0.,
                );
            }
            Tool::Conch => {
                c.begin_path();
                c.move_to(-29., 8.);
                c.line_to(-20., 0.);
                c.line_to(-18., -9.);
                c.line_to(-11., -9.);
                c.line_to(-5., -19.);
                c.bezier_curve_to(9., -25., 27., -12., 26., 3.);
                c.quadratic_curve_to(21., 21., 4., 20.);
                c.line_to(-6., 14.);
                c.close_path();
                c.set_fill_style_str(base);
                c.fill();
                c.set_shadow_color("transparent");
                c.set_stroke_style_str("#9c7666");
                c.stroke();
                c.begin_path();
                c.move_to(-15., 3.);
                c.bezier_curve_to(-6., -10., 20., -14., 18., 3.);
                c.bezier_curve_to(16., 14., 1., 14., 1., 4.);
                c.bezier_curve_to(1., -2., 11., -2., 10., 5.);
                c.set_line_width(2.);
                c.set_stroke_style_str(&light);
                c.stroke();
                self.ellipse(15., 10., 4.5, 8., &dark);
            }
            Tool::Anemone => {
                self.ellipse(0., 9., 20., 13., &dark);
                c.set_shadow_color("transparent");
                for k in 0..15 {
                    let a = k as f64 * std::f64::consts::TAU / 15.;
                    let (sy, sx) = a.sin_cos();
                    c.begin_path();
                    c.move_to(sx * 5., sy * 5.);
                    c.bezier_curve_to(
                        sx * 22. - sy * 7.,
                        sy * 22. + sx * 7.,
                        sx * 20. + sy * 9.,
                        sy * 20. - sx * 9.,
                        sx * 29.,
                        sy * 29.,
                    );
                    c.set_stroke_style_str(&dark);
                    c.set_line_cap("round");
                    c.set_line_width(6.);
                    c.stroke();
                    c.set_stroke_style_str(base);
                    c.set_line_width(4.);
                    c.stroke();
                    self.ellipse(sx * 29., sy * 29., 2.5, 2.5, &light);
                }
                self.ellipse(0., 0., 10., 10., base);
                self.ellipse(0., 0., 5., 5., "#e8dba6");
                self.ellipse(0., 0., 2., 2., &dark);
            }
            _ => {}
        }
        c.restore();
    }
}

fn normals(from: Point, to: Point, prev: Option<(f64, f64)>) -> ((f64, f64), (f64, f64)) {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let len = dx.hypot(dy).max(0.001);
    let next = (-dy / len, dx / len);
    let old = prev.unwrap_or(next);
    let blend = (old.0 * 0.72 + next.0 * 0.28, old.1 * 0.72 + next.1 * 0.28);
    let l = blend.0.hypot(blend.1).max(0.001);
    (old, (blend.0 / l, blend.1 / l))
}

fn tone(c: &str, t: f64) -> String {
    let target = if t >= 0. {
        [255., 248., 229.]
    } else {
        [23., 48., 61.]
    };
    let rgb = [
        u8::from_str_radix(&c[1..3], 16).unwrap(),
        u8::from_str_radix(&c[3..5], 16).unwrap(),
        u8::from_str_radix(&c[5..7], 16).unwrap(),
    ];
    let mut out = [0u8; 3];
    for k in 0..3 {
        out[k] = (rgb[k] as f64 + (target[k] - rgb[k] as f64) * t.abs()) as u8;
    }
    format!("#{:02x}{:02x}{:02x}", out[0], out[1], out[2])
}
