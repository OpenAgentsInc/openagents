//! Title cards and captions, laid out and drawn the way `cut.py` draws them
//! with Pillow: titles in Paper Mono (in this repository, OFL), captions in
//! Geist Regular when the operator passes the film fonts, else Paper Mono.
//!
//! Glyphs are drawn one character at a time at their advance, without
//! kerning, as Pillow's basic layout does, so line widths match `cut.py`'s
//! within a pixel or so.

use std::path::Path;

use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};
use serde_json::Value;

const REGULAR: &[u8] = include_bytes!("../../paper-mono/fonts/PaperMono-Regular.ttf");
const MEDIUM: &[u8] = include_bytes!("../../paper-mono/fonts/PaperMono-Medium.ttf");
const SEMIBOLD: &[u8] = include_bytes!("../../paper-mono/fonts/PaperMono-SemiBold.ttf");
const BOLD: &[u8] = include_bytes!("../../paper-mono/fonts/PaperMono-Bold.ttf");
/// The caption font's file name in the film fonts folder (`FILM_FONTS`).
pub const CAPTION_FONT: &str = "Geist-Regular.ttf";

/// The fonts a render draws with.
#[derive(Clone)]
pub struct Fonts {
    weights: [FontArc; 4],
    caption: FontArc,
}

impl Default for Fonts {
    fn default() -> Self {
        let load = |b: &'static [u8]| FontArc::try_from_slice(b).expect("Paper Mono loads");
        let weights = [load(REGULAR), load(MEDIUM), load(SEMIBOLD), load(BOLD)];
        let caption = weights[0].clone();
        Self { weights, caption }
    }
}

impl Fonts {
    /// Paper Mono for titles, and the caption font from the film fonts folder.
    ///
    /// # Errors
    /// The folder has no readable `Geist-Regular.ttf`.
    pub fn with_film_fonts(dir: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(dir.join(CAPTION_FONT))
            .map_err(|_| format!("Put {CAPTION_FONT} in the fonts folder."))?;
        let caption = FontArc::try_from_vec(bytes)
            .map_err(|_| format!("{CAPTION_FONT} isn't a font this computer can read."))?;
        Ok(Self {
            caption,
            ..Self::default()
        })
    }

    fn weight(&self, name: &str) -> &FontArc {
        &self.weights[match name {
            "medium" => 1,
            "semibold" => 2,
            "bold" => 3,
            _ => 0,
        }]
    }
}

/// A font at a size in pixels per em, as Pillow's `truetype(font, size)`.
#[derive(Clone, Copy)]
struct Face<'a> {
    font: &'a FontArc,
    px: f32,
}

impl Face<'_> {
    fn scaled(&self) -> ab_glyph::PxScaleFont<&FontArc> {
        self.font.as_scaled(self.scale())
    }

    fn scale(&self) -> PxScale {
        self.font
            .pt_to_px_scale(self.px)
            .unwrap_or(PxScale::from(self.px))
    }

    /// Pillow's `getmetrics()`: whole pixels above and below the baseline.
    fn metrics(&self) -> (f64, f64) {
        let s = self.scaled();
        (f64::from(s.ascent()).ceil(), f64::from(-s.descent()).ceil())
    }

    fn advance(&self, c: char) -> f64 {
        let s = self.scaled();
        f64::from(s.h_advance(s.glyph_id(c)))
    }

    /// Pillow's `getlength(text)`.
    fn length(&self, text: &str) -> f64 {
        text.chars().map(|c| self.advance(c)).sum()
    }

    /// Calls `put(x, y, coverage)` for each pixel of `c` drawn with its top
    /// (Pillow's ascender line) at `(x, y)`.
    fn draw(&self, c: char, x: f64, y: f64, mut put: impl FnMut(i64, i64, f32)) {
        let (ascent, _) = self.metrics();
        let glyph = self
            .font
            .glyph_id(c)
            .with_scale_and_position(self.scale(), point(x as f32, (y + ascent) as f32));
        if let Some(outline) = self.font.outline_glyph(glyph) {
            let b = outline.px_bounds();
            outline.draw(|gx, gy, cov| {
                put(
                    b.min.x as i64 + i64::from(gx),
                    b.min.y as i64 + i64::from(gy),
                    cov,
                );
            });
        }
    }
}

fn num(v: &Value, key: &str, default: f64) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(default)
}

/// One title line, laid out.
#[derive(Clone, Debug)]
pub struct TitleLine {
    pub text: String,
    weight: String,
    pub px: u32,
    tracking: f64,
    pub width: f64,
    pub height: f64,
    /// x0, y0, x1, y1
    pub bounds: [f64; 4],
    delay: f64,
    rise: f64,
    color: [f64; 3],
}

