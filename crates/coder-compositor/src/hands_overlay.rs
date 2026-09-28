//! The overlay that shows the tracked hand: the 21 landmarks, the bones
//! between them, and the gesture the desk read with its margin.
//!
//! The compositor draws it over every window on the focused screen, in
//! the same mirrored mapping the pointer uses, so the index tip sits where
//! the pointer is. The picture is a small ARGB raster the renderer draws
//! as a memory buffer, the way it draws the pointer, so no client sees it
//! and a screen copy records it. The text is a five-by-seven bitmap face
//! held here, because the compositor draws no other text.

use coder_hands::gestures::{Label, to_screen};
use coder_hands::{CONNECTIONS, INDEX_TIP, Landmark, THUMB_TIP};

use crate::layout;

/// The pixels around the hand the raster keeps, so a thick bone at the
/// edge is whole.
const PAD: i32 = 14;
/// The bones' thickness in pixels.
const BONE: i32 = 3;
/// A joint's radius in pixels.
const JOINT: i32 = 4;
/// The index tip's radius in pixels.
const TIP: i32 = 7;
/// How many pixels one bit of the face takes.
const TEXT_SCALE: i32 = 3;
/// The pixels between the label and the screen's corner.
const MARGIN: i32 = 16;
/// The pixels of dark ground around the label.
const INSET: i32 = 8;
/// One glyph's width and height in bits, and the gap between glyphs.
const GLYPH_W: i32 = 5;
const GLYPH_H: i32 = 7;
const GAP: i32 = 1;

/// A screen in the shared space, in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Screen {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// One raster the renderer draws at a place in the shared space:
/// premultiplied `ARGB8888` in memory order, `B`, `G`, `R`, `A`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u8>,
}

impl Picture {
    fn blank(x: i32, y: i32, width: i32, height: i32) -> Picture {
        let width = width.max(1);
        let height = height.max(1);
        Picture {
            x,
            y,
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
        }
    }

