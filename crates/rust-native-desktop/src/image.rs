//! Images: bounded decoding, fitting a box, and filtered painting.
//!
//! An application shows an image as a Rust Native `surface` whose label is
//! the image's alternative text, so the semantic view says what the image
//! is; this module paints the pixels into the surface's rectangle. A large
//! image is shrunk with an area filter and an enlarged one is sampled
//! bilinearly, and [`fit`] keeps an image's aspect ratio and never enlarges
//! it past a bound the caller picks, so it stays sharp.

use crate::{Frame, PxRect};
use rust_native::style::Color;
use std::sync::Arc;

/// The widest and tallest image decoded, in pixels.
pub const MAX_DIMENSION: u32 = 4096;
/// The most bytes a decoder may allocate for one image.
const MAX_ALLOCATION: usize = 64 * 1024 * 1024;

/// Decoded pixels: eight-bit RGBA, straight (not premultiplied) alpha,
/// rows top to bottom.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

impl Image {
    /// An image from RGBA pixels, checked against its size.
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Result<Image, String> {
        if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err(format!(
                "images are 1 to {MAX_DIMENSION} pixels a side, not {width} × {height}"
            ));
        }
        if rgba.len() != width as usize * height as usize * 4 {
            return Err("the pixels don't match the image's size".into());
        }
        Ok(Image {
            width,
            height,
            rgba: rgba.into(),
        })
    }

    /// Decodes a PNG of any bit depth and color type, bounded in size.
    pub fn png(bytes: &[u8]) -> Result<Image, String> {
        let mut decoder = png::Decoder::new_with_limits(
            std::io::Cursor::new(bytes),
            png::Limits {
                bytes: MAX_ALLOCATION,
            },
        );
        decoder.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = decoder
            .read_info()
            .map_err(|error| format!("not a PNG: {error}"))?;
        let (width, height) = {
            let info = reader.info();
            (info.width, info.height)
        };
        if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err(format!(
                "images are 1 to {MAX_DIMENSION} pixels a side, not {width} × {height}"
            ));
        }
        let size = reader
            .output_buffer_size()
            .ok_or("the PNG is too large to decode")?;
        let mut buffer = vec![0; size];
        let output = reader
            .next_frame(&mut buffer)
            .map_err(|error| format!("a damaged PNG: {error}"))?;
        let pixels = &buffer[..output.buffer_size()];
        let rgba: Vec<u8> = match output.color_type {
            png::ColorType::Rgba => pixels.to_vec(),
            png::ColorType::Rgb => pixels
                .chunks_exact(3)
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect(),
            png::ColorType::GrayscaleAlpha => pixels
                .chunks_exact(2)
                .flat_map(|p| [p[0], p[0], p[0], p[1]])
                .collect(),
            png::ColorType::Grayscale => pixels.iter().flat_map(|&v| [v, v, v, 255]).collect(),
            png::ColorType::Indexed => return Err("an indexed PNG was not expanded".into()),
        };
        Image::from_rgba(width, height, rgba)
    }
}

/// Where an image `width` by `height` pixels goes in `area`: as large as
/// fits with its aspect ratio kept, but no more than `max_scale` times its
/// own size, centered, on whole pixels.
pub fn fit(width: u32, height: u32, area: PxRect, max_scale: f32) -> PxRect {
    let (w, h) = (width.max(1) as f32, height.max(1) as f32);
    let scale = (area.w / w).min(area.h / h).min(max_scale).max(0.0);
    let (fw, fh) = ((w * scale).round(), (h * scale).round());
    PxRect {
        x: (area.x + (area.w - fw) / 2.0).round(),
        y: (area.y + (area.h - fh) / 2.0).round(),
        w: fw,
        h: fh,
    }
}

/// For each destination pixel along one axis, the source pixels it reads
/// and their weights: every covered pixel, evenly, when the image shrinks
/// (an area filter), and the nearest two when it grows (bilinear).
fn taps(source: usize, destination: usize) -> Vec<Vec<(usize, f32)>> {
    let step = source as f32 / destination.max(1) as f32;
    (0..destination)
        .map(|d| {
            if step > 1.0 {
                let start = ((d as f32 * step).floor() as usize).min(source - 1);
                let end = (((d + 1) as f32 * step).ceil() as usize).clamp(start + 1, source);
                let weight = 1.0 / (end - start) as f32;
                (start..end).map(|i| (i, weight)).collect()
            } else {
                let at = ((d as f32 + 0.5) * step - 0.5).max(0.0);
                let low = (at.floor() as usize).min(source - 1);
                let high = (low + 1).min(source - 1);
                let t = at - low as f32;
                if low == high || t <= 0.0 {
                    vec![(low, 1.0)]
                } else {
                    vec![(low, 1.0 - t), (high, t)]
                }
            }
        })
        .collect()
}

