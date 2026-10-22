//! The die-cut cards, drawn in code: flat palette shapes in the manner of
//! vintage Halloween prints, cut out with a paper edge and a thick licorice
//! border, about 512 px tall on a transparent ground. Nothing is traced
//! from a reference; every card is these shapes.
//!
//! A card is a list of shapes in its own unit square (x right, y down),
//! painted in order. [`draw`] rasterises it into a palette image
//! (`icecream_zone::image`), which a bake packs into a content-addressed
//! chunk and the page decodes into a texture.

use icecream_zone::image::Image;

use crate::PALETTE;

/// The card's height, px.
pub const HEIGHT: u16 = 512;
/// The paper edge around the figure, px.
const PAPER_EDGE: f32 = 6.0;
/// The licorice border outside that, px.
const BORDER: f32 = 8.0;
/// Room around the figure for the edge and the border, px.
const PAD: f32 = 18.0;

/// A palette index: 0 is transparent, then the palette in order.
fn ix(token: &str) -> u8 {
    PALETTE
        .iter()
        .position(|(name, _, _)| *name == token)
        .map_or(4, |i| i as u8 + 1)
}

/// One shape in the card's unit square.
#[derive(Clone, Debug)]
enum Shape {
    /// Centre, radii, and a turn in radians.
    Ellipse([f32; 2], [f32; 2], f32),
    /// A polygon, any winding.
    Poly(Vec<[f32; 2]>),
    /// A thick stroke between two points.
    Line([f32; 2], [f32; 2], f32),
}

struct Card {
    /// Width over height.
    aspect: f32,
    shapes: Vec<(Shape, u8)>,
}

impl Card {
    fn new(aspect: f32) -> Self {
        Self {
            aspect,
            shapes: Vec::new(),
        }
    }
    fn e(&mut self, c: [f32; 2], r: [f32; 2], token: &str) -> &mut Self {
        self.shapes.push((Shape::Ellipse(c, r, 0.0), ix(token)));
        self
    }
    fn et(&mut self, c: [f32; 2], r: [f32; 2], turn: f32, token: &str) -> &mut Self {
        self.shapes.push((Shape::Ellipse(c, r, turn), ix(token)));
        self
    }
    fn p(&mut self, points: &[[f32; 2]], token: &str) -> &mut Self {
        self.shapes.push((Shape::Poly(points.to_vec()), ix(token)));
        self
    }
    fn l(&mut self, a: [f32; 2], b: [f32; 2], w: f32, token: &str) -> &mut Self {
        self.shapes.push((Shape::Line(a, b, w), ix(token)));
        self
    }
    /// A chain of strokes.
    fn path(&mut self, points: &[[f32; 2]], w: f32, token: &str) -> &mut Self {
        for pair in points.windows(2) {
            self.l(pair[0], pair[1], w, token);
            self.e(pair[1], [w / 2.0, w / 2.0], token);
        }
        self
    }
}