/// Each line's font, size and box, centered as a block (`cut.py`'s
/// `title_layout`). Sizes are fractions of the frame height.
pub fn title_layout(spec: &Value, w: u32, h: u32, fonts: &Fonts) -> Vec<TitleLine> {
    let (wf, hf) = (f64::from(w), f64::from(h));
    let mut lines: Vec<TitleLine> = Vec::new();
    for (i, ln) in spec["lines"].as_array().into_iter().flatten().enumerate() {
        let px = ((num(ln, "size", 0.05) * hf) as u32).max(8);
        let weight = ln["weight"].as_str().unwrap_or("regular").to_owned();
        let face = Face {
            font: fonts.weight(&weight),
            px: px as f32,
        };
        let text = ln["text"].as_str().unwrap_or_default().to_owned();
        let tracking = num(ln, "tracking", 0.0) * f64::from(px);
        let count = text.chars().count();
        let width = face.length(&text) + tracking * count.saturating_sub(1) as f64;
        let (asc, desc) = face.metrics();
        let color = ln["color"]
            .as_array()
            .map(|c| {
                let at = |k: usize| c.get(k).and_then(Value::as_f64).unwrap_or(236.0);
                [at(0), at(1), at(2)]
            })
            .unwrap_or([236.0, 238.0, 241.0]);
        lines.push(TitleLine {
            text,
            weight,
            px,
            tracking,
            width,
            height: asc + desc,
            bounds: [0.0; 4],
            delay: num(ln, "delay", 0.25 + 0.35 * i as f64),
            rise: num(ln, "rise", 1.1),
            color,
        });
    }
    let gap = num(spec, "gap", 0.025) * hf;
    let block: f64 =
        lines.iter().map(|l| l.height).sum::<f64>() + gap * lines.len().saturating_sub(1) as f64;
    let mut y = hf * num(spec, "center", 0.5) - block / 2.0;
    for l in &mut lines {
        l.bounds = [(wf - l.width) / 2.0, y, (wf + l.width) / 2.0, y + l.height];
        y += l.height + gap;
    }
    lines
}

fn ease_out(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    1.0 - (1.0 - x).powi(3)
}

/// The near-black, neutral backdrop with a soft pool of light, as RGB floats.
pub fn backdrop(w: u32, h: u32) -> Vec<f32> {
    let (wf, hf) = (f64::from(w), f64::from(h));
    let (cx, cy) = (wf * 0.5, hf * 0.46);
    let mut out = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let r = (((f64::from(x) - cx) / wf).powi(2) + ((f64::from(y) - cy) / hf * 0.8).powi(2))
                .sqrt();
            let v = (6.0 + 14.0 * (-(r / 0.42).powi(2)).exp()) as f32;
            out.extend([v, v, v]);
        }
    }
    out
}

/// A small seeded generator for film grain (xorshift, Box-Muller).
pub struct Grain(u64);

