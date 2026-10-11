//! Bounded image decoding, Clef prompt placement, and native vision projection.

mod resize;

use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use psionic_models::{
    ClefNativeVisionRuntime, Qwen38RgbFrame, Qwen38VisionAdmissionLimits, TokenId,
    qwen38_preprocess_image,
};
use sha2::{Digest, Sha256};

use super::encode::RequestRefusal;
use super::json::ClefJson;

const MAX_IMAGES: usize = 4;
const MAX_BYTES: usize = 8 * 1024 * 1024;
const VISION_START: u32 = 248053;
const IMAGE_PAD: u32 = 248056;
const VISION_END: u32 = 248054;

/// An admitted compressed image. Pixel decoding is bounded before allocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClefImage {
    bytes: Arc<[u8]>,
    mime: &'static str,
    digest: String,
    grid: [usize; 3],
}

/// Images and their exact token ranges in one encoded Clef request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedMedia {
    images: Vec<ClefImage>,
    spans: Vec<(usize, usize)>,
    positions: Vec<[usize; 3]>,
}

/// Decoder inputs produced by the admitted image encoder.
#[derive(Clone, Debug)]
pub struct ClefMediaProjection {
    pub embeddings: Vec<(usize, Vec<f32>)>,
    pub positions: Vec<[usize; 3]>,
}

impl ClefMediaProjection {
    /// Checks every row and position before a decoder submits work.
    pub fn validate(&self, tokens: &[TokenId], hidden_size: usize) -> Result<(), String> {
        if hidden_size == 0
            || self.positions.len() != tokens.len()
            || self.embeddings.is_empty()
            || tokens
                .iter()
                .filter(|token| **token == TokenId(IMAGE_PAD))
                .count()
                != self.embeddings.len()
            || self
                .positions
                .iter()
                .flatten()
                .any(|position| *position > i32::MAX as usize)
        {
            return Err(String::from(
                "image decoder positions do not match the prompt",
            ));
        }
        let mut previous = None;
        for (index, row) in &self.embeddings {
            if previous.is_some_and(|old| old >= *index)
                || tokens.get(*index) != Some(&TokenId(IMAGE_PAD))
                || row.len() != hidden_size
                || row.iter().any(|value| !value.is_finite())
            {
                return Err(String::from("image decoder rows do not match the prompt"));
            }
            previous = Some(*index);
        }
        Ok(())
    }
}

/// Explicit CPU vision execution with a complete matching Clef tower.
pub struct ClefImageRuntime {
    model: ClefNativeVisionRuntime,
}

impl ClefImageRuntime {
    pub fn from_ollama_manifest(
        manifest: &Path,
        blobs: &Path,
        hidden_size: usize,
    ) -> Result<Self, String> {
        Ok(Self {
            model: ClefNativeVisionRuntime::from_ollama_manifest(manifest, blobs, hidden_size)?,
        })
    }

    pub fn digest(&self) -> &str {
        self.model.digest()
    }

    pub fn backend(&self) -> &'static str {
        "cpu"
    }
}

fn invalid(message: impl Into<String>) -> RequestRefusal {
    RequestRefusal::Invalid(message.into())
}
fn refused(message: impl Into<String>) -> RequestRefusal {
    RequestRefusal::NotAdmitted(message.into())
}

/// Parses PNG, JPEG, and WebP base64 inputs without fetching URLs.
pub(super) fn parse_images(value: Option<&ClefJson>) -> Result<Vec<ClefImage>, RequestRefusal> {
    let values = match value {
        None | Some(ClefJson::Null) => return Ok(Vec::new()),
        Some(ClefJson::Array(values)) => values,
        _ => {
            return Err(invalid(
                "images must be an array of base64 PNG, JPEG, or WebP strings",
            ));
        }
    };
    if values.len() > MAX_IMAGES {
        return Err(refused(format!(
            "this Clef server accepts at most {MAX_IMAGES} images per request"
        )));
    }
    let mut encoded_bytes = 0usize;
    values
        .iter()
        .map(|value| {
            let ClefJson::String(encoded) = value else {
                return Err(invalid("each image must be a base64 string"));
            };
            encoded_bytes = encoded_bytes
                .checked_add(encoded.len())
                .ok_or_else(|| refused("images exceed the request size limit"))?;
            if encoded_bytes > MAX_BYTES {
                return Err(refused("images exceed the 8 MiB request size limit"));
            }
            let (declared_mime, payload) = if let Some(data) = encoded.strip_prefix("data:") {
                let (metadata, payload) = data
                    .split_once(',')
                    .ok_or_else(|| invalid("image data URL has no comma"))?;
                let mime = metadata
                    .strip_suffix(";base64")
                    .ok_or_else(|| invalid("image data URL must use base64 encoding"))?;
                (Some(mime), payload)
            } else {
                (None, encoded.as_str())
            };
            let bytes = STANDARD
                .decode(payload)
                .map_err(|_| invalid("image is not valid base64"))?;
            let format = image::guess_format(&bytes)
                .map_err(|_| invalid("image is not a readable PNG, JPEG, or WebP file"))?;
            let mime = match format {
                ImageFormat::Png => "image/png",
                ImageFormat::Jpeg => "image/jpeg",
                ImageFormat::WebP => "image/webp",
                _ => {
                    return Err(refused(
                        "this Clef server accepts PNG, JPEG, and WebP images",
                    ));
                }
            };
            if declared_mime.is_some_and(|declared| {
                declared != mime && !(declared == "image/jpg" && mime == "image/jpeg")
            }) {
                return Err(invalid("image data URL type does not match its contents"));
            }
            let frame =
                resize::prepare(decode_image(&bytes, mime).map_err(invalid)?).map_err(refused)?;
            let limits = image_limits();
            let digest = hex::encode(Sha256::digest(&bytes));
            // Use the existing preprocessing validator, including aspect ratio and
            // patch count bounds, before the request reaches model execution.
            let prepared = qwen38_preprocess_image(digest.clone(), mime, &frame, limits)
                .map_err(|error| refused(error.to_string()))?;
            Ok(ClefImage {
                bytes: bytes.into(),
                mime,
                digest,
                grid: prepared.receipt.grid_thw,
            })
        })
        .collect()
}

