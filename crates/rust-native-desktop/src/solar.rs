//! Embedded Solar artwork, rasterized once per pixel size and tinted at paint time.
use crate::{Frame, PxRect};
use rust_native::{Glyph, style::Color};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

macro_rules! asset {
    ($name:literal) => {
        include_str!(concat!("../assets/solar/", $name, ".svg"))
    };
}
const COPY: &str = asset!("copy");
fn asset(glyph: Glyph) -> Option<&'static str> {
    Some(match glyph {
        Glyph::Back => asset!("arrow-left"),
        Glyph::Compose => asset!("pen-new-square"),
        Glyph::Edit => asset!("pen"),
        Glyph::Menu => asset!("sidebar-minimalistic-left"),
        Glyph::History => asset!("clock-circle"),
        Glyph::Folder => asset!("folder"),
        Glyph::Computer => asset!("monitor"),
        Glyph::Cloud => asset!("cloud"),
        Glyph::Add => asset!("add-circle"),
        Glyph::ArrowUp => asset!("arrow-up"),
        Glyph::Paperclip => asset!("paperclip"),
        Glyph::Clipboard => COPY,
        Glyph::Search => asset!("magnifer"),
        Glyph::Settings => asset!("settings"),
        Glyph::Pin => asset!("pin"),
        Glyph::Archive => asset!("archive-minimalistic"),
        Glyph::Restore => asset!("archive-up-minimalistic"),
        Glyph::More => asset!("more-horizontal"),
        Glyph::Check | Glyph::Checked => asset!("check"),
        Glyph::Ask => asset!("chat-round-line"),
        Glyph::Key => asset!("key-minimalistic"),
        _ => return None,
    })
}
#[derive(Default)]
struct Cache {
    masks: HashMap<(usize, u32, u32), Rc<Vec<u8>>>,
    bytes: usize,
}
thread_local! { static CACHE: RefCell<Cache> = RefCell::new(Cache::default()); }
fn mask(source: &'static str, width: u32, height: u32) -> Option<Rc<Vec<u8>>> {
    CACHE.with(|cache| {
        let key = (source.as_ptr() as usize, width, height);
        let mut cache = cache.borrow_mut();
        if let Some(mask) = cache.masks.get(&key) {
            return Some(mask.clone());
        }
        let source = source
            .replace("1em", "24")
            .replace("currentColor", "#ffffff");
        let tree = resvg::usvg::Tree::from_str(&source, &resvg::usvg::Options::default()).ok()?;
        let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(
                width as f32 / tree.size().width(),
                height as f32 / tree.size().height(),
            ),
            &mut pixmap.as_mut(),
        );
        let mask = Rc::new(
            pixmap
                .data()
                .chunks_exact(4)
                .map(|p| p[3])
                .collect::<Vec<_>>(),
        );
        if cache.masks.len() >= 128 || cache.bytes + mask.len() > 2 * 1024 * 1024 {
            cache.masks.clear();
            cache.bytes = 0;
        }
        cache.bytes += mask.len();
        cache.masks.insert(key, mask.clone());
        Some(mask)
    })
}
fn draw(frame: &mut Frame, rect: PxRect, source: &'static str, color: Color) -> bool {
    let width = rect.w.round();
    let height = rect.h.round();
    if !width.is_finite()
        || !height.is_finite()
        || !(1.0..=256.0).contains(&width)
        || !(1.0..=256.0).contains(&height)
    {
        return false;
    }
    let (width, height) = (width as u32, height as u32);
    let Some(mask) = mask(source, width, height) else {
        return false;
    };
    for (index, alpha) in mask.iter().enumerate().filter(|(_, alpha)| **alpha != 0) {
        frame.blend(
            rect.x.round() as i64 + (index % width as usize) as i64,
            rect.y.round() as i64 + (index / width as usize) as i64,
            color,
            f32::from(*alpha) / 255.0,
        );
    }
    true
}
pub(crate) fn draw_glyph(frame: &mut Frame, rect: PxRect, glyph: Glyph, color: Color) -> bool {
    if glyph == Glyph::Stop {
        frame.fill(rect, 3.0 * rect.w / 11.0, color);
        return true;
    }
    asset(glyph).is_some_and(|source| draw(frame, rect, source, color))
}
pub(crate) fn draw_copy(frame: &mut Frame, rect: PxRect, color: Color) {
    draw(frame, rect, COPY, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embedded_vectors_render_and_preserve_the_damage_clip() {
        for glyph in [
            Glyph::Edit,
            Glyph::Search,
            Glyph::Settings,
            Glyph::Pin,
            Glyph::Archive,
            Glyph::Restore,
            Glyph::Stop,
            Glyph::Back,
            Glyph::Compose,
            Glyph::Menu,
            Glyph::History,
            Glyph::Folder,
            Glyph::Computer,
            Glyph::Cloud,
            Glyph::Add,
            Glyph::ArrowUp,
            Glyph::Paperclip,
            Glyph::Clipboard,
            Glyph::More,
            Glyph::Check,
            Glyph::Ask,
            Glyph::Key,
        ] {
            for scale in [1.0, 2.0] {
                let side = (24.0 * scale) as usize;
                let mut frame = Frame::transparent(side, side);
                let clip = PxRect {
                    x: 0.0,
                    y: 0.0,
                    w: side as f32 / 2.0,
                    h: side as f32,
                };
                frame.clip_to(clip);
                assert!(draw_glyph(
                    &mut frame,
                    PxRect {
                        x: 0.0,
                        y: 0.0,
                        w: side as f32,
                        h: side as f32
                    },
                    glyph,
                    Color::rgb(163, 163, 163)
                ));
                assert!(frame.pixels.chunks_exact(4).any(|p| p[3] != 0), "{glyph:?}");
                for y in 0..side {
                    for x in side / 2..side {
                        assert_eq!(frame.pixels[(y * side + x) * 4 + 3], 0);
                    }
                }
            }
        }
    }
}
