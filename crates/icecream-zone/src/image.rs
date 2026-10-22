//! An image chunk: a palette and run-length rows, for die-cut cards drawn
//! in a few flat colours.
//!
//! ```text
//! "ICI1"  u16 width  u16 height  u8 colours
//! per colour: u8 r, g, b, a
//! runs until width * height pixels: u8 length (1 to 255), u8 colour index
//! ```
//!
//! Index 0 is usually transparent. A card in a handful of flat colours
//! packs to a few kilobytes, and a decoder can check that every pixel is a
//! palette colour.

const MAGIC: &[u8; 4] = b"ICI1";

/// A decoded image: RGBA, row by row from the top.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u16,
    pub height: u16,
    pub palette: Vec<[u8; 4]>,
    /// One palette index a pixel.
    pub pixels: Vec<u8>,
}

impl Image {
    /// The pixels as RGBA bytes.
    #[must_use]
    pub fn rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 4);
        for &i in &self.pixels {
            out.extend_from_slice(&self.palette.get(usize::from(i)).copied().unwrap_or([0; 4]));
        }
        out
    }
}

/// Packs an image.
///
/// # Panics
/// The pixel count isn't width times height, or the palette has more than
/// 255 colours.
#[must_use]
pub fn encode(image: &Image) -> Vec<u8> {
    assert_eq!(
        image.pixels.len(),
        usize::from(image.width) * usize::from(image.height)
    );
    assert!(image.palette.len() <= 255);
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&image.width.to_le_bytes());
    out.extend_from_slice(&image.height.to_le_bytes());
    out.push(image.palette.len() as u8);
    for c in &image.palette {
        out.extend_from_slice(c);
    }
    let mut i = 0;
    while i < image.pixels.len() {
        let colour = image.pixels[i];
        let mut run = 1;
        while run < 255 && i + run < image.pixels.len() && image.pixels[i + run] == colour {
            run += 1;
        }
        out.push(run as u8);
        out.push(colour);
        i += run;
    }
    out
}

/// Unpacks an image.
///
/// # Errors
/// The bytes aren't an image chunk, end early, or name a colour the
/// palette doesn't have.
pub fn decode(bytes: &[u8]) -> Result<Image, String> {
    if bytes.len() < 9 || &bytes[..4] != MAGIC {
        return Err("not an image chunk".into());
    }
    let width = u16::from_le_bytes([bytes[4], bytes[5]]);
    let height = u16::from_le_bytes([bytes[6], bytes[7]]);
    let colours = usize::from(bytes[8]);
    let start = 9 + colours * 4;
    let palette_bytes = bytes.get(9..start).ok_or("the image ends early")?;
    let palette: Vec<[u8; 4]> = palette_bytes
        .chunks_exact(4)
        .map(|c| [c[0], c[1], c[2], c[3]])
        .collect();
    let total = usize::from(width) * usize::from(height);
    let mut pixels = Vec::with_capacity(total);
    for run in bytes[start..].chunks_exact(2) {
        if usize::from(run[1]) >= palette.len() {
            return Err("the image names a colour it doesn't have".into());
        }
        pixels.extend(std::iter::repeat_n(run[1], usize::from(run[0])));
    }
    if pixels.len() != total {
        return Err("the image ends early".into());
    }
    Ok(Image {
        width,
        height,
        palette,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_image_round_trips_and_flat_colour_packs_small() {
        let mut pixels = vec![0u8; 300 * 400];
        for (i, p) in pixels.iter_mut().enumerate() {
            if (i / 300) > 100 && (i % 300) > 50 {
                *p = 1 + u8::from(i % 300 > 200);
            }
        }
        let image = Image {
            width: 300,
            height: 400,
            palette: vec![[0, 0, 0, 0], [27, 23, 20, 255], [241, 228, 200, 255]],
            pixels,
        };
        let bytes = encode(&image);
        assert!(bytes.len() < 10_000, "{}", bytes.len());
        let back = decode(&bytes).unwrap();
        assert_eq!(back, image);
        assert_eq!(back.rgba().len(), 300 * 400 * 4);
        assert!(decode(&bytes[..bytes.len() - 2]).is_err());
        assert!(decode(b"ICZ1....").is_err());
    }
}