fn image_limits() -> Qwen38VisionAdmissionLimits {
    Qwen38VisionAdmissionLimits {
        timeout_ms: 120_000,
        ..Qwen38VisionAdmissionLimits::default()
    }
}

fn decode_image(bytes: &[u8], mime: &str) -> Result<Qwen38RgbFrame, String> {
    let format = match mime {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/webp" => ImageFormat::WebP,
        _ => return Err(String::from("unsupported image type")),
    };
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let decoder = reader
        .into_decoder()
        .map_err(|_| String::from("image cannot be decoded within this server's limits"))?;
    let color = decoder.color_type();
    if color.bits_per_pixel() / u16::from(color.channel_count()) != 8 {
        return Err(String::from(
            "images must use 8-bit color; convert the image before sending it",
        ));
    }
    // The released Torchvision processor decodes base64 strings without
    // applying EXIF orientation. Preserve that byte-input pixel order.
    let decoded = DynamicImage::from_decoder(decoder)
        .map_err(|_| String::from("image cannot be decoded within this server's limits"))?;
    let pixels = decoded.into_rgb8();
    Qwen38RgbFrame::new(
        pixels.width() as usize,
        pixels.height() as usize,
        pixels.into_raw(),
    )
    .map_err(|error| error.to_string())
}

impl EncodedMedia {
    /// Appends the reference's media segment after P, including its final newline.
    pub(super) fn append(
        prefix: &mut Vec<u32>,
        images: &[ClefImage],
        tokenize: &dyn Fn(&str) -> Vec<u32>,
    ) -> Option<Self> {
        if images.is_empty() {
            return None;
        }
        let mut spans = Vec::with_capacity(images.len());
        for image in images {
            prefix.push(VISION_START);
            let first = prefix.len();
            let count = image.grid.iter().product::<usize>() / 4;
            prefix.extend(std::iter::repeat_n(IMAGE_PAD, count));
            spans.push((first, prefix.len()));
            prefix.push(VISION_END);
        }
        prefix.extend(tokenize("\n"));
        Some(Self {
            images: images.to_vec(),
            spans,
            positions: Vec::new(),
        })
    }

    pub(super) fn finish_positions(&mut self, token_count: usize) {
        let mut token = 0usize;
        let mut next = 0usize;
        for (image, &(first, end)) in self.images.iter().zip(&self.spans) {
            while token < first {
                self.positions.push([next; 3]);
                token += 1;
                next += 1;
            }
            let height = image.grid[1] / 2;
            let width = image.grid[2] / 2;
            for row in 0..height {
                for column in 0..width {
                    self.positions.push([next, next + row, next + column]);
                }
            }
            token = end;
            next += height.max(width);
        }
        while token < token_count {
            self.positions.push([next; 3]);
            token += 1;
            next += 1;
        }
    }

