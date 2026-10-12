// SPDX-FileCopyrightText: 2026 OpenAgents
// SPDX-License-Identifier: MIT

//! Layers a film composes from the product: text set in Paper Mono as vector
//! paths, desktop components (panels, buttons, chat rows) painted with the
//! Coder Light and Coder Noir tokens, and Verse content as image layers
//! (a zone capture, or any PNG or JPEG). Each builder uploads into a
//! [`Film`] and returns the records that draw it, which a scene places with
//! [`place`] and shows in a frame.

use std::collections::HashMap;

use oa_tokens::{Resolver, Rgba, Scheme};
use swash::zeno::Verb;
use swash::{FontRef, GlyphId, scale::ScaleContext};

use crate::take::{Error, Film, Kind, Record};

/// The palette a desktop component is painted with, from the token table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub background: [f32; 4],
    pub surface: [f32; 4],
    pub border: [f32; 4],
    pub text: [f32; 4],
    pub text_secondary: [f32; 4],
    pub accent: [f32; 4],
    pub on_accent: [f32; 4],
}

fn color(rgba: Rgba) -> [f32; 4] {
    [rgba.r as f32, rgba.g as f32, rgba.b as f32, rgba.a as f32]
}

impl Theme {
    /// Coder Light (`Scheme::Light`) or Coder Noir (`Scheme::Dark`).
    pub fn from_tokens(scheme: Scheme) -> Self {
        let resolver = Resolver::new();
        let pick = |name: &str, fallback: u32| {
            color(
                resolver
                    .color(name, scheme)
                    .unwrap_or_else(|| Rgba::opaque(fallback)),
            )
        };
        Self {
            background: pick("--color-surface", 0xffffff),
            surface: pick("--color-surface-elevated", 0xf7f7f7),
            border: pick("--color-border", 0xe0e0e0),
            text: pick("--color-text", 0x181818),
            text_secondary: pick("--color-text-secondary", 0x6b6b6b),
            accent: pick("--color-background-primary-solid", 0x181818),
            on_accent: pick("--color-text-inverse", 0xffffff),
        }
    }

    pub fn light() -> Self {
        Self::from_tokens(Scheme::Light)
    }

    pub fn noir() -> Self {
        Self::from_tokens(Scheme::Dark)
    }
}

/// Paper Mono, scaled once per size, with each glyph uploaded once.
pub struct Font {
    context: ScaleContext,
    glyphs: HashMap<(GlyphId, u32), Option<(u64, f32)>>,
}

impl Default for Font {
    fn default() -> Self {
        Self::new()
    }
}

impl Font {
    pub fn new() -> Self {
        Self {
            context: ScaleContext::new(),
            glyphs: HashMap::new(),
        }
    }

    fn font() -> FontRef<'static> {
        FontRef::from_index(paper_mono::VARIABLE, 0).expect("Paper Mono parses")
    }

    /// Lays out `text` on one line in the view units of `size` (the em), its
    /// baseline at `(x, y)`, and returns the glyph records plus the advance.
    pub fn text(
        &mut self,
        film: &mut Film,
        text: &str,
        size: f32,
        color: [f32; 4],
        x: f32,
        y: f32,
        depth: f32,
    ) -> Result<(Vec<Record>, f32), Error> {
        let font = Self::font();
        let charmap = font.charmap();
        // Outlines scale at 1000 px per em so the key does not depend on size.
        let units = 1000.0;
        let metrics = font.glyph_metrics(&[]).scale(units);
        let mut scaler = self
            .context
            .builder(font)
            .size(units)
            .hint(false)
            .variations([("wght", 500.0)])
            .build();
        let scale = size / units;
        let mut records = Vec::new();
        let mut pen = 0.0f32;
        for ch in text.chars() {
            let id = charmap.map(ch);
            let entry = match self.glyphs.get(&(id, 0)) {
                Some(entry) => *entry,
                None => {
                    let outline = scaler.scale_outline(id);
                    let points = outline.as_ref().map(outline_to_cubics).unwrap_or_default();
                    let entry = if points.len() >= 4 {
                        Some((film.path(&points, true)?, metrics.advance_width(id)))
                    } else {
                        None
                    };
                    self.glyphs.insert((id, 0), entry);
                    entry
                }
            };
            match entry {
                Some((key, advance)) => {
                    let mut record = Record::new(key, Kind::Path);
                    record.transform = [scale, 0.0, 0.0, scale, x + pen * scale, y];
                    record.fill = color;
                    record.depth = depth;
                    records.push(record);
                    pen += advance;
                }
                None => pen += metrics.advance_width(id).max(units * 0.6),
            }
        }
        Ok((records, pen * scale))
    }
}