    fn put(&mut self, x: i32, y: i32, color: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return;
        }
        let at = ((y * self.width + x) * 4) as usize;
        self.pixels[at..at + 4].copy_from_slice(&color);
    }

    fn disc(&mut self, cx: i32, cy: i32, radius: i32, color: [u8; 4]) {
        for y in -radius..=radius {
            for x in -radius..=radius {
                if x * x + y * y <= radius * radius {
                    self.put(cx + x, cy + y, color);
                }
            }
        }
    }

    fn line(&mut self, from: (i32, i32), to: (i32, i32), thickness: i32, color: [u8; 4]) {
        let (mut x, mut y) = from;
        let dx = (to.0 - from.0).abs();
        let dy = -(to.1 - from.1).abs();
        let sx = if from.0 < to.0 { 1 } else { -1 };
        let sy = if from.1 < to.1 { 1 } else { -1 };
        let mut err = dx + dy;
        let radius = thickness / 2;
        loop {
            self.disc(x, y, radius, color);
            if x == to.0 && y == to.1 {
                break;
            }
            let twice = 2 * err;
            if twice >= dy {
                err += dy;
                x += sx;
            }
            if twice <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    fn fill(&mut self, color: [u8; 4]) {
        for px in self.pixels.chunks_exact_mut(4) {
            px.copy_from_slice(&color);
        }
    }

    fn text(&mut self, x: i32, y: i32, text: &str, color: [u8; 4]) {
        let mut pen = x;
        for c in text.chars() {
            let rows = glyph(c);
            for (row, bits) in rows.iter().enumerate() {
                for column in 0..GLYPH_W {
                    if bits & (1 << (GLYPH_W - 1 - column)) != 0 {
                        for sy in 0..TEXT_SCALE {
                            for sx in 0..TEXT_SCALE {
                                self.put(
                                    pen + column * TEXT_SCALE + sx,
                                    y + row as i32 * TEXT_SCALE + sy,
                                    color,
                                );
                            }
                        }
                    }
                }
            }
            pen += (GLYPH_W + GAP) * TEXT_SCALE;
        }
    }
}

/// A color as the raster holds it, premultiplied by its alpha.
fn color(rgba: [f32; 4]) -> [u8; 4] {
    let channel =
        |value: f32| (value.clamp(0.0, 1.0) * rgba[3].clamp(0.0, 1.0) * 255.0).round() as u8;
    [
        channel(rgba[2]),
        channel(rgba[1]),
        channel(rgba[0]),
        (rgba[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

/// The pixels `text` takes at the face's scale.
fn text_size(text: &str) -> (i32, i32) {
    let count = text.chars().count() as i32;
    (
        (count * (GLYPH_W + GAP) - GAP).max(0) * TEXT_SCALE,
        GLYPH_H * TEXT_SCALE,
    )
}

/// What the overlay says under the hand: the label and its margin, or
/// what holds it back.
pub fn caption(label: Label, margin: f32, holding: bool, status: Option<&str>) -> String {
    if let Some(status) = status {
        return status.to_string();
    }
    match (label, holding) {
        (_, true) => format!("pinch held {margin:+.2}"),
        (Label::None, false) => "hands: no gesture".to_string(),
        (label, false) => format!("{} {margin:+.2}", label.word()),
    }
}

/// The pictures the overlay draws on one screen: the hand, when one is
/// tracked, and the caption in the screen's bottom left corner.
pub fn draw(
    screen: Screen,
    hand: Option<&[Landmark; 21]>,
    label: Label,
    margin: f32,
    holding: bool,
    status: Option<&str>,
) -> Vec<Picture> {
    let mut pictures = Vec::new();
    if let Some(hand) = hand {
        pictures.push(skeleton(screen, hand, holding));
    }
    pictures.push(label_picture(
        screen,
        &caption(label, margin, holding, status),
    ));
    pictures
}

/// The hand's bones and joints, at the place on the screen the pointer
/// mapping puts them.
fn skeleton(screen: Screen, hand: &[Landmark; 21], holding: bool) -> Picture {
    let points: Vec<(i32, i32)> = hand
        .iter()
        .map(|point| {
            let (sx, sy) = to_screen((point.x, point.y));
            (
                screen.x + (sx * screen.width as f32).round() as i32,
                screen.y + (sy * screen.height as f32).round() as i32,
            )
        })
        .collect();
    let left = points.iter().map(|p| p.0).min().unwrap_or(0) - PAD;
    let top = points.iter().map(|p| p.1).min().unwrap_or(0) - PAD;
    let right = points.iter().map(|p| p.0).max().unwrap_or(0) + PAD;
    let bottom = points.iter().map(|p| p.1).max().unwrap_or(0) + PAD;
    let mut picture = Picture::blank(left, top, right - left, bottom - top);
    let bone = color(layout::BORDER_ACTIVE);
    let joint = color([1.0, 1.0, 1.0, 0.9]);
    let tip = if holding {
        color([1.0, 1.0, 1.0, 1.0])
    } else {
        color(layout::BORDER_ACTIVE)
    };
    let local = |i: usize| (points[i].0 - left, points[i].1 - top);
    for [a, b] in CONNECTIONS {
        picture.line(local(*a), local(*b), BONE, bone);
    }
    for i in 0..21 {
        let (x, y) = local(i);
        picture.disc(x, y, JOINT, joint);
    }
    let (x, y) = local(INDEX_TIP);
    picture.disc(x, y, TIP, tip);
    if holding {
        let (x, y) = local(THUMB_TIP);
        picture.disc(x, y, TIP, tip);
    }
    picture
}

/// The caption on a dark ground in the screen's bottom left corner.
fn label_picture(screen: Screen, text: &str) -> Picture {
    let (w, h) = text_size(text);
    let width = w + 2 * INSET;
    let height = h + 2 * INSET;
    let mut picture = Picture::blank(
        screen.x + MARGIN,
        screen.y + screen.height - MARGIN - height,
        width,
        height,
    );
    picture.fill(color([
        layout::BACKGROUND[0],
        layout::BACKGROUND[1],
        layout::BACKGROUND[2],
        0.85,
    ]));
    picture.text(INSET, INSET, text, color(layout::BORDER_ACTIVE));
    picture
}

/// One glyph of the face, seven rows of five bits with the leftmost bit
/// highest. A character the face lacks is blank.
fn glyph(c: char) -> [u8; 7] {
    match c {
        'a' => [
            0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111,
        ],
        'b' => [
            0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b11110,
        ],
        'c' => [
            0b00000, 0b00000, 0b01110, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'd' => [
            0b00001, 0b00001, 0b01101, 0b10011, 0b10001, 0b10001, 0b01111,
        ],
        'e' => [
            0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110,
        ],
        'f' => [
            0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000,
        ],
        'g' => [
            0b00000, 0b01111, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110,
        ],
        'h' => [
            0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001,
        ],
        'i' => [
            0b00100, 0b00000, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'j' => [
            0b00010, 0b00000, 0b00110, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        'k' => [
            0b10000, 0b10000, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010,
        ],
        'l' => [
            0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        'm' => [
            0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10001, 0b10001,
        ],
        'n' => [
            0b00000, 0b00000, 0b10110, 0b11001, 0b10001, 0b10001, 0b10001,
        ],
        'o' => [
            0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'p' => [
            0b00000, 0b00000, 0b11110, 0b10001, 0b11110, 0b10000, 0b10000,
        ],
        'q' => [
            0b00000, 0b00000, 0b01101, 0b10011, 0b01111, 0b00001, 0b00001,
        ],
        'r' => [
            0b00000, 0b00000, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000,
        ],
        's' => [
            0b00000, 0b00000, 0b01110, 0b10000, 0b01110, 0b00001, 0b11110,
        ],
        't' => [
            0b01000, 0b01000, 0b11100, 0b01000, 0b01000, 0b01001, 0b00110,
        ],
        'u' => [
            0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101,
        ],
        'v' => [
            0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'w' => [
            0b00000, 0b00000, 0b10001, 0b10001, 0b10101, 0b10101, 0b01010,
        ],
        'x' => [
            0b00000, 0b00000, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001,
        ],
        'y' => [
            0b00000, 0b00000, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110,
        ],
        'z' => [
            0b00000, 0b00000, 0b11111, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        '+' => [
            0b00000, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00000,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],
        '.' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100,
        ],
        ':' => [
            0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000,
        ],
        ',' => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b00100, 0b01000,
        ],
        _ => [0; 7],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_hands::WRIST;

    fn screen() -> Screen {
        Screen {
            x: 0,
            y: 0,
            width: 1280,
            height: 800,
        }
    }

    fn hand_at(x: f32, y: f32) -> [Landmark; 21] {
        let mut hand = [Landmark { x, y, z: 1.0 }; 21];
        hand[INDEX_TIP] = Landmark {
            x: x + 0.02,
            y: y - 0.08,
            z: 1.0,
        };
        hand[WRIST] = Landmark {
            x,
            y: y + 0.08,
            z: 1.0,
        };
        hand
    }

    fn pixel(picture: &Picture, x: i32, y: i32) -> [u8; 4] {
        let at = ((y * picture.width + x) * 4) as usize;
        let mut px = [0; 4];
        px.copy_from_slice(&picture.pixels[at..at + 4]);
        px
    }

    #[test]
    fn the_hand_draws_where_the_pointer_mapping_puts_it() {
        let hand = hand_at(0.5, 0.5);
        let pictures = draw(screen(), Some(&hand), Label::Point, 0.4, false, None);
        assert_eq!(pictures.len(), 2);
        let skeleton = &pictures[0];
        // The wrist at the frame's middle is mirrored to the screen's
        // middle, and the raster's box holds it.
        let (sx, sy) = to_screen((0.5, 0.58));
        let wrist = ((sx * 1280.0).round() as i32, (sy * 800.0).round() as i32);
        assert!(skeleton.x <= wrist.0 && wrist.0 < skeleton.x + skeleton.width);
        assert!(skeleton.y <= wrist.1 && wrist.1 < skeleton.y + skeleton.height);
        let px = pixel(skeleton, wrist.0 - skeleton.x, wrist.1 - skeleton.y);
        assert_eq!(px[3], 230, "a joint draws at the wrist: {px:?}");
        // The index tip, to the right of the wrist in the frame, is to
        // the left of it on the screen.
        let (tx, _) = to_screen((0.52, 0.42));
        assert!(tx < sx);
    }

    #[test]
    fn a_missing_hand_draws_the_caption_alone() {
        let pictures = draw(screen(), None, Label::None, 0.0, false, None);
        assert_eq!(pictures.len(), 1);
        let caption = &pictures[0];
        assert_eq!(caption.x, MARGIN);
        assert_eq!(caption.y + caption.height, 800 - MARGIN);
        let ground = pixel(caption, 1, 1);
        assert_eq!(ground[3], 217, "the ground is dark and mostly opaque");
    }

    #[test]
    fn the_caption_names_the_label_and_its_margin() {
        assert_eq!(caption(Label::Point, 0.4173, false, None), "point +0.42");
        assert_eq!(caption(Label::Pinch, -0.05, true, None), "pinch held -0.05");
        assert_eq!(caption(Label::None, 0.0, false, None), "hands: no gesture");
        assert_eq!(
            caption(
                Label::Point,
                0.4,
                false,
                Some("hands: waiting for the camera daemon")
            ),
            "hands: waiting for the camera daemon"
        );
    }

    #[test]
    fn a_glyph_has_ink_and_a_stranger_has_none() {
        let mut picture = Picture::blank(0, 0, 40, 30);
        picture.text(0, 0, "a", [255, 255, 255, 255]);
        assert!(picture.pixels.iter().any(|byte| *byte != 0));
        let mut blank = Picture::blank(0, 0, 40, 30);
        blank.text(0, 0, "\u{e9}", [255, 255, 255, 255]);
        assert!(blank.pixels.iter().all(|byte| *byte == 0));
        assert_eq!(text_size("ab"), (11 * TEXT_SCALE, 7 * TEXT_SCALE));
    }

    #[test]
    fn a_color_is_premultiplied_in_memory_order() {
        assert_eq!(color([1.0, 0.5, 0.0, 1.0]), [0, 128, 255, 255]);
        assert_eq!(color([1.0, 1.0, 1.0, 0.5]), [128, 128, 128, 128]);
    }
}