impl Grain {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    fn normal(&mut self) -> f64 {
        let (u, v) = (self.next(), self.next());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

/// One title frame at time `t` of a card `dur` seconds long, over `base`
/// (RGB floats), as RGB bytes (`cut.py`'s `draw_title`): each line fades
/// and rises in after its delay, its tracking settling from wide, and fades
/// out over the spec's `out` seconds.
#[allow(clippy::too_many_arguments)]
pub fn draw_title(
    spec: &Value,
    lines: &[TitleLine],
    w: u32,
    h: u32,
    t: f64,
    dur: f64,
    base: &[f32],
    grain: Option<&mut Grain>,
    fonts: &Fonts,
) -> Vec<u8> {
    let mut img: Vec<f32> = base.to_vec();
    if let Some(g) = grain {
        for px in img.chunks_mut(3) {
            let n = (g.normal() * 1.6) as f32;
            px.iter_mut().for_each(|c| *c += n);
        }
    }
    let mut frame: Vec<f32> = img.iter().map(|c| c.clamp(0.0, 255.0).trunc()).collect();
    let hf = f64::from(h);
    let out = num(spec, "out", 0.0);
    for l in lines {
        let mut p = ease_out((t - l.delay) / l.rise);
        if out > 0.0 {
            p *= 1.0 - ease_out((t - (dur - out)) / out);
        }
        if p <= 0.0 {
            continue;
        }
        let face = Face {
            font: fonts.weight(&l.weight),
            px: l.px as f32,
        };
        let track = l.tracking + (1.0 - p) * 0.12 * f64::from(l.px);
        let count = l.text.chars().count();
        let width = face.length(&l.text) + track * count.saturating_sub(1) as f64;
        let mut x = (f64::from(w) - width) / 2.0;
        let y = l.bounds[1] + (1.0 - p) * 0.012 * hf;
        let alpha = (255.0 * p).trunc() / 255.0;
        for c in l.text.chars() {
            face.draw(c, x, y, |px, py, cov| {
                if px < 0 || py < 0 || px >= i64::from(w) || py >= i64::from(h) {
                    return;
                }
                let a = f64::from(cov.clamp(0.0, 1.0)) * alpha;
                let i = ((py as u32 * w + px as u32) * 3) as usize;
                for k in 0..3 {
                    let under = f64::from(frame[i + k]);
                    frame[i + k] = (l.color[k].trunc() * a + under * (1.0 - a)) as f32;
                }
            });
            x += face.advance(c) + track;
        }
    }
    frame
        .iter()
        .map(|c| c.round().clamp(0.0, 255.0) as u8)
        .collect()
}

/// The cut's caption style at frame height `h` (`cut.py`'s `caption_style`).
#[derive(Clone, Copy, Debug)]
pub struct CaptionStyle {
    pub px: u32,
    y: f64,
    width: f64,
}

pub fn caption_style(cut: &Value, h: u32) -> CaptionStyle {
    let st = &cut["caption_style"];
    CaptionStyle {
        px: ((num(st, "size", 0.03) * f64::from(h)) as u32).max(8),
        y: num(st, "y", 0.855),
        width: num(st, "width", 0.66),
    }
}

fn wrap(text: &str, face: Face<'_>, maxw: f64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let trial = if cur.is_empty() {
            word.to_owned()
        } else {
            format!("{cur} {word}")
        };
        if !cur.is_empty() && face.length(&trial) > maxw {
            lines.push(std::mem::replace(&mut cur, word.to_owned()));
        } else {
            cur = trial;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.len() == 2 {
        // Balance two lines so the second isn't a stray word or two.
        let words: Vec<&str> = text.split_whitespace().collect();
        let cost = |k: usize| {
            face.length(&words[..k].join(" "))
                .max(face.length(&words[k..].join(" ")))
        };
        let mut best = 1;
        for k in 2..words.len() {
            if cost(k) < cost(best) {
                best = k;
            }
        }
        lines = vec![words[..best].join(" "), words[best..].join(" ")];
    }
    lines
}

/// One caption, laid out: its lines and its band's box.
#[derive(Clone, Debug)]
pub struct Caption {
    pub lines: Vec<String>,
    /// x, y, width, height of the band, in pixels.
    pub bounds: [i64; 4],
    line_height: i64,
    pady: i64,
    px: u32,
}

/// Where a caption's band sits (`cut.py`'s `caption_layout`).
pub fn caption_layout(text: &str, st: CaptionStyle, w: u32, h: u32, fonts: &Fonts) -> Caption {
    let face = Face {
        font: &fonts.caption,
        px: st.px as f32,
    };
    let lines = wrap(text, face, st.width * f64::from(w));
    let (asc, desc) = face.metrics();
    let lh = ((asc + desc) * 1.18) as i64;
    let px = f64::from(st.px);
    let (padx, pady) = ((px * 0.8) as i64, (px * 0.42) as i64);
    let widest = lines.iter().map(|l| face.length(l)).fold(0.0, f64::max);
    let bw = widest as i64 + 2 * padx;
    let bh = lh * lines.len() as i64 + 2 * pady;
    let x = (i64::from(w) - bw).div_euclid(2);
    let y = (st.y * f64::from(h) - bh as f64 / 2.0) as i64;
    Caption {
        lines,
        bounds: [x, y, bw, bh],
        line_height: lh,
        pady,
        px: st.px,
    }
}

/// The caption's band as RGBA bytes, `bounds[2]` by `bounds[3]`: a soft dark
/// rounded band with the lines centered on it (`cut.py`'s `render_caption`).
pub fn caption_rgba(cap: &Caption, fonts: &Fonts) -> Vec<u8> {
    let [_, _, bw, bh] = cap.bounds;
    let (bw, bh) = (bw.max(1), bh.max(1));
    let radius = (f64::from(cap.px) * 0.5) as i64;
    let mut im = vec![0u8; (bw * bh * 4) as usize];
    for y in 0..bh {
        for x in 0..bw {
            if inside_rounded(x, y, bw, bh, radius) {
                im[((y * bw + x) * 4 + 3) as usize] = 185;
            }
        }
    }
    let face = Face {
        font: &fonts.caption,
        px: cap.px as f32,
    };
    let ink = [238.0f32, 239.0, 242.0, 255.0];
    for (i, line) in cap.lines.iter().enumerate() {
        let mut x = (bw as f64 - face.length(line)) / 2.0;
        let y = (cap.pady + i as i64 * cap.line_height) as f64;
        for c in line.chars() {
            face.draw(c, x, y, |px, py, cov| {
                if px < 0 || py < 0 || px >= bw || py >= bh {
                    return;
                }
                let m = cov.clamp(0.0, 1.0);
                let at = ((py * bw + px) * 4) as usize;
                for k in 0..4 {
                    let old = f32::from(im[at + k]);
                    im[at + k] = (ink[k] * m + old * (1.0 - m)).round() as u8;
                }
            });
            x += face.advance(c);
        }
    }
    im
}

fn inside_rounded(x: i64, y: i64, w: i64, h: i64, r: i64) -> bool {
    let r = r.min(w / 2).min(h / 2);
    let cx = if x < r {
        r
    } else if x > w - 1 - r {
        w - 1 - r
    } else {
        return true;
    };
    let cy = if y < r {
        r
    } else if y > h - 1 - r {
        h - 1 - r
    } else {
        return true;
    };
    let (dx, dy) = (x - cx, y - cy);
    dx * dx + dy * dy <= r * r
}

/// Draws `text` in Paper Mono Regular at `px` pixels per em onto an RGB
/// frame, its top at `(x, y)`.
#[allow(clippy::too_many_arguments)]
pub fn draw_label(
    rgb: &mut [u8],
    w: u32,
    h: u32,
    x: f64,
    y: f64,
    text: &str,
    px: f32,
    color: [u8; 3],
    fonts: &Fonts,
) {
    let face = Face {
        font: fonts.weight("regular"),
        px,
    };
    let mut x = x;
    for c in text.chars() {
        face.draw(c, x, y, |gx, gy, cov| {
            if gx < 0 || gy < 0 || gx >= i64::from(w) || gy >= i64::from(h) {
                return;
            }
            let m = cov.clamp(0.0, 1.0);
            let at = ((gy as u32 * w + gx as u32) * 3) as usize;
            for k in 0..3 {
                let old = f32::from(rgb[at + k]);
                rgb[at + k] = (f32::from(color[k]) * m + old * (1.0 - m)).round() as u8;
            }
        });
        x += face.advance(c);
    }
}

/// Writes RGBA bytes as a PNG.
///
/// # Errors
/// The file can't be written.
pub fn write_png(path: &Path, rgba: &[u8], w: u32, h: u32) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .and_then(|mut wr| wr.write_image_data(rgba))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_title_is_centered_as_a_block() {
        let fonts = Fonts::default();
        let spec = json!({"lines": [{"text": "OPENAGENTS", "size": 0.08, "weight": "bold"},
                                    {"text": "Coder", "size": 0.04}]});
        let lines = title_layout(&spec, 1920, 1080, &fonts);
        assert_eq!(lines[0].px, 86);
        assert_eq!(lines[1].px, 43);
        // Paper Mono is monospaced: ten characters are ten advances.
        let one = Face {
            font: fonts.weight("bold"),
            px: 86.0,
        }
        .advance('O');
        assert!((lines[0].width - 10.0 * one).abs() < 1e-6);
        let [x0, y0, x1, _] = lines[0].bounds;
        assert!(((x0 + x1) / 2.0 - 960.0).abs() < 1e-6);
        let block = lines[1].bounds[3] - y0;
        assert!(((y0 + block / 2.0) - 540.0).abs() < 1.0, "{y0} {block}");
    }

