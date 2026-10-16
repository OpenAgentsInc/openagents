//! Small illustrated guests, drawn in the beach finds' outline style.
use crate::visitors::{Guest, Kind};
use web_sys::CanvasRenderingContext2d as C;
fn oval(c: &C, x: f64, y: f64, rx: f64, ry: f64, color: &str) {
    c.begin_path();
    let _ = c.ellipse(x, y, rx, ry, 0., 0., std::f64::consts::TAU);
    c.set_fill_style_str(color);
    c.fill();
}
fn line(c: &C, x: f64, y: f64, bx: f64, by: f64, color: &str, w: f64) {
    c.begin_path();
    c.move_to(x, y);
    c.line_to(bx, by);
    c.set_stroke_style_str(color);
    c.set_line_width(w);
    c.stroke();
}
pub fn draw(c: &C, g: &Guest) {
    c.save();
    let _ = c.translate(g.position.x, g.position.y);
    let _ = c.rotate(g.heading);
    c.set_global_alpha(g.alpha);
    c.set_line_cap("round");
    c.set_line_join("round");
    let walk = g.phase.sin();
    oval(c, 2., 4., 18., 9., "rgba(20,44,49,0.14)");
    match g.kind {
        Kind::Crab | Kind::Hermit => {
            for side in [-1., 1.] {
                for k in 0..3 {
                    let x = -7. + k as f64 * 6.;
                    let lift = (g.phase + k as f64 * 2.).sin() * 2.;
                    line(c, x, side * 6., x + 4., side * (15. + lift), "#99674b", 2.);
                }
            }
            oval(c, 0., 0., 14., 10., "#ce9062");
            for y in [-10., 10.] {
                line(c, 8., y * 0.5, 17., y, "#99674b", 2.);
                oval(c, 17., y, 4., 3., "#dfb080");
                oval(c, 12., y * 0.4, 1.6, 1.6, "#203c49");
            }
            if g.kind == Kind::Hermit {
                oval(c, -5., 0., 13., 12., "#dbad94");
                c.begin_path();
                let _ = c.arc(-5., 0., 7., 0., 5.5);
                c.set_stroke_style_str("#f3d9bc");
                c.set_line_width(2.);
                c.stroke();
            }
        }
        Kind::Minnow => {
            c.begin_path();
            c.move_to(-11., 0.);
            c.line_to(-25., -10. - walk * 2.);
            c.line_to(-23., 10. + walk * 2.);
            c.close_path();
            c.set_fill_style_str("#648e9d");
            c.fill();
            oval(c, 0., 0., 18., 8., "#9ebdb6");
            line(c, -7., -3., 9., -3., "#d4ddc5", 2.);
            oval(c, 11., -1., 2., 2., "#233e4a");
        }
        Kind::Shrimp => {
            c.begin_path();
            c.move_to(-18., 3.);
            c.bezier_curve_to(-14., -15., 11., -13., 15., 1.);
            c.bezier_curve_to(15., 10., 2., 12., 0., 7.);
            c.set_stroke_style_str("#dfa990");
            c.set_line_width(7.);
            c.stroke();
            for k in 0..4 {
                let x = -10. + k as f64 * 6.;
                line(c, x, -3., x + 2. + walk, 9., "#b77f72", 1.);
            }
            line(c, 14., 0., 27., -8. - walk, "#d4c0a0", 1.);
            line(c, 14., 0., 28., 5. + walk, "#d4c0a0", 1.);
            oval(c, 14., -2., 1.7, 1.7, "#263f4a");
        }
        Kind::SeaStar => {
            c.begin_path();
            for k in 0..10 {
                let a = k as f64 * std::f64::consts::PI / 5.;
                let r = if k % 2 == 0 { 21. } else { 9. };
                let (x, y) = (a.cos() * r, a.sin() * r);
                if k == 0 {
                    c.move_to(x, y);
                } else {
                    c.line_to(x, y);
                }
            }
            c.close_path();
            c.set_fill_style_str("#d6a18b");
            c.fill();
            c.set_stroke_style_str("#ab7f73");
            c.set_line_width(1.5);
            c.stroke();
            for k in 0..5 {
                let a = k as f64 * std::f64::consts::TAU / 5.;
                oval(c, a.cos() * 9., a.sin() * 9., 1.4, 1.4, "#f4ddbc");
            }
        }
        Kind::Sandpiper => {
            for y in [-5., 5.] {
                line(c, -4., y, walk * 4., y * 2.5, "#7e7662", 1.7);
                line(c, walk * 4., y * 2.5, 7., y * 2.5, "#7e7662", 1.4);
            }
            oval(c, 0., 0., 20., 11., "#d4cbb0");
            oval(c, -3., -2., 13., 7., "#8c9b96");
            oval(c, 15., 0., 8., 7., "#e9dec1");
            line(c, 21., 0., 35., 1., "#59686b", 2.);
            oval(c, 18., -2., 1.8, 1.8, "#233e4a");
        }
        Kind::Turtle => {
            for y in [-1., 1.] {
                oval(c, 8. + walk, y * 14., 9., 4., "#83a392");
                oval(c, -11. - walk, y * 12., 7., 3., "#83a392");
            }
            oval(c, 22., 0., 8., 6., "#a6b89c");
            oval(c, 0., 0., 19., 14., "#607f75");
            oval(c, -2., -2., 14., 10., "#88a18a");
            line(c, -11., 0., 10., 0., "#607f75", 1.);
            line(c, 0., -10., 0., 10., "#607f75", 1.);
            oval(c, 25., -2., 1.6, 1.6, "#233e4a");
        }
    }
    c.restore();
}