/// One closed path of `3n + 1` cubic control points per contour, joined
/// into a single path by repeating the first point so the fill stays
/// nonzero across contours (the renderer fills by winding, not by
/// subpath).
fn outline_to_cubics(outline: &swash::scale::outline::Outline) -> Vec<[f32; 2]> {
    let points = outline.points();
    let mut out: Vec<[f32; 2]> = Vec::new();
    let mut i = 0;
    let mut start = [0.0f32; 2];
    let mut last = [0.0f32; 2];
    let mut first_start: Option<[f32; 2]> = None;
    let mut line = |out: &mut Vec<[f32; 2]>, a: [f32; 2], b: [f32; 2]| {
        out.push([a[0] + (b[0] - a[0]) / 3.0, a[1] + (b[1] - a[1]) / 3.0]);
        out.push([
            a[0] + 2.0 * (b[0] - a[0]) / 3.0,
            a[1] + 2.0 * (b[1] - a[1]) / 3.0,
        ]);
        out.push(b);
    };
    for verb in outline.verbs() {
        match verb {
            Verb::MoveTo => {
                let p = [points[i].x, points[i].y];
                i += 1;
                if let Some(first) = first_start {
                    // Close the previous contour and bridge to the new one
                    // with a zero-area pair of lines.
                    if last != start {
                        line(&mut out, last, start);
                    }
                    line(&mut out, start, first);
                    line(&mut out, first, p);
                } else {
                    first_start = Some(p);
                    out.push(p);
                }
                start = p;
                last = p;
            }
            Verb::LineTo => {
                let p = [points[i].x, points[i].y];
                i += 1;
                line(&mut out, last, p);
                last = p;
            }
            Verb::QuadTo => {
                let c = [points[i].x, points[i].y];
                let p = [points[i + 1].x, points[i + 1].y];
                i += 2;
                out.push([
                    last[0] + 2.0 / 3.0 * (c[0] - last[0]),
                    last[1] + 2.0 / 3.0 * (c[1] - last[1]),
                ]);
                out.push([
                    p[0] + 2.0 / 3.0 * (c[0] - p[0]),
                    p[1] + 2.0 / 3.0 * (c[1] - p[1]),
                ]);
                out.push(p);
                last = p;
            }
            Verb::CurveTo => {
                out.push([points[i].x, points[i].y]);
                out.push([points[i + 1].x, points[i + 1].y]);
                let p = [points[i + 2].x, points[i + 2].y];
                out.push(p);
                i += 3;
                last = p;
            }
            Verb::Close => {
                if last != start {
                    line(&mut out, last, start);
                    last = start;
                }
            }
        }
    }
    if let Some(first) = first_start {
        if last != start {
            line(&mut out, last, start);
        }
        if start != first {
            line(&mut out, start, first);
        }
    }
    out
}