fn inside_poly(points: &[[f32; 2]], x: f32, y: f32) -> bool {
    let mut inside = false;
    let mut j = points.len() - 1;
    for i in 0..points.len() {
        let (a, b) = (points[i], points[j]);
        if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn line_poly(a: [f32; 2], b: [f32; 2], w: f32) -> Vec<[f32; 2]> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = dx.hypot(dy).max(1e-6);
    let (nx, ny) = (-dy / len * w / 2.0, dx / len * w / 2.0);
    vec![
        [a[0] + nx, a[1] + ny],
        [b[0] + nx, b[1] + ny],
        [b[0] - nx, b[1] - ny],
        [a[0] - nx, a[1] - ny],
    ]
}

fn rasterise(card: &Card) -> Image {
    let height = HEIGHT;
    let width = ((f32::from(height) - 2.0 * PAD) * card.aspect + 2.0 * PAD).round() as u16;
    let (w, h) = (usize::from(width), usize::from(height));
    let inner = [f32::from(width) - 2.0 * PAD, f32::from(height) - 2.0 * PAD];
    let to_px = |p: [f32; 2]| [PAD + p[0] * inner[0], PAD + p[1] * inner[1]];
    let mut pixels = vec![0u8; w * h];
    for (shape, colour) in &card.shapes {
        let (points, ellipse) = match shape {
            Shape::Poly(points) => (points.iter().map(|p| to_px(*p)).collect::<Vec<_>>(), None),
            Shape::Line(a, b, width) => {
                let scale = inner[1];
                let (a, b) = (to_px(*a), to_px(*b));
                (line_poly(a, b, width * scale), None)
            }
            Shape::Ellipse(c, r, turn) => {
                let c = to_px(*c);
                let r = [r[0] * inner[0], r[1] * inner[1]];
                let reach = r[0].max(r[1]);
                (
                    vec![[c[0] - reach, c[1] - reach], [c[0] + reach, c[1] + reach]],
                    Some((c, r, *turn)),
                )
            }
        };
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in &points {
            x0 = x0.min(p[0]);
            y0 = y0.min(p[1]);
            x1 = x1.max(p[0]);
            y1 = y1.max(p[1]);
        }
        let xs = (x0.floor().max(0.0) as usize)..(x1.ceil().min(w as f32) as usize);
        let ys = (y0.floor().max(0.0) as usize)..(y1.ceil().min(h as f32) as usize);
        for y in ys {
            for x in xs.clone() {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let hit = match ellipse {
                    Some((c, r, turn)) => {
                        let (s, co) = turn.sin_cos();
                        let (dx, dy) = (px - c[0], py - c[1]);
                        let (u, v) = (dx * co + dy * s, -dx * s + dy * co);
                        (u / r[0]).powi(2) + (v / r[1]).powi(2) <= 1.0
                    }
                    None => inside_poly(&points, px, py),
                };
                if hit {
                    pixels[y * w + x] = *colour;
                }
            }
        }
    }
    // The die cut: distance from the figure (a two-pass chamfer), then a
    // paper edge and a licorice border.
    let mut dist = vec![f32::MAX; w * h];
    for (i, p) in pixels.iter().enumerate() {
        if *p != 0 {
            dist[i] = 0.0;
        }
    }
    let (a, b) = (1.0_f32, std::f32::consts::SQRT_2);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let mut d = dist[i];
            if x > 0 {
                d = d.min(dist[i - 1] + a);
            }
            if y > 0 {
                d = d.min(dist[i - w] + a);
                if x > 0 {
                    d = d.min(dist[i - w - 1] + b);
                }
                if x + 1 < w {
                    d = d.min(dist[i - w + 1] + b);
                }
            }
            dist[i] = d;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            let mut d = dist[i];
            if x + 1 < w {
                d = d.min(dist[i + 1] + a);
            }
            if y + 1 < h {
                d = d.min(dist[i + w] + a);
                if x + 1 < w {
                    d = d.min(dist[i + w + 1] + b);
                }
                if x > 0 {
                    d = d.min(dist[i + w - 1] + b);
                }
            }
            dist[i] = d;
        }
    }
    let (paper, licorice) = (ix("paper"), ix("licorice"));
    for (i, p) in pixels.iter_mut().enumerate() {
        if *p == 0 {
            if dist[i] <= PAPER_EDGE {
                *p = paper;
            } else if dist[i] <= PAPER_EDGE + BORDER {
                *p = licorice;
            }
        }
    }
    let mut palette = vec![[0, 0, 0, 0]];
    for (_, hex, _) in PALETTE {
        palette.push([(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255]);
    }
    Image {
        width,
        height,
        palette,
        pixels,
    }
}

fn black_cat() -> Card {
    let mut c = Card::new(0.78);
    c.path(
        &[[0.72, 0.9], [0.92, 0.82], [0.95, 0.6], [0.85, 0.45]],
        0.07,
        "licorice",
    )
    .e([0.45, 0.72], [0.36, 0.24], "licorice")
    .e([0.42, 0.36], [0.27, 0.21], "licorice")
    .p(&[[0.2, 0.3], [0.17, 0.05], [0.36, 0.2]], "licorice")
    .p(&[[0.5, 0.2], [0.68, 0.06], [0.66, 0.3]], "licorice")
    .p(&[[0.23, 0.26], [0.22, 0.13], [0.31, 0.21]], "brick")
    .p(&[[0.56, 0.21], [0.63, 0.13], [0.62, 0.26]], "brick")
    .e([0.32, 0.34], [0.07, 0.055], "mustard")
    .e([0.54, 0.34], [0.07, 0.055], "mustard")
    .e([0.33, 0.34], [0.015, 0.045], "licorice")
    .e([0.55, 0.34], [0.015, 0.045], "licorice")
    .p(&[[0.4, 0.4], [0.46, 0.4], [0.43, 0.43]], "brick")
    // The grin.
    .p(
        &[
            [0.27, 0.45],
            [0.43, 0.5],
            [0.59, 0.45],
            [0.55, 0.49],
            [0.43, 0.54],
            [0.31, 0.49],
        ],
        "paper",
    )
    .l([0.18, 0.44], [0.03, 0.41], 0.008, "paper")
    .l([0.18, 0.47], [0.03, 0.49], 0.008, "paper")
    .l([0.68, 0.44], [0.83, 0.41], 0.008, "paper")
    .l([0.68, 0.47], [0.83, 0.49], 0.008, "paper")
    // A party ruff.
    .p(
        &[
            [0.22, 0.56],
            [0.3, 0.62],
            [0.38, 0.57],
            [0.45, 0.63],
            [0.52, 0.57],
            [0.6, 0.62],
            [0.66, 0.55],
            [0.6, 0.67],
            [0.3, 0.67],
        ],
        "pumpkin",
    );
    c
}

