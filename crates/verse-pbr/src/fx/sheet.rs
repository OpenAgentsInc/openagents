//! The sprite sheets particles draw from: square PNGs under
//! `assets/verse/fx/`, rendered by the Blender scripts under
//! `scripts/blender/fx/` (`scripts/blender/build-fx.sh`).
//!
//! A sheet is a grid of equal cells read row by row from the top left.
//! Its RGB is premultiplied linear color, sRGB-encoded, and its alpha is
//! linear coverage (`scripts/blender/fx/common.py`). The renderer stacks the
//! sheets as the layers of one texture array, so every particle draws
//! through one pipeline and one bind group.

/// One sheet.
#[derive(Debug)]
pub struct Sheet {
    pub name: &'static str,
    pub columns: u32,
    pub rows: u32,
    /// Cells in use, row by row; the rest are empty.
    pub frames: u32,
    pub png: &'static [u8],
}

/// Every sheet's edge, pixels.
pub const SIZE: u32 = 512;

/// The sheets, in texture-array layer order.
pub static SHEETS: [Sheet; 3] = [
    Sheet {
        name: "fireball",
        columns: 4,
        rows: 4,
        frames: 16,
        png: include_bytes!("../../../../assets/verse/fx/fireball.png"),
    },
    Sheet {
        name: "smoke",
        columns: 4,
        rows: 4,
        frames: 16,
        png: include_bytes!("../../../../assets/verse/fx/smoke.png"),
    },
    Sheet {
        name: "sparks",
        columns: 2,
        rows: 2,
        frames: 4,
        png: include_bytes!("../../../../assets/verse/fx/sparks.png"),
    },
];

/// The sheet called `name`.
#[must_use]
pub fn find(name: &str) -> Option<&'static Sheet> {
    SHEETS.iter().find(|s| s.name == name)
}

/// The texture-array layer of the sheet called `name`.
#[must_use]
pub fn layer(name: &str) -> Option<u32> {
    SHEETS.iter().position(|s| s.name == name).map(|i| i as u32)
}

impl Sheet {
    /// Cell `frame`'s texture rectangle: left, top, right, and bottom, in
    /// 0..1, inset half a texel so filtering never reads a neighbor.
    #[must_use]
    pub fn rect(&self, frame: u32) -> [f32; 4] {
        let frame = frame.min(self.frames.saturating_sub(1));
        let (col, row) = (frame % self.columns, frame / self.columns);
        let (w, h) = (1.0 / self.columns as f32, 1.0 / self.rows as f32);
        let inset = 0.5 / SIZE as f32;
        [
            col as f32 * w + inset,
            row as f32 * h + inset,
            (col + 1) as f32 * w - inset,
            (row + 1) as f32 * h - inset,
        ]
    }

    /// The sheet's RGBA8 pixels.
    ///
    /// # Errors
    ///
    /// Returns why the PNG doesn't decode to a [`SIZE`]-square RGBA image.
    pub fn decode(&self) -> Result<Vec<u8>, String> {
        let decoder = png::Decoder::new(std::io::Cursor::new(self.png));
        let mut reader = decoder
            .read_info()
            .map_err(|e| format!("fx sheet {}: {e}", self.name))?;
        let mut pixels = vec![0; reader.output_buffer_size().unwrap_or(0)];
        let info = reader
            .next_frame(&mut pixels)
            .map_err(|e| format!("fx sheet {}: {e}", self.name))?;
        if info.width != SIZE
            || info.height != SIZE
            || info.color_type != png::ColorType::Rgba
            || info.bit_depth != png::BitDepth::Eight
        {
            return Err(format!(
                "fx sheet {} is {}x{} {:?} {:?}, not {SIZE}x{SIZE} RGBA8",
                self.name, info.width, info.height, info.color_type, info.bit_depth
            ));
        }
        pixels.truncate(info.buffer_size());
        Ok(pixels)
    }
}

/// Every sheet's mip chain, for a texture array: `levels[mip][layer]` is
/// that level's RGBA8 pixels. `skip` drops the largest levels, which the
/// low tier does to quarter the memory.
///
/// # Errors
///
/// Returns why a sheet doesn't decode.
pub fn mip_layers(skip: u32) -> Result<Vec<(u32, Vec<Vec<u8>>)>, String> {
    let mut levels: Vec<(u32, Vec<Vec<u8>>)> = Vec::new();
    for sheet in &SHEETS {
        let mut size = SIZE;
        let mut level = sheet.decode()?;
        let mut mip = 0;
        loop {
            if mip >= skip {
                let index = (mip - skip) as usize;
                if levels.len() <= index {
                    levels.push((size, Vec::new()));
                }
                levels[index].1.push(level.clone());
            }
            if size == 1 {
                break;
            }
            level = halve(&level, size);
            size /= 2;
            mip += 1;
        }
    }
    Ok(levels)
}

/// A 2-by-2 box filter in linear light. The color is premultiplied, so
/// averaging it is correct; alpha is linear already.
fn halve(pixels: &[u8], size: u32) -> Vec<u8> {
    let table = verse_engine::mips::srgb_to_linear();
    let half = size / 2;
    let mut out = vec![0u8; (half * half * 4) as usize];
    for y in 0..half {
        for x in 0..half {
            let mut sum = [0.0f64; 4];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let i = (((y * 2 + dy) * size + x * 2 + dx) * 4) as usize;
                for c in 0..3 {
                    sum[c] += table[pixels[i + c] as usize];
                }
                sum[3] += f64::from(pixels[i + 3]) / 255.0;
            }
            let o = ((y * half + x) * 4) as usize;
            for c in 0..3 {
                out[o + c] = encode(sum[c] / 4.0);
            }
            out[o + 3] = (sum[3] / 4.0 * 255.0).round() as u8;
        }
    }
    out
}

fn encode(linear: f64) -> u8 {
    let l = linear.clamp(0.0, 1.0);
    let s = if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}