/// A rectangle `width` × `height` centered on the origin with corners of
/// radius `radius`, as one closed cubic path.
pub fn rounded_rect(film: &mut Film, width: f32, height: f32, radius: f32) -> Result<u64, Error> {
    let r = radius.min(width / 2.0).min(height / 2.0).max(0.0);
    let (hw, hh) = (width / 2.0, height / 2.0);
    // Circle-arc cubic constant.
    let k = 0.552_284_8 * r;
    let mut p = Vec::with_capacity(25);
    let line = |p: &mut Vec<[f32; 2]>, a: [f32; 2], b: [f32; 2]| {
        p.push([a[0] + (b[0] - a[0]) / 3.0, a[1] + (b[1] - a[1]) / 3.0]);
        p.push([
            a[0] + 2.0 * (b[0] - a[0]) / 3.0,
            a[1] + 2.0 * (b[1] - a[1]) / 3.0,
        ]);
        p.push(b);
    };
    p.push([-hw + r, -hh]);
    line(&mut p, [-hw + r, -hh], [hw - r, -hh]);
    p.extend([[hw - r + k, -hh], [hw, -hh + r - k], [hw, -hh + r]]);
    line(&mut p, [hw, -hh + r], [hw, hh - r]);
    p.extend([[hw, hh - r + k], [hw - r + k, hh], [hw - r, hh]]);
    line(&mut p, [hw - r, hh], [-hw + r, hh]);
    p.extend([[-hw + r - k, hh], [-hw, hh - r + k], [-hw, hh - r]]);
    line(&mut p, [-hw, hh - r], [-hw, -hh + r]);
    p.extend([[-hw, -hh + r - k], [-hw + r - k, -hh], [-hw + r, -hh]]);
    film.path(&p, true)
}

/// Moves every record by `(dx, dy)` and adds `depth` to its depth.
pub fn place(records: &mut [Record], dx: f32, dy: f32, depth: f32) {
    for record in records {
        record.transform[4] += dx;
        record.transform[5] += dy;
        record.depth += depth;
    }
}

/// A filled, bordered panel `width` × `height` centered on the origin.
pub fn panel(
    film: &mut Film,
    theme: &Theme,
    width: f32,
    height: f32,
    radius: f32,
    stroke_width: f32,
) -> Result<Record, Error> {
    let key = rounded_rect(film, width, height, radius)?;
    let mut record = Record::new(key, Kind::Path);
    record.fill = theme.surface;
    record.stroke = theme.border;
    record.stroke_width = stroke_width;
    Ok(record)
}

/// A primary button with `label` centered in it.
pub fn button(
    film: &mut Film,
    font: &mut Font,
    theme: &Theme,
    label: &str,
    width: f32,
    height: f32,
) -> Result<Vec<Record>, Error> {
    let key = rounded_rect(film, width, height, height / 2.0)?;
    let mut back = Record::new(key, Kind::Path);
    back.fill = theme.accent;
    let size = height * 0.42;
    let (mut text, advance) = font.text(film, label, size, theme.on_accent, 0.0, 0.0, -0.1)?;
    place(&mut text, -advance / 2.0, -size * 0.35, 0.0);
    let mut records = vec![back];
    records.extend(text);
    Ok(records)
}

/// A chat row: `who` in secondary text over `line` in body text, inside a
/// panel `width` wide. Returns the records and the row's height.
pub fn chat_row(
    film: &mut Film,
    font: &mut Font,
    theme: &Theme,
    who: &str,
    line: &str,
    width: f32,
    size: f32,
) -> Result<(Vec<Record>, f32), Error> {
    let height = size * 3.6;
    let mut records = vec![panel(film, theme, width, height, size * 0.6, size * 0.06)?];
    let left = -width / 2.0 + size * 0.9;
    let (name, _) = font.text(
        film,
        who,
        size * 0.8,
        theme.text_secondary,
        left,
        size * 0.75,
        -0.1,
    )?;
    let (body, _) = font.text(film, line, size, theme.text, left, -size * 0.9, -0.1)?;
    records.extend(name);
    records.extend(body);
    Ok((records, height))
}

/// A PNG or JPEG file as an image layer: the record fills the unit square,
/// so scale it by `(aspect * h / 2, h / 2)` to show it `h` tall.
pub fn image_file(film: &mut Film, path: &std::path::Path) -> Result<(Record, f32), String> {
    let decoded = image::open(path)
        .map_err(|error| format!("can't read {}: {error}", path.display()))?
        .into_rgba8();
    let (width, height) = decoded.dimensions();
    let key = film
        .image(width, height, decoded.as_raw())
        .map_err(|error| error.to_string())?;
    let mut record = Record::new(key, Kind::Image);
    record.fill = [1.0, 1.0, 1.0, 1.0];
    Ok((record, width as f32 / height.max(1) as f32))
}