fn werewolf() -> Card {
    let mut c = Card::new(0.6);
    // Legs and boots.
    c.p(
        &[[0.3, 0.62], [0.47, 0.62], [0.45, 0.95], [0.3, 0.95]],
        "night_teal",
    )
    .p(
        &[[0.53, 0.62], [0.7, 0.62], [0.7, 0.95], [0.55, 0.95]],
        "night_teal",
    )
    .p(
        &[[0.25, 0.93], [0.47, 0.93], [0.47, 0.99], [0.22, 0.99]],
        "plum",
    )
    .p(
        &[[0.53, 0.93], [0.76, 0.93], [0.78, 0.99], [0.53, 0.99]],
        "plum",
    )
    // Torn shirt.
    .p(
        &[
            [0.24, 0.3],
            [0.76, 0.3],
            [0.8, 0.5],
            [0.72, 0.66],
            [0.64, 0.6],
            [0.56, 0.68],
            [0.46, 0.6],
            [0.36, 0.68],
            [0.27, 0.62],
            [0.2, 0.5],
        ],
        "brick",
    )
    .p(&[[0.42, 0.3], [0.58, 0.3], [0.5, 0.48]], "plum")
    // Arms reaching, with claws.
    .p(
        &[[0.24, 0.32], [0.06, 0.5], [0.1, 0.56], [0.27, 0.42]],
        "plum",
    )
    .p(
        &[[0.76, 0.32], [0.94, 0.5], [0.9, 0.56], [0.73, 0.42]],
        "plum",
    )
    .e([0.08, 0.56], [0.06, 0.04], "plum")
    .e([0.92, 0.56], [0.06, 0.04], "plum");
    for k in 0..3 {
        let o = k as f32 * 0.03;
        c.p(
            &[[0.03 + o, 0.58], [0.05 + o, 0.58], [0.03 + o, 0.63]],
            "paper",
        );
        c.p(
            &[[0.92 + o, 0.58], [0.94 + o, 0.58], [0.95 + o, 0.63]],
            "paper",
        );
    }
    // Head: shaggy, pointed ears, a long snout.
    c.e([0.5, 0.2], [0.21, 0.15], "plum")
        .p(&[[0.3, 0.15], [0.28, -0.0], [0.41, 0.09]], "plum")
        .p(&[[0.7, 0.15], [0.72, -0.0], [0.59, 0.09]], "plum")
        .p(
            &[
                [0.3, 0.3],
                [0.33, 0.36],
                [0.38, 0.31],
                [0.43, 0.37],
                [0.5, 0.31],
                [0.57, 0.37],
                [0.62, 0.31],
                [0.67, 0.36],
                [0.7, 0.3],
            ],
            "plum",
        )
        .e([0.5, 0.27], [0.11, 0.07], "dusk")
        .e([0.5, 0.22], [0.035, 0.025], "licorice")
        .e([0.4, 0.15], [0.045, 0.03], "mustard")
        .e([0.6, 0.15], [0.045, 0.03], "mustard")
        .e([0.4, 0.15], [0.012, 0.025], "licorice")
        .e([0.6, 0.15], [0.012, 0.025], "licorice")
        .p(&[[0.42, 0.29], [0.58, 0.29], [0.5, 0.33]], "licorice")
        .p(&[[0.44, 0.29], [0.46, 0.29], [0.45, 0.32]], "paper")
        .p(&[[0.54, 0.29], [0.56, 0.29], [0.55, 0.32]], "paper");
    c
}

fn sheet_ghost() -> Card {
    let mut c = Card::new(0.62);
    c.e([0.5, 0.28], [0.36, 0.26], "paper").p(
        &[
            [0.14, 0.28],
            [0.86, 0.28],
            [0.94, 0.92],
            [0.84, 0.86],
            [0.74, 0.96],
            [0.63, 0.87],
            [0.5, 0.97],
            [0.38, 0.87],
            [0.26, 0.96],
            [0.15, 0.86],
            [0.06, 0.92],
        ],
        "paper",
    );
    // Folds in the sheet, a little plum.
    c.l([0.3, 0.45], [0.24, 0.85], 0.02, "dusk")
        .l([0.7, 0.45], [0.77, 0.84], 0.02, "dusk")
        .l([0.5, 0.6], [0.5, 0.9], 0.015, "dusk")
        .e([0.37, 0.27], [0.06, 0.08], "licorice")
        .e([0.63, 0.27], [0.06, 0.08], "licorice")
        .e([0.5, 0.42], [0.05, 0.045], "licorice")
        // Little sneakers peeking out: a kid in a costume.
        .e([0.38, 0.98], [0.07, 0.025], "pumpkin")
        .e([0.62, 0.98], [0.07, 0.025], "pumpkin");
    c
}

