//! The pointer the compositor draws.
//!
//! A client that sets a cursor surface over its window draws its own, and
//! the compositor places that surface at the pointer. Everywhere else the
//! compositor draws the `default` image of the xcursor theme the session
//! names in `XCURSOR_THEME` and `XCURSOR_SIZE`, and an arrow of its own when
//! no theme answers, so a TTY with no theme installed still shows where the
//! pointer is.
//!
//! The nested backend draws the same pointer and hides the host session's
//! pointer over its window, so a cursor a client sets looks the same on
//! both backends.

use std::io::Read;

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::utils::{Logical, Point, Transform};
use xcursor::CursorTheme;
use xcursor::parser::{Image, parse_xcursor};

/// The nominal size a theme's image is picked at when `XCURSOR_SIZE` names
/// none.
pub const DEFAULT_SIZE: u32 = 24;

/// The theme and size a session names, read from its environment.
pub fn named(value: impl Fn(&str) -> Option<String>) -> (String, u32) {
    let theme = value("XCURSOR_THEME")
        .filter(|held| !held.is_empty())
        .unwrap_or_else(|| "default".to_string());
    let size = value("XCURSOR_SIZE")
        .and_then(|held| held.parse().ok())
        .filter(|size| *size > 0)
        .unwrap_or(DEFAULT_SIZE);
    (theme, size)
}

/// The images of one cursor, and the buffers the renderer draws them from.
pub struct Cursor {
    images: Vec<Image>,
    size: u32,
    buffers: Vec<(Image, MemoryRenderBuffer)>,
}

impl Cursor {
    /// The cursor the session's environment names.
    pub fn load() -> Cursor {
        let (theme, size) = named(|name| std::env::var(name).ok());
        let images = match load_theme(&theme) {
            Ok(images) => images,
            Err(err) => {
                log::info!(
                    "the xcursor theme {theme} did not load ({err}), so the compositor draws its own arrow"
                );
                vec![arrow(size)]
            }
        };
        Cursor::from_images(images, size)
    }

    /// A cursor made of these images, picked at this nominal size.
    pub fn from_images(images: Vec<Image>, size: u32) -> Cursor {
        let images = if images.is_empty() {
            vec![arrow(size)]
        } else {
            images
        };
        Cursor {
            images,
            size,
            buffers: Vec::new(),
        }
    }

    /// The buffer to draw on a screen at `scale`, `millis` into an animated
    /// cursor, and the hotspot in logical pixels.
    pub fn buffer(&mut self, scale: f64, millis: u32) -> (MemoryRenderBuffer, Point<i32, Logical>) {
        let factor = buffer_scale(scale);
        let image = pick(&self.images, self.size * factor as u32, millis)
            .cloned()
            .unwrap_or_else(|| arrow(self.size * factor as u32));
        let hotspot = Point::from((image.xhot as i32 / factor, image.yhot as i32 / factor));
        if let Some((_, buffer)) = self.buffers.iter().find(|(held, _)| *held == image) {
            return (buffer.clone(), hotspot);
        }
        let buffer = MemoryRenderBuffer::from_slice(
            &image.pixels_rgba,
            Fourcc::Argb8888,
            (image.width as i32, image.height as i32),
            factor,
            Transform::Normal,
            None,
        );
        self.buffers.push((image, buffer.clone()));
        (buffer, hotspot)
    }
}

/// The whole-number scale a cursor image is drawn at on a screen of this
/// scale: the image is picked that many times larger than its nominal size
/// and drawn that many times smaller, so a fractional screen gets the
/// sharper image.
pub fn buffer_scale(scale: f64) -> i32 {
    if scale.is_finite() && scale > 1.0 {
        scale.ceil() as i32
    } else {
        1
    }
}

/// The image nearest a nominal size, stepped through by delay when the
/// cursor animates, or nothing when there are no images.
pub fn pick(images: &[Image], size: u32, millis: u32) -> Option<&Image> {
    let nearest = images
        .iter()
        .min_by_key(|image| (i64::from(size) - i64::from(image.size)).abs())?;
    let frames: Vec<&Image> = images
        .iter()
        .filter(|image| image.width == nearest.width && image.height == nearest.height)
        .collect();
    let total: u32 = frames.iter().map(|image| image.delay).sum();
    if total == 0 {
        return Some(nearest);
    }
    let mut left = millis % total;
    for image in frames {
        if left < image.delay {
            return Some(image);
        }
        left -= image.delay;
    }
    Some(nearest)
}

fn load_theme(theme: &str) -> Result<Vec<Image>, String> {
    let path = CursorTheme::load(theme)
        .load_icon("default")
        .ok_or("the theme has no default cursor")?;
    let mut bytes = Vec::new();
    std::fs::File::open(&path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|err| format!("{}: {err}", path.display()))?;
    parse_xcursor(&bytes).ok_or_else(|| format!("{} is not an xcursor file", path.display()))
}