/// An image record scaled to `height` view units, centered at `(x, y)`.
pub fn fit(record: &mut Record, aspect: f32, height: f32, x: f32, y: f32) {
    record.transform = [aspect * height / 2.0, 0.0, 0.0, height / 2.0, x, y];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::take::View;

    #[test]
    fn text_sets_glyphs_as_cubic_paths_and_advances() {
        let mut film = Film::new(640, 360, 30.0);
        let mut font = Font::new();
        let (records, advance) = font
            .text(&mut film, "Verse", 1.0, [1.0; 4], 0.0, 0.0, 0.0)
            .unwrap();
        assert_eq!(records.len(), 5);
        assert!(advance > 2.0 && advance < 4.0, "advance {advance}");
        let (again, _) = font
            .text(&mut film, "e", 1.0, [1.0; 4], 0.0, 0.0, 0.0)
            .unwrap();
        assert_eq!(again[0].key, records[1].key, "a glyph uploads once");
    }

    #[test]
    fn a_chat_row_in_both_themes_makes_a_readable_take() {
        let mut film = Film::new(1280, 720, 60.0);
        let mut font = Font::new();
        let mut records = Vec::new();
        for (i, theme) in [Theme::light(), Theme::noir()].iter().enumerate() {
            let (mut row, _) = chat_row(
                &mut film,
                &mut font,
                theme,
                "Christopher",
                "Ship it",
                7.0,
                0.4,
            )
            .unwrap();
            place(&mut row, 0.0, 2.0 - 4.0 * i as f32, 0.0);
            records.extend(row);
            records.extend(button(&mut film, &mut font, theme, "Run", 2.0, 0.6).unwrap());
        }
        film.frame(
            View {
                x: -8.0,
                y: -4.5,
                width: 16.0,
                height: 9.0,
            },
            1,
            &records,
        )
        .unwrap();
        let take = crate::read(&film.finish(false)).unwrap();
        assert_eq!(take.frames[0].records.len(), records.len());
        assert_ne!(Theme::light().text, Theme::noir().text);
    }
}

/// A 1280×720, 60 fps take: a Verse capture (`image`, if given) under a
/// Coder Noir chat row and a button whose label reveals over `pictures`
/// pictures, each held `hold` frames.
pub fn demo(image: Option<&std::path::Path>, pictures: u32, hold: u32) -> Result<Vec<u8>, String> {
    let mut film = Film::new(1280, 720, 60.0);
    let mut font = Font::new();
    let theme = Theme::noir();
    let view = crate::take::View {
        x: -8.0,
        y: -4.5,
        width: 16.0,
        height: 9.0,
    };
    let mut fixed = Vec::new();
    let back = rounded_rect(&mut film, 16.0, 9.0, 0.0).map_err(|e| e.to_string())?;
    let mut back = Record::new(back, Kind::Path);
    back.fill = theme.background;
    back.depth = 10.0;
    fixed.push(back);
    if let Some(path) = image {
        let (mut picture, aspect) = image_file(&mut film, path)?;
        fit(&mut picture, aspect, 6.0, -3.2, 0.3);
        picture.depth = 5.0;
        fixed.push(picture);
    }
    let (mut row, _) = chat_row(
        &mut film,
        &mut font,
        &theme,
        "Christopher",
        "Walk me to the Everglade",
        6.4,
        0.36,
    )
    .map_err(|e| e.to_string())?;
    place(&mut row, 4.2, 1.6, 0.0);
    fixed.extend(row);
    let (mut title, _) = font
        .text(&mut film, "OpenAgents", 0.9, theme.text, -7.4, 3.6, 0.0)
        .map_err(|e| e.to_string())?;
    fixed.extend(title.drain(..));
    let mut go = button(&mut film, &mut font, &theme, "Run in Coder", 3.6, 0.8)
        .map_err(|e| e.to_string())?;
    place(&mut go, 4.2, -0.2, 0.0);
    for i in 0..pictures.max(1) {
        let t = i as f32 / (pictures.max(2) - 1) as f32;
        let mut records = fixed.clone();
        for record in &go {
            let mut r = record.clone();
            if r.kind == Kind::Path && r.fill == theme.on_accent {
                r.reveal = [0.0, t];
            }
            records.push(r);
        }
        film.frame(view, hold, &records)
            .map_err(|e| e.to_string())?;
    }
    Ok(film.finish(false))
}