fn bat(c: &mut Card, at: [f32; 2], s: f32) {
    let [x, y] = at;
    c.p(
        &[
            [x, y],
            [x - 0.45 * s, y - 0.25 * s],
            [x - 0.38 * s, y + 0.02 * s],
            [x - 0.27 * s, y - 0.04 * s],
            [x - 0.2 * s, y + 0.08 * s],
            [x - 0.1 * s, y + 0.02 * s],
        ],
        "licorice",
    )
    .p(
        &[
            [x, y],
            [x + 0.45 * s, y - 0.25 * s],
            [x + 0.38 * s, y + 0.02 * s],
            [x + 0.27 * s, y - 0.04 * s],
            [x + 0.2 * s, y + 0.08 * s],
            [x + 0.1 * s, y + 0.02 * s],
        ],
        "licorice",
    )
    .e([x, y + 0.02 * s], [0.07 * s, 0.1 * s], "licorice")
    .p(
        &[
            [x - 0.05 * s, y - 0.05 * s],
            [x - 0.07 * s, y - 0.14 * s],
            [x - 0.01 * s, y - 0.07 * s],
        ],
        "licorice",
    )
    .p(
        &[
            [x + 0.05 * s, y - 0.05 * s],
            [x + 0.07 * s, y - 0.14 * s],
            [x + 0.01 * s, y - 0.07 * s],
        ],
        "licorice",
    )
    .e([x - 0.025 * s, y], [0.015 * s, 0.015 * s], "mustard")
    .e([x + 0.025 * s, y], [0.015 * s, 0.015 * s], "mustard");
}

fn bat_swarm() -> Card {
    let mut c = Card::new(1.5);
    for (at, s) in [
        ([0.5, 0.45], 0.9),
        ([0.22, 0.25], 0.5),
        ([0.8, 0.22], 0.55),
        ([0.2, 0.75], 0.45),
        ([0.78, 0.78], 0.5),
    ] {
        bat(&mut c, at, s);
    }
    c
}

fn witch() -> Card {
    let mut c = Card::new(0.62);
    // Broom behind her.
    c.l([0.05, 0.95], [0.95, 0.55], 0.03, "pumpkin_deep")
        .p(
            &[[0.0, 0.98], [0.12, 0.86], [0.16, 0.95], [0.06, 1.0]],
            "mustard",
        )
        // Dress.
        .p(
            &[[0.32, 0.4], [0.68, 0.4], [0.84, 0.98], [0.16, 0.98]],
            "plum",
        )
        .p(
            &[[0.32, 0.42], [0.12, 0.62], [0.18, 0.66], [0.36, 0.5]],
            "plum",
        )
        .p(
            &[[0.68, 0.42], [0.88, 0.58], [0.84, 0.63], [0.64, 0.5]],
            "plum",
        )
        .e([0.15, 0.65], [0.045, 0.035], "moss")
        .e([0.86, 0.61], [0.045, 0.035], "moss")
        // Face and long nose.
        .e([0.5, 0.3], [0.15, 0.12], "moss")
        .p(&[[0.5, 0.29], [0.67, 0.37], [0.52, 0.34]], "moss")
        .e([0.44, 0.27], [0.025, 0.02], "licorice")
        .e([0.56, 0.27], [0.025, 0.02], "licorice")
        .p(
            &[[0.4, 0.36], [0.5, 0.38], [0.58, 0.35], [0.5, 0.41]],
            "licorice",
        )
        // Hair and hat.
        .p(&[[0.33, 0.22], [0.26, 0.48], [0.36, 0.38]], "pumpkin")
        .p(&[[0.67, 0.22], [0.74, 0.48], [0.64, 0.38]], "pumpkin")
        .e([0.5, 0.21], [0.3, 0.035], "licorice")
        .p(
            &[
                [0.33, 0.21],
                [0.67, 0.21],
                [0.6, 0.06],
                [0.72, -0.0],
                [0.5, 0.02],
            ],
            "licorice",
        )
        .p(
            &[[0.34, 0.19], [0.66, 0.19], [0.65, 0.16], [0.35, 0.16]],
            "mustard",
        );
    c
}