/// The arrow the compositor draws when no theme answers: white with a
/// black edge, its tip at the top left corner, in the byte order a theme's
/// images carry.
pub fn arrow(size: u32) -> Image {
    let side = size.max(8);
    let mut pixels = Vec::with_capacity((side * side * 4) as usize);
    for y in 0..side {
        for x in 0..side {
            let inside = arrow_holds(side, x, y);
            let edge = inside
                && (x == 0
                    || !arrow_holds(side, x + 1, y)
                    || !arrow_holds(side, x, y + 1)
                    || y == 0);
            let (value, alpha) = match (inside, edge) {
                (false, _) => (0, 0),
                (true, true) => (0, 255),
                (true, false) => (255, 255),
            };
            pixels.extend_from_slice(&[value, value, value, alpha]);
        }
    }
    Image {
        size: side,
        width: side,
        height: side,
        xhot: 0,
        yhot: 0,
        delay: 0,
        pixels_argb: Vec::new(),
        pixels_rgba: pixels,
    }
}

/// Whether one pixel of an arrow `side` pixels across is inside it. The
/// arrow is the triangle between the tip, the point straight below it, and
/// the point down and to the right of it at 45 degrees.
fn arrow_holds(side: u32, x: u32, y: u32) -> bool {
    let reach = side * 3 / 4;
    y < reach && x <= y && x + y / 2 < reach
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(size: u32, delay: u32, mark: u8) -> Image {
        Image {
            size,
            width: size,
            height: size,
            xhot: 4,
            yhot: 2,
            delay,
            pixels_rgba: vec![mark; (size * size * 4) as usize],
            pixels_argb: Vec::new(),
        }
    }

    #[test]
    fn the_session_names_the_theme_and_the_size() {
        let (theme, size) = named(|name| match name {
            "XCURSOR_THEME" => Some("Adwaita".to_string()),
            "XCURSOR_SIZE" => Some("32".to_string()),
            _ => None,
        });
        assert_eq!((theme.as_str(), size), ("Adwaita", 32));
        let (theme, size) = named(|name| match name {
            "XCURSOR_SIZE" => Some("huge".to_string()),
            _ => None,
        });
        assert_eq!((theme.as_str(), size), ("default", DEFAULT_SIZE));
    }

    #[test]
    fn the_arrow_has_its_tip_at_the_hotspot_and_nothing_in_the_far_corner() {
        let arrow = arrow(24);
        assert_eq!(arrow.pixels_rgba.len(), 24 * 24 * 4);
        assert_eq!((arrow.xhot, arrow.yhot), (0, 0));
        assert_eq!(arrow.pixels_rgba[3], 255, "the tip is opaque");
        let far = ((24 * 24 - 1) * 4 + 3) as usize;
        assert_eq!(arrow.pixels_rgba[far], 0, "the far corner is clear");
        let filled = (5 * 24 + 2) * 4;
        assert_eq!(arrow.pixels_rgba[filled], 255, "the body is white");
    }

    #[test]
    fn a_fractional_screen_draws_the_next_whole_size_up() {
        assert_eq!(buffer_scale(1.0), 1);
        assert_eq!(buffer_scale(1.25), 2);
        assert_eq!(buffer_scale(2.0), 2);
        assert_eq!(buffer_scale(0.5), 1);
        assert_eq!(buffer_scale(f64::NAN), 1);
    }

    #[test]
    fn the_image_nearest_the_size_is_picked() {
        let images = vec![image(24, 0, 1), image(48, 0, 2)];
        assert_eq!(pick(&images, 24, 0).map(|image| image.size), Some(24));
        assert_eq!(pick(&images, 40, 0).map(|image| image.size), Some(48));
    }

    #[test]
    fn an_animated_cursor_steps_through_its_frames_by_delay() {
        let images = vec![image(24, 100, 1), image(24, 50, 2)];
        assert_eq!(
            pick(&images, 24, 0).map(|image| image.pixels_rgba[0]),
            Some(1)
        );
        assert_eq!(
            pick(&images, 24, 120).map(|image| image.pixels_rgba[0]),
            Some(2)
        );
        assert_eq!(
            pick(&images, 24, 160).map(|image| image.pixels_rgba[0]),
            Some(1)
        );
    }

    #[test]
    fn a_doubled_cursor_halves_its_hotspot_and_makes_its_buffer_once() {
        let mut cursor = Cursor::from_images(vec![image(48, 0, 9)], 24);
        let (_, hotspot) = cursor.buffer(2.0, 0);
        assert_eq!((hotspot.x, hotspot.y), (2, 1));
        let (_, again) = cursor.buffer(2.0, 0);
        assert_eq!(again, hotspot);
        assert_eq!(cursor.buffers.len(), 1, "the buffer is made once");
        let empty = Cursor::from_images(Vec::new(), 24);
        assert_eq!(empty.images.len(), 1);
        assert!(pick(&[], 24, 0).is_none());
    }
}