    #[test]
    fn a_title_fades_in_over_its_backdrop() {
        let fonts = Fonts::default();
        let spec = json!({"lines": [{"text": "HI", "size": 0.2}]});
        let (w, h) = (160, 90);
        let lines = title_layout(&spec, w, h, &fonts);
        let base = backdrop(w, h);
        let before = draw_title(&spec, &lines, w, h, 0.0, 2.0, &base, None, &fonts);
        let after = draw_title(&spec, &lines, w, h, 1.6, 2.0, &base, None, &fonts);
        let bright = |f: &[u8]| f.iter().filter(|&&c| c > 120).count();
        assert_eq!(bright(&before), 0);
        assert!(bright(&after) > 50, "{}", bright(&after));
        // The pool of light is brightest in the middle.
        assert!(base[((41 * w + 80) * 3) as usize] > base[0]);
    }

    #[test]
    fn captions_wrap_to_balanced_lines_inside_the_frame() {
        let fonts = Fonts::default();
        let cut = json!({});
        let st = caption_style(&cut, 360);
        assert_eq!(st.px, 10);
        let short = caption_layout("Hello there", st, 640, 360, &fonts);
        assert_eq!(short.lines, ["Hello there"]);
        let long = caption_layout(
            "Every edit is a new version and a refused edit never reaches a render",
            st,
            640,
            360,
            &fonts,
        );
        assert_eq!(long.lines.len(), 2);
        let [x, y, bw, bh] = long.bounds;
        assert!(x >= 0 && x + bw <= 640 && y > 180 && y + bh <= 360);
        let rgba = caption_rgba(&long, &fonts);
        assert_eq!(rgba.len() as i64, bw * bh * 4);
        // The band's corners are clear and its middle is the dark band.
        assert_eq!(rgba[3], 0);
        assert!(rgba.chunks(4).any(|p| p[0] > 200 && p[3] > 200));
    }
}