fn skeleton() -> Card {
    let mut c = Card::new(0.58);
    let bone = "paper";
    c.e([0.5, 0.14], [0.16, 0.12], bone)
        .p(&[[0.4, 0.2], [0.6, 0.2], [0.57, 0.28], [0.43, 0.28]], bone)
        .e([0.44, 0.13], [0.045, 0.045], "licorice")
        .e([0.56, 0.13], [0.045, 0.045], "licorice")
        .p(&[[0.49, 0.19], [0.51, 0.19], [0.5, 0.21]], "licorice")
        .l([0.44, 0.25], [0.56, 0.25], 0.008, "licorice")
        .l([0.5, 0.28], [0.5, 0.62], 0.035, bone);
    for k in 0..4 {
        let y = 0.34 + k as f32 * 0.06;
        let w = 0.17 - k as f32 * 0.02;
        c.et([0.5, y], [w, 0.015], 0.0, bone);
    }
    // Dancing: one arm up, one down, a knee bent.
    c.path(&[[0.36, 0.33], [0.2, 0.22], [0.16, 0.05]], 0.03, bone)
        .path(&[[0.64, 0.33], [0.8, 0.45], [0.9, 0.38]], 0.03, bone)
        .e([0.5, 0.64], [0.1, 0.04], bone)
        .path(&[[0.44, 0.66], [0.3, 0.8], [0.36, 0.98]], 0.035, bone)
        .path(&[[0.56, 0.66], [0.62, 0.82], [0.78, 0.9]], 0.035, bone)
        .e([0.33, 0.98], [0.07, 0.02], bone)
        .e([0.82, 0.92], [0.06, 0.02], bone);
    c
}

fn mummy() -> Card {
    let mut c = Card::new(0.55);
    c.e([0.5, 0.15], [0.17, 0.14], "paper")
        .p(
            &[[0.3, 0.26], [0.7, 0.26], [0.72, 0.65], [0.28, 0.65]],
            "paper",
        )
        .p(
            &[[0.3, 0.6], [0.48, 0.6], [0.46, 0.98], [0.3, 0.98]],
            "paper",
        )
        .p(
            &[[0.52, 0.6], [0.7, 0.6], [0.7, 0.98], [0.54, 0.98]],
            "paper",
        )
        .p(
            &[[0.3, 0.3], [0.04, 0.38], [0.06, 0.45], [0.32, 0.4]],
            "paper",
        )
        .p(
            &[[0.7, 0.3], [0.96, 0.38], [0.94, 0.45], [0.68, 0.4]],
            "paper",
        );
    for k in 0..12 {
        let y = 0.06 + k as f32 * 0.078;
        c.l([0.26, y], [0.74, y + 0.03], 0.008, "dusk");
    }
    c.e([0.44, 0.14], [0.04, 0.025], "mustard")
        .e([0.56, 0.14], [0.04, 0.025], "mustard")
        // The loose bandage.
        .path(
            &[[0.7, 0.5], [0.82, 0.62], [0.78, 0.78], [0.9, 0.9]],
            0.03,
            "paper",
        );
    c
}

fn zombie_hand() -> Card {
    let mut c = Card::new(1.0);
    c.p(
        &[[0.0, 0.85], [1.0, 0.85], [1.0, 1.0], [0.0, 1.0]],
        "pumpkin_deep",
    )
    .p(&[[0.3, 0.85], [0.38, 0.7], [0.44, 0.85]], "pumpkin_deep")
    .p(&[[0.58, 0.85], [0.66, 0.72], [0.72, 0.85]], "pumpkin_deep")
    .p(
        &[[0.4, 0.86], [0.6, 0.86], [0.62, 0.62], [0.38, 0.62]],
        "brick",
    )
    .e([0.5, 0.5], [0.17, 0.15], "moss");
    for (x, top, turn) in [
        (0.36, 0.12, -0.25_f32),
        (0.45, 0.05, -0.08),
        (0.55, 0.06, 0.08),
        (0.64, 0.14, 0.22),
    ] {
        c.path(
            &[
                [x, 0.45],
                [x + turn * 0.2, top + 0.15],
                [x + turn * 0.4, top],
            ],
            0.055,
            "moss",
        );
    }
    c.path(&[[0.33, 0.55], [0.22, 0.42], [0.18, 0.32]], 0.05, "moss");
    c
}