    /// Runs the actual image encoder after request admission and queueing.
    pub fn project(&self, runtime: &ClefImageRuntime) -> Result<ClefMediaProjection, String> {
        let mut embeddings = Vec::new();
        for (image, &(first, end)) in self.images.iter().zip(&self.spans) {
            let frame = resize::prepare(decode_image(&image.bytes, image.mime)?)?;
            let prepared =
                qwen38_preprocess_image(image.digest.clone(), image.mime, &frame, image_limits())
                    .map_err(|error| error.to_string())?;
            let rows = runtime.model.encode(&prepared)?;
            if rows.len() != end - first {
                return Err(String::from(
                    "image token count does not match its projected rows",
                ));
            }
            embeddings.extend(
                rows.into_iter()
                    .enumerate()
                    .map(|(offset, row)| (first + offset, row)),
            );
        }
        Ok(ClefMediaProjection {
            embeddings,
            positions: self.positions.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoded_image(format: ImageFormat, width: u32, height: u32) -> String {
        let image = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 251) as u8, (y % 241) as u8, 79])
        });
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut bytes, format)
            .unwrap();
        STANDARD.encode(bytes.into_inner())
    }

    #[test]
    fn clef_images_decode_png_jpeg_webp_and_reject_wrong_content_types() {
        for format in [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::WebP] {
            let encoded = encoded_image(format, 256, 256);
            let images = parse_images(Some(&ClefJson::Array(vec![ClefJson::String(
                encoded.clone(),
            )])))
            .unwrap();
            assert_eq!(images[0].grid, [1, 16, 16]);
            let bad = ClefJson::Array(vec![ClefJson::String(format!(
                "data:image/gif;base64,{encoded}"
            ))]);
            assert!(matches!(
                parse_images(Some(&bad)),
                Err(RequestRefusal::Invalid(_))
            ));
        }
    }

    #[test]
    fn clef_images_resize_small_inputs_and_refuse_invalid_base64() {
        let small = ClefJson::Array(vec![ClefJson::String(encoded_image(
            ImageFormat::Png,
            32,
            32,
        ))]);
        assert_eq!(parse_images(Some(&small)).unwrap()[0].grid, [1, 16, 16]);
        let large = ClefJson::Array(vec![ClefJson::String(encoded_image(
            ImageFormat::Png,
            512,
            1024,
        ))]);
        assert!(matches!(
            parse_images(Some(&large)),
            Err(RequestRefusal::NotAdmitted(_))
        ));
        for encoded in ["not base64", "https://example.com/image.png", "aGk="] {
            assert!(matches!(
                parse_images(Some(&ClefJson::Array(vec![ClefJson::String(
                    encoded.into()
                )]))),
                Err(RequestRefusal::Invalid(_))
            ));
        }
        assert!(parse_images(Some(&ClefJson::String(String::new()))).is_err());
    }

    #[test]
    fn clef_images_refuse_unvalidated_high_bit_depth() {
        let image = image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::from_pixel(
            256,
            256,
            image::Luma([1024]),
        );
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageLuma16(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        assert!(
            decode_image(&bytes.into_inner(), "image/png")
                .unwrap_err()
                .contains("8-bit")
        );
    }

    #[test]
    fn clef_image_projection_rejects_missing_duplicate_or_invalid_rows() {
        let tokens = [TokenId(11), TokenId(IMAGE_PAD), TokenId(12)];
        let valid = ClefMediaProjection {
            embeddings: vec![(1, vec![0.25, -0.5])],
            positions: vec![[0; 3], [1; 3], [2; 3]],
        };
        assert!(valid.validate(&tokens, 2).is_ok());
        let mut bad = valid.clone();
        bad.embeddings[0].1[0] = f32::NAN;
        assert!(bad.validate(&tokens, 2).is_err());
        let mut bad = valid.clone();
        bad.embeddings.push(bad.embeddings[0].clone());
        assert!(bad.validate(&tokens, 2).is_err());
        assert!(
            valid
                .validate(&[TokenId(IMAGE_PAD), TokenId(IMAGE_PAD), TokenId(12)], 2)
                .is_err()
        );
        assert!(valid.validate(&tokens, 3).is_err());
        assert!(valid.validate(&tokens[..2], 2).is_err());
    }

    #[test]
    fn clef_images_have_distinct_ranges_and_three_axis_positions() {
        let images = parse_images(Some(&ClefJson::Array(vec![
            ClefJson::String(encoded_image(ImageFormat::Png, 256, 256)),
            ClefJson::String(encoded_image(ImageFormat::Png, 256, 256)),
        ])))
        .unwrap();
        let mut tokens = vec![11, 12];
        let mut media = EncodedMedia::append(&mut tokens, &images, &|_| vec![10]).unwrap();
        assert_eq!(media.spans, [(3, 67), (69, 133)]);
        assert_eq!(tokens[2], VISION_START);
        assert_eq!(tokens[67], VISION_END);
        assert_eq!(tokens.last(), Some(&10));
        tokens.extend([21, 22]);
        media.finish_positions(tokens.len());
        assert_eq!(media.positions.len(), tokens.len());
        assert_eq!(media.positions[3], [3, 3, 3]);
        assert_eq!(media.positions[10], [3, 3, 10]);
        assert_eq!(media.positions[11], [3, 4, 3]);
        assert_eq!(media.positions[66], [3, 10, 10]);
        assert_eq!(media.positions[67], [11; 3]);
        assert_eq!(media.positions[69], [13; 3]);
        assert_eq!(media.positions[133], [21; 3]);
    }
}
