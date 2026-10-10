//! A computer's screenshot or image file (#11185), brought down to a small
//! grid of brightness levels the Verse HUD draws in its amber ladder, like
//! the invitation QR code, and the Android host as a bitmap. The image is
//! decoded here, in Rust, under fixed bounds; native code never decodes
//! it.
use std::io::Cursor;

use serde::{Deserialize, Serialize};

/// The most columns and rows a grid has.
pub(crate) const MAX_COLUMNS: u32 = 128;
pub(crate) const MAX_ROWS: u32 = 96;
/// The largest picture decoded, a side.
const MAX_SIDE: u32 = 8192;

/// One capture's picture: `rows` strings of `width` digits, `0` darkest
/// to `9` brightest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureGrid {
    /// The capture's surface resource (`image:computer-capture-N`).
    pub resource: String,
    pub label: String,
    pub width: usize,
    pub height: usize,
    pub rows: Vec<String>,
}

impl CaptureGrid {
    /// Whether a grid from the native feed keeps its bounds.
    pub(crate) fn valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.width <= MAX_COLUMNS as usize
            && self.height <= MAX_ROWS as usize
            && self.rows.len() == self.height
            && self.resource.len() <= 96
            && self.label.len() <= 256
            && self
                .rows
                .iter()
                .all(|row| row.len() == self.width && row.bytes().all(|b| b.is_ascii_digit()))
    }
}

/// The grid for a PNG or JPEG, keeping its proportions (a cell is about
/// twice as tall as wide on the HUD, so rows are halved); `None` when it
/// doesn't decode within the bounds.
pub(crate) fn grid(bytes: &[u8], resource: &str, label: &str) -> Option<CaptureGrid> {
    let format = image::guess_format(bytes).ok()?;
    if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        return None;
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader.decode().ok()?;
    let (w, h) = (decoded.width().max(1), decoded.height().max(1));
    let columns = MAX_COLUMNS.min(w);
    // Cells are square here; the HUD draws them square too.
    let rows =
        (u64::from(h) * u64::from(columns) / u64::from(w)).clamp(1, u64::from(MAX_ROWS)) as u32;
    let columns = if rows == MAX_ROWS {
        (u64::from(w) * u64::from(MAX_ROWS) / u64::from(h)).clamp(1, u64::from(MAX_COLUMNS)) as u32
    } else {
        columns
    };
    let small = decoded
        .resize_exact(columns, rows, image::imageops::FilterType::Triangle)
        .into_luma8();
    let rows: Vec<String> = small
        .rows()
        .map(|row| {
            row.map(|pixel| char::from(b'0' + (u16::from(pixel.0[0]) * 10 / 256) as u8))
                .collect()
        })
        .collect();
    Some(CaptureGrid {
        resource: resource.to_owned(),
        label: label.chars().take(200).collect(),
        width: columns as usize,
        height: rows.len(),
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_becomes_a_bounded_grid_and_other_bytes_none() {
        let mut png = Vec::new();
        let picture = image::RgbImage::from_fn(400, 200, |x, _| {
            if x < 200 {
                image::Rgb([0, 0, 0])
            } else {
                image::Rgb([255, 255, 255])
            }
        });
        image::DynamicImage::ImageRgb8(picture)
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let grid = grid(&png, "image:computer-capture-1", "A screenshot").unwrap();
        assert!(grid.valid());
        assert_eq!((grid.width, grid.height), (128, 64));
        assert!(grid.rows[0].starts_with('0'));
        assert!(grid.rows[0].ends_with('9'));
        assert!(super::grid(b"not a picture", "image:computer-capture-1", "x").is_none());
    }
}