fn giant_spider() -> Card {
    let mut c = Card::new(1.0);
    for k in 0..8 {
        let a = k as f32 / 8.0 * std::f32::consts::TAU;
        let (s, co) = a.sin_cos();
        c.l([0.5, 0.5], [0.5 + co * 0.5, 0.5 + s * 0.5], 0.006, "dusk");
    }
    for r in [0.16_f32, 0.3, 0.44] {
        let pts: Vec<[f32; 2]> = (0..=8)
            .map(|k| {
                let a = k as f32 / 8.0 * std::f32::consts::TAU;
                [0.5 + a.cos() * r, 0.5 + a.sin() * r]
            })
            .collect();
        c.path(&pts, 0.006, "dusk");
    }
    for side in [-1.0_f32, 1.0] {
        for k in 0..4 {
            let y = 0.42 + k as f32 * 0.06;
            c.path(
                &[
                    [0.5, y],
                    [0.5 + side * 0.22, y - 0.12 + k as f32 * 0.02],
                    [0.5 + side * 0.36, y + 0.12 + k as f32 * 0.04],
                ],
                0.03,
                "licorice",
            );
        }
    }
    c.e([0.5, 0.58], [0.15, 0.17], "licorice")
        .e([0.5, 0.38], [0.09, 0.08], "licorice")
        .e([0.46, 0.37], [0.025, 0.025], "pumpkin")
        .e([0.54, 0.37], [0.025, 0.025], "pumpkin")
        .p(&[[0.45, 0.56], [0.55, 0.56], [0.5, 0.66]], "pumpkin");
    c
}

fn crow() -> Card {
    let mut c = Card::new(1.0);
    c.l([0.3, 0.95], [0.75, 0.95], 0.03, "pumpkin_deep")
        .e([0.48, 0.6], [0.26, 0.2], "licorice")
        .p(
            &[[0.26, 0.6], [0.02, 0.8], [0.12, 0.84], [0.34, 0.72]],
            "licorice",
        )
        .e([0.66, 0.36], [0.15, 0.14], "licorice")
        .p(&[[0.78, 0.33], [0.98, 0.4], [0.78, 0.42]], "mustard")
        .e([0.7, 0.32], [0.035, 0.035], "paper")
        .e([0.71, 0.32], [0.015, 0.015], "licorice")
        .l([0.45, 0.78], [0.42, 0.95], 0.02, "mustard")
        .l([0.55, 0.78], [0.58, 0.95], 0.02, "mustard");
    c
}

fn owl() -> Card {
    let mut c = Card::new(0.7);
    c.e([0.5, 0.58], [0.4, 0.38], "pumpkin_deep")
        .p(&[[0.16, 0.3], [0.2, 0.06], [0.34, 0.24]], "pumpkin_deep")
        .p(&[[0.84, 0.3], [0.8, 0.06], [0.66, 0.24]], "pumpkin_deep")
        .e([0.5, 0.68], [0.24, 0.24], "mustard")
        .e([0.34, 0.36], [0.14, 0.12], "paper")
        .e([0.66, 0.36], [0.14, 0.12], "paper")
        .e([0.34, 0.36], [0.07, 0.06], "licorice")
        .e([0.66, 0.36], [0.07, 0.06], "licorice")
        .p(&[[0.46, 0.44], [0.54, 0.44], [0.5, 0.53]], "pumpkin");
    for k in 0..3 {
        let y = 0.62 + k as f32 * 0.09;
        c.p(&[[0.42, y], [0.5, y + 0.05], [0.58, y]], "pumpkin_deep");
    }
    c.l([0.1, 0.98], [0.9, 0.98], 0.03, "licorice");
    c
}

fn scarecrow() -> Card {
    let mut c = Card::new(0.9);
    c.l([0.5, 0.25], [0.5, 1.0], 0.04, "pumpkin_deep")
        .l([0.05, 0.38], [0.95, 0.38], 0.035, "pumpkin_deep")
        .p(
            &[
                [0.1, 0.32],
                [0.9, 0.32],
                [0.82, 0.48],
                [0.68, 0.46],
                [0.66, 0.8],
                [0.34, 0.8],
                [0.32, 0.46],
                [0.18, 0.48],
            ],
            "brick",
        );
    for x in [0.25_f32, 0.42, 0.58, 0.75] {
        c.l([x, 0.33], [x, 0.47], 0.008, "mustard");
    }
    c.l([0.33, 0.6], [0.67, 0.6], 0.008, "mustard")
        .p(&[[0.04, 0.42], [0.1, 0.36], [0.12, 0.5]], "mustard")
        .p(&[[0.96, 0.42], [0.9, 0.36], [0.88, 0.5]], "mustard")
        .e([0.5, 0.2], [0.15, 0.13], "paper")
        .e([0.45, 0.18], [0.025, 0.025], "licorice")
        .e([0.55, 0.18], [0.025, 0.025], "licorice")
        .path(
            &[
                [0.42, 0.25],
                [0.46, 0.27],
                [0.5, 0.25],
                [0.54, 0.27],
                [0.58, 0.25],
            ],
            0.01,
            "licorice",
        )
        .e([0.5, 0.1], [0.24, 0.03], "pumpkin_deep")
        .p(
            &[[0.36, 0.1], [0.64, 0.1], [0.58, 0.0], [0.42, 0.0]],
            "pumpkin_deep",
        );
    c
}