/// Paints `image` stretched to `destination`, in pixels. Callers keep the
/// aspect ratio with [`fit`]. The frame's clip applies.
pub fn paint(frame: &mut Frame, image: &Image, destination: PxRect) {
    let (w, h) = (
        destination.w.round().max(0.0) as usize,
        destination.h.round().max(0.0) as usize,
    );
    if w == 0 || h == 0 {
        return;
    }
    let (x0, y0) = (destination.x.round() as i64, destination.y.round() as i64);
    let columns = taps(image.width as usize, w);
    let rows = taps(image.height as usize, h);
    let stride = image.width as usize * 4;
    for (dy, row) in rows.iter().enumerate() {
        let y = y0 + dy as i64;
        if y < 0 || y as usize >= frame.height {
            continue;
        }
        for (dx, column) in columns.iter().enumerate() {
            // Premultiplied sums, so transparent pixels don't darken edges.
            let mut sum = [0.0f32; 4];
            for &(sy, wy) in row {
                for &(sx, wx) in column {
                    let p = &image.rgba[sy * stride + sx * 4..sy * stride + sx * 4 + 4];
                    let weight = wx * wy;
                    let alpha = f32::from(p[3]) / 255.0 * weight;
                    sum[0] += f32::from(p[0]) * alpha;
                    sum[1] += f32::from(p[1]) * alpha;
                    sum[2] += f32::from(p[2]) * alpha;
                    sum[3] += alpha;
                }
            }
            if sum[3] <= 0.0 {
                continue;
            }
            let color = Color::rgb(
                (sum[0] / sum[3]).round().clamp(0.0, 255.0) as u8,
                (sum[1] / sum[3]).round().clamp(0.0, 255.0) as u8,
                (sum[2] / sum[3]).round().clamp(0.0, 255.0) as u8,
            );
            frame.blend(x0 + dx as i64, y, color, sum[3].min(1.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(size: u32) -> Image {
        let mut rgba = vec![];
        for y in 0..size {
            for x in 0..size {
                let on = (x + y) % 2 == 0;
                let v = if on { 255 } else { 0 };
                rgba.extend([v, v, v, 255]);
            }
        }
        Image::from_rgba(size, size, rgba).unwrap()
    }

    #[test]
    fn a_fit_keeps_the_aspect_ratio_centers_and_caps_the_scale() {
        let area = PxRect {
            x: 0.0,
            y: 0.0,
            w: 1000.0,
            h: 500.0,
        };
        let wide = fit(400, 100, area, 10.0);
        assert_eq!((wide.w, wide.h), (1000.0, 250.0));
        assert_eq!(wide.y, 125.0);
        let capped = fit(400, 100, area, 1.0);
        assert_eq!((capped.x, capped.w, capped.h), (300.0, 400.0, 100.0));
    }

    #[test]
    fn a_shrunk_image_averages_and_a_same_size_image_is_exact() {
        let image = checker(8);
        let mut frame = Frame::new(8, 8, Color::rgb(0, 0, 0));
        paint(
            &mut frame,
            &image,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: 8.0,
                h: 8.0,
            },
        );
        assert_eq!(&frame.pixels[..8], &[255, 255, 255, 255, 0, 0, 0, 255]);
        let mut small = Frame::new(2, 2, Color::rgb(0, 0, 0));
        paint(
            &mut small,
            &image,
            PxRect {
                x: 0.0,
                y: 0.0,
                w: 2.0,
                h: 2.0,
            },
        );
        assert!(
            (120..=135).contains(&small.pixels[0]),
            "{}",
            small.pixels[0]
        );
    }

    #[test]
    fn a_png_round_trips_and_garbage_is_refused() {
        let frame = Frame::new(3, 2, Color::rgb(10, 20, 30));
        let image = Image::png(&frame.png().unwrap()).unwrap();
        assert_eq!((image.width, image.height), (3, 2));
        assert_eq!(&image.rgba[..4], &[10, 20, 30, 255]);
        assert!(Image::png(b"not a png").is_err());
    }
}
