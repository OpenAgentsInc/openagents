//! One decoded frame, and the pixel conversions the outputs need.
//!
//! The camera is decoded once, to RGB, and every output reads that one
//! frame: the loopback turns it into YUYV, the recorder hands it to
//! `ffmpeg` as raw RGB, and the tracker resizes it for the model.

use std::time::{SystemTime, UNIX_EPOCH};

/// One frame off the camera.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// The frame's number since the daemon opened the camera.
    pub seq: u64,
    /// Seconds since the Unix epoch when the frame was read.
    pub timestamp: f64,
    pub width: u32,
    pub height: u32,
    /// Three bytes a pixel, rows top to bottom.
    pub rgb: Vec<u8>,
}

impl Frame {
    /// A frame of one colour, for a test.
    #[cfg(test)]
    pub fn solid(seq: u64, width: u32, height: u32, rgb: [u8; 3]) -> Frame {
        let mut pixels = Vec::with_capacity((width * height * 3) as usize);
        for _ in 0..width * height {
            pixels.extend_from_slice(&rgb);
        }
        Frame {
            seq,
            timestamp: now(),
            width,
            height,
            rgb: pixels,
        }
    }
}

/// Seconds since the Unix epoch, as the hands socket stamps a line.
pub fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// RGB to packed YUYV 4:2:2, full range, the format the loopback node
/// serves. Two pixels share one chroma pair, taken from the first.
pub fn rgb_to_yuyv(rgb: &[u8], width: u32, height: u32) -> Vec<u8> {
    let pixels = (width * height) as usize;
    let mut out = vec![0u8; pixels * 2];
    let mut o = 0;
    let mut i = 0;
    while i + 5 < rgb.len() && o + 3 < out.len() {
        let (y0, u, v) = yuv(rgb[i], rgb[i + 1], rgb[i + 2]);
        let (y1, _, _) = yuv(rgb[i + 3], rgb[i + 4], rgb[i + 5]);
        out[o] = y0;
        out[o + 1] = u;
        out[o + 2] = y1;
        out[o + 3] = v;
        i += 6;
        o += 4;
    }
    out
}

/// Packed YUYV to RGB, the inverse of [`rgb_to_yuyv`].
#[cfg(any(target_os = "linux", test))]
pub fn yuyv_to_rgb(buf: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = vec![0u8; (width * height * 3) as usize];
    let mut i = 0;
    let mut o = 0;
    while i + 3 < buf.len() && o + 5 < out.len() {
        let u = buf[i + 1] as i32 - 128;
        let v = buf[i + 3] as i32 - 128;
        out[o..o + 3].copy_from_slice(&rgb(buf[i] as i32, u, v));
        out[o + 3..o + 6].copy_from_slice(&rgb(buf[i + 2] as i32, u, v));
        i += 4;
        o += 6;
    }
    out
}

/// One pixel to full-range YUV, in fixed point.
fn yuv(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let (r, g, b) = (r as i32, g as i32, b as i32);
    let y = (77 * r + 150 * g + 29 * b + 128) >> 8;
    let u = ((-43 * r - 85 * g + 128 * b + 128) >> 8) + 128;
    let v = ((128 * r - 107 * g - 21 * b + 128) >> 8) + 128;
    (clamp(y), clamp(u), clamp(v))
}

/// One full-range YUV sample to RGB, in fixed point.
#[cfg(any(target_os = "linux", test))]
fn rgb(y: i32, u: i32, v: i32) -> [u8; 3] {
    let r = y + ((359 * v) >> 8);
    let g = y - ((88 * u + 183 * v) >> 8);
    let b = y + ((454 * u) >> 8);
    [clamp(r), clamp(g), clamp(b)]
}

fn clamp(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

/// One MJPEG frame decoded to RGB, with the size the header says.
#[cfg(target_os = "linux")]
pub fn mjpeg_to_rgb(buf: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(buf));
    let pixels = decoder.decode().map_err(|err| format!("mjpeg: {err}"))?;
    let info = decoder
        .info()
        .ok_or_else(|| "mjpeg has no header".to_string())?;
    let (width, height) = (info.width as u32, info.height as u32);
    match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => Ok((pixels, width, height)),
        jpeg_decoder::PixelFormat::L8 => {
            let mut rgb = Vec::with_capacity(pixels.len() * 3);
            for p in pixels {
                rgb.extend_from_slice(&[p, p, p]);
            }
            Ok((rgb, width, height))
        }
        other => Err(format!("mjpeg format {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_is_full_luma_with_centred_chroma() {
        let yuyv = rgb_to_yuyv(&[255, 255, 255, 255, 255, 255], 2, 1);
        assert_eq!(yuyv, vec![255, 128, 255, 128]);
        let black = rgb_to_yuyv(&[0, 0, 0, 0, 0, 0], 2, 1);
        assert_eq!(black, vec![0, 128, 0, 128]);
    }

    #[test]
    fn grey_survives_the_round_trip() {
        let rgb = vec![100u8; 4 * 2 * 3];
        let back = yuyv_to_rgb(&rgb_to_yuyv(&rgb, 4, 2), 4, 2);
        assert_eq!(back.len(), rgb.len());
        for (a, b) in back.iter().zip(&rgb) {
            assert!((*a as i32 - *b as i32).abs() <= 1, "{a} vs {b}");
        }
    }

    #[test]
    fn a_red_pixel_comes_back_red() {
        let yuyv = rgb_to_yuyv(&[255, 0, 0, 255, 0, 0], 2, 1);
        let back = yuyv_to_rgb(&yuyv, 2, 1);
        assert!(back[0] > 240 && back[1] < 15 && back[2] < 15, "{back:?}");
    }

    #[test]
    fn a_solid_frame_is_the_size_it_says() {
        let frame = Frame::solid(7, 4, 3, [1, 2, 3]);
        assert_eq!(frame.rgb.len(), 4 * 3 * 3);
        assert_eq!(&frame.rgb[..3], &[1, 2, 3]);
        assert_eq!(frame.seq, 7);
        assert!(frame.timestamp > 1.0e9);
    }
}