fn pumpkin_man() -> Card {
    let mut c = Card::new(0.55);
    c.p(
        &[[0.32, 0.3], [0.68, 0.3], [0.66, 0.66], [0.34, 0.66]],
        "licorice",
    )
    .p(
        &[[0.34, 0.64], [0.48, 0.64], [0.45, 0.98], [0.34, 0.98]],
        "licorice",
    )
    .p(
        &[[0.52, 0.64], [0.66, 0.64], [0.66, 0.98], [0.55, 0.98]],
        "licorice",
    )
    .p(&[[0.45, 0.3], [0.55, 0.3], [0.5, 0.42]], "paper")
    .p(&[[0.47, 0.32], [0.53, 0.32], [0.5, 0.36]], "brick")
    // One arm waving.
    .path(&[[0.33, 0.33], [0.16, 0.24], [0.1, 0.08]], 0.05, "licorice")
    .path(&[[0.67, 0.33], [0.78, 0.5], [0.84, 0.62]], 0.05, "licorice")
    .e([0.1, 0.07], [0.04, 0.04], "paper")
    .e([0.85, 0.63], [0.04, 0.04], "paper")
    .e([0.5, 0.17], [0.2, 0.14], "pumpkin")
    .e([0.42, 0.17], [0.07, 0.13], "pumpkin_deep")
    .e([0.58, 0.17], [0.07, 0.13], "pumpkin_deep")
    .e([0.5, 0.17], [0.07, 0.14], "pumpkin")
    .l([0.5, 0.04], [0.52, -0.0], 0.025, "moss")
    .p(&[[0.4, 0.13], [0.46, 0.13], [0.43, 0.08]], "candle")
    .p(&[[0.54, 0.13], [0.6, 0.13], [0.57, 0.08]], "candle")
    .p(
        &[
            [0.38, 0.2],
            [0.62, 0.2],
            [0.58, 0.25],
            [0.5, 0.23],
            [0.42, 0.25],
        ],
        "candle",
    );
    c
}

fn headless_rider() -> Card {
    let mut c = Card::new(1.17);
    // A moon behind the silhouette.
    c.e([0.6, 0.45], [0.35, 0.42], "dusk")
        // The horse, rearing.
        .e([0.5, 0.62], [0.28, 0.13], "licorice")
        .p(
            &[
                [0.72, 0.58],
                [0.84, 0.3],
                [0.94, 0.28],
                [0.9, 0.38],
                [0.8, 0.6],
            ],
            "licorice",
        )
        .p(&[[0.86, 0.28], [0.88, 0.2], [0.91, 0.28]], "licorice")
        .path(
            &[[0.68, 0.68], [0.78, 0.82], [0.86, 0.78]],
            0.045,
            "licorice",
        )
        .path(&[[0.62, 0.7], [0.66, 0.88], [0.72, 0.9]], 0.045, "licorice")
        .path(
            &[[0.32, 0.68], [0.26, 0.86], [0.3, 0.98]],
            0.045,
            "licorice",
        )
        .path(&[[0.4, 0.7], [0.38, 0.88], [0.42, 0.98]], 0.045, "licorice")
        .path(&[[0.24, 0.58], [0.1, 0.62], [0.04, 0.74]], 0.04, "licorice")
        // The rider, no head, cape flying.
        .p(
            &[[0.42, 0.52], [0.56, 0.52], [0.58, 0.24], [0.42, 0.24]],
            "licorice",
        )
        .p(
            &[
                [0.42, 0.26],
                [0.1, 0.2],
                [0.2, 0.32],
                [0.04, 0.4],
                [0.4, 0.46],
            ],
            "licorice",
        )
        .path(&[[0.56, 0.3], [0.7, 0.38], [0.74, 0.32]], 0.035, "licorice")
        .e([0.49, 0.24], [0.06, 0.02], "brick");
    c
}

fn rat() -> Card {
    let mut c = Card::new(1.6);
    c.path(
        &[[0.18, 0.7], [0.08, 0.6], [0.04, 0.42], [0.1, 0.3]],
        0.03,
        "brick",
    )
    .e([0.45, 0.68], [0.28, 0.22], "plum")
    .e([0.73, 0.62], [0.17, 0.15], "plum")
    .p(&[[0.84, 0.58], [0.99, 0.68], [0.84, 0.72]], "plum")
    .e([0.98, 0.68], [0.025, 0.03], "brick")
    .e([0.66, 0.44], [0.07, 0.1], "plum")
    .e([0.66, 0.45], [0.04, 0.06], "brick")
    .e([0.78, 0.56], [0.025, 0.03], "mustard")
    .e([0.785, 0.56], [0.01, 0.02], "licorice")
    .l([0.9, 0.7], [0.99, 0.8], 0.006, "paper")
    .l([0.9, 0.66], [0.99, 0.6], 0.006, "paper")
    .e([0.35, 0.9], [0.05, 0.03], "brick")
    .e([0.6, 0.9], [0.05, 0.03], "brick");
    c
}

fn vampire() -> Card {
    let mut c = Card::new(0.62);
    // The cape, high collar, red inside.
    c.p(&[[0.5, 0.26], [0.02, 0.98], [0.98, 0.98]], "licorice")
        .p(&[[0.5, 0.34], [0.2, 0.96], [0.8, 0.96]], "brick")
        .p(
            &[[0.4, 0.34], [0.6, 0.34], [0.64, 0.98], [0.36, 0.98]],
            "licorice",
        )
        .p(&[[0.44, 0.34], [0.56, 0.34], [0.5, 0.5]], "paper")
        .p(&[[0.47, 0.36], [0.53, 0.36], [0.5, 0.41]], "brick")
        .p(&[[0.2, 0.36], [0.3, 0.12], [0.42, 0.32]], "licorice")
        .p(&[[0.8, 0.36], [0.7, 0.12], [0.58, 0.32]], "licorice")
        // Face with a widow's peak and fangs.
        .e([0.5, 0.22], [0.13, 0.13], "dusk")
        .p(
            &[
                [0.36, 0.17],
                [0.5, 0.06],
                [0.64, 0.17],
                [0.64, 0.08],
                [0.5, 0.02],
                [0.36, 0.08],
            ],
            "licorice",
        )
        .p(&[[0.39, 0.18], [0.47, 0.2], [0.39, 0.21]], "licorice")
        .p(&[[0.61, 0.18], [0.53, 0.2], [0.61, 0.21]], "licorice")
        .e([0.43, 0.21], [0.015, 0.012], "brick")
        .e([0.57, 0.21], [0.015, 0.012], "brick")
        .p(&[[0.44, 0.28], [0.56, 0.28], [0.5, 0.3]], "licorice")
        .p(&[[0.46, 0.28], [0.48, 0.28], [0.47, 0.31]], "paper")
        .p(&[[0.52, 0.28], [0.54, 0.28], [0.53, 0.31]], "paper");
    c
}

/// The card for asset `id`, or `None` when it isn't a card.
#[must_use]
pub fn draw(id: &str) -> Option<Image> {
    let card = match id {
        "black-cat" => black_cat(),
        "werewolf" => werewolf(),
        "sheet-ghost" => sheet_ghost(),
        "bat-swarm" => bat_swarm(),
        "witch" => witch(),
        "skeleton" => skeleton(),
        "mummy" => mummy(),
        "zombie-hand" => zombie_hand(),
        "giant-spider" => giant_spider(),
        "crow" => crow(),
        "owl" => owl(),
        "scarecrow" => scarecrow(),
        "pumpkin-man" => pumpkin_man(),
        "headless-rider" => headless_rider(),
        "rat" => rat(),
        "vampire" => vampire(),
        _ => return None,
    };
    Some(rasterise(&card))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Kind, catalog};

    #[test]
    fn every_card_is_drawn_tall_in_palette_colours_with_a_licorice_border() {
        for asset in catalog().assets.iter().filter(|a| a.kind == Kind::Card) {
            let image = draw(&asset.id).unwrap_or_else(|| panic!("no card for {}", asset.id));
            assert_eq!(image.height, HEIGHT, "{}", asset.id);
            assert_eq!(image.palette.len(), PALETTE.len() + 1);
            let (w, h) = (usize::from(image.width), usize::from(image.height));
            // The corners are see-through, and the figure is there.
            assert_eq!(image.pixels[0], 0, "{}", asset.id);
            assert_eq!(image.pixels[w * h - 1], 0, "{}", asset.id);
            let filled = image.pixels.iter().filter(|p| **p != 0).count();
            assert!(filled > w * h / 6, "{} is nearly empty", asset.id);
            // Every edge of the cut-out is licorice: no figure pixel
            // touches the transparent ground.
            let licorice = ix("licorice");
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    let p = image.pixels[y * w + x];
                    if p == 0 || p == licorice {
                        continue;
                    }
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let q =
                            image.pixels[(y as i32 + dy) as usize * w + (x as i32 + dx) as usize];
                        assert_ne!(q, 0, "{} has an open edge at {x},{y}", asset.id);
                    }
                }
            }
            // It packs small.
            let bytes = icecream_zone::image::encode(&image);
            assert!(
                bytes.len() < 60_000,
                "{} packs to {}",
                asset.id,
                bytes.len()
            );
        }
        assert!(draw("armchair").is_none());
    }
}
