//! Clef's Transformers 5.10.2 Torchvision CPU uint8 image resize.
//!
//! Reimplemented from the public Qwen2VL smart-resize and PyTorch 2.9.1
//! antialiased bicubic algorithms. Coefficient precision and intermediate
//! uint8 rounding matter: Pillow uses different coefficient precision.

use psionic_models::Qwen38RgbFrame;

const FACTOR: usize = 32;
const MIN_PIXELS: usize = 65_536;
const REFERENCE_MAX_PIXELS: usize = 16_777_216;
const SERVER_MAX_PIXELS: usize = 262_144;

/// Preserves the reference's resize target, then applies the server's bound.
pub(super) fn prepare(frame: Qwen38RgbFrame) -> Result<Qwen38RgbFrame, String> {
    let (width, height) = dimensions(frame.width, frame.height)?;
    resize(&frame, width, height)
}

fn dimensions(width: usize, height: usize) -> Result<(usize, usize), String> {
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err(String::from(
            "image dimensions must be between 1 and 4,096 pixels",
        ));
    }
    if width.max(height) as f64 / width.min(height) as f64 > 200.0 {
        return Err(String::from("image aspect ratio must not exceed 200"));
    }
    let mut resized_width = (width as f64 / FACTOR as f64).round_ties_even() as usize * FACTOR;
    let mut resized_height = (height as f64 / FACTOR as f64).round_ties_even() as usize * FACTOR;
    let pixels = width * height;
    if resized_width * resized_height > REFERENCE_MAX_PIXELS {
        let beta = (pixels as f64 / REFERENCE_MAX_PIXELS as f64).sqrt();
        resized_width = FACTOR.max((width as f64 / beta / FACTOR as f64).floor() as usize * FACTOR);
        resized_height =
            FACTOR.max((height as f64 / beta / FACTOR as f64).floor() as usize * FACTOR);
    } else if resized_width * resized_height < MIN_PIXELS {
        let beta = (MIN_PIXELS as f64 / pixels as f64).sqrt();
        resized_width = (width as f64 * beta / FACTOR as f64).ceil() as usize * FACTOR;
        resized_height = (height as f64 * beta / FACTOR as f64).ceil() as usize * FACTOR;
    }
    if resized_width * resized_height > SERVER_MAX_PIXELS {
        return Err(String::from(
            "image exceeds this server's 262,144-pixel processing limit; send a smaller image",
        ));
    }
    Ok((resized_width, resized_height))
}

struct Coefficients {
    starts: Vec<usize>,
    weights: Vec<Vec<i32>>,
    precision: u32,
}

fn cubic(value: f64) -> f64 {
    let x = value.abs();
    if x < 1.0 {
        (1.5 * x - 2.5) * x * x + 1.0
    } else if x < 2.0 {
        ((-0.5 * x + 2.5) * x - 4.0) * x + 2.0
    } else {
        0.0
    }
}

fn coefficients(input: usize, output: usize) -> Coefficients {
    let scale = input as f64 / output as f64;
    let support = 2.0 * scale.max(1.0);
    let inverse_scale = if scale >= 1.0 { 1.0 / scale } else { 1.0 };
    let maximum_size = support.ceil() as usize * 2 + 1;
    let mut starts = Vec::with_capacity(output);
    let mut values = Vec::with_capacity(output);
    let mut maximum_weight = 0.0f64;
    for index in 0..output {
        let center = scale * (index as f64 + 0.5);
        let first = ((center - support + 0.5) as isize).max(0) as usize;
        let end = ((center + support + 0.5) as usize).min(input);
        let count = end.saturating_sub(first).min(maximum_size);
        let mut row: Vec<f64> = (0..count)
            .map(|offset| cubic((offset as f64 + first as f64 - center + 0.5) * inverse_scale))
            .collect();
        let total: f64 = row.iter().sum();
        if total != 0.0 {
            for value in &mut row {
                *value /= total;
                maximum_weight = maximum_weight.max(*value);
            }
        }
        starts.push(first);
        values.push(row);
    }
    let mut precision = 0;
    while precision < 22 {
        let next = (0.5 + maximum_weight * f64::from(1u32 << (precision + 1))) as i32;
        if next >= 1 << 15 {
            break;
        }
        precision += 1;
    }
    let multiplier = f64::from(1u32 << precision);
    let weights = values
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|value| {
                    let scaled = value * multiplier;
                    if scaled < 0.0 {
                        (scaled - 0.5) as i32
                    } else {
                        (scaled + 0.5) as i32
                    }
                })
                .collect()
        })
        .collect();
    Coefficients {
        starts,
        weights,
        precision,
    }
}

fn resize(frame: &Qwen38RgbFrame, width: usize, height: usize) -> Result<Qwen38RgbFrame, String> {
    let horizontal = if width == frame.width {
        frame.rgb8.clone()
    } else {
        let weights = coefficients(frame.width, width);
        let mut pixels = vec![0u8; frame.height * width * 3];
        for row in 0..frame.height {
            for column in 0..width {
                for channel in 0..3 {
                    let mut value = 1i64 << (weights.precision - 1);
                    for (offset, coefficient) in weights.weights[column].iter().enumerate() {
                        let pixel = frame.rgb8
                            [(row * frame.width + weights.starts[column] + offset) * 3 + channel];
                        value += i64::from(pixel) * i64::from(*coefficient);
                    }
                    pixels[(row * width + column) * 3 + channel] =
                        (value >> weights.precision).clamp(0, 255) as u8;
                }
            }
        }
        pixels
    };
    let pixels = if height == frame.height {
        horizontal
    } else {
        let weights = coefficients(frame.height, height);
        let mut pixels = vec![0u8; height * width * 3];
        for row in 0..height {
            for column in 0..width {
                for channel in 0..3 {
                    let mut value = 1i64 << (weights.precision - 1);
                    for (offset, coefficient) in weights.weights[row].iter().enumerate() {
                        let pixel = horizontal
                            [((weights.starts[row] + offset) * width + column) * 3 + channel];
                        value += i64::from(pixel) * i64::from(*coefficient);
                    }
                    pixels[(row * width + column) * 3 + channel] =
                        (value >> weights.precision).clamp(0, 255) as u8;
                }
            }
        }
        pixels
    };
    Qwen38RgbFrame::new(width, height, pixels).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn clef_resize_preserves_reference_rounding_and_server_bounds() {
        assert_eq!(dimensions(32, 32).unwrap(), (256, 256));
        assert_eq!(dimensions(272, 272).unwrap(), (256, 256));
        assert_eq!(dimensions(304, 304).unwrap(), (320, 320));
        assert_eq!(dimensions(400, 160).unwrap(), (416, 192));
        assert!(dimensions(4096, 4096).is_err());
        assert!(dimensions(0, 1).is_err());
        assert!(dimensions(201, 1).is_err());
    }

    #[test]
    fn clef_images_match_torchvision_processor_fixtures() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/clef-images");
        let reference: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("reference.json")).unwrap()).unwrap();
        for case in reference["fixtures"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let bytes = std::fs::read(root.join(case["file"].as_str().unwrap())).unwrap();
            assert_eq!(
                hex::encode(Sha256::digest(&bytes)),
                case["source_sha256"],
                "{name}: source"
            );
            let mime = match case["format"].as_str().unwrap() {
                "PNG" => "image/png",
                "JPEG" => "image/jpeg",
                "WEBP" => "image/webp",
                _ => unreachable!(),
            };
            let decoded = super::super::decode_image(&bytes, mime).unwrap();
            assert_eq!(
                (decoded.width, decoded.height),
                (
                    case["width"].as_u64().unwrap() as usize,
                    case["height"].as_u64().unwrap() as usize
                )
            );
            let reference_decoded = if mime == "image/jpeg" {
                let bytes =
                    std::fs::read(root.join(case["decoded_file"].as_str().unwrap())).unwrap();
                let reference = super::super::decode_image(&bytes, "image/png").unwrap();
                assert_eq!(
                    hex::encode(Sha256::digest(&reference.rgb8)),
                    case["decoded_rgb_sha256"],
                    "{name}: decoded golden"
                );
                assert_eq!(
                    (decoded.width, decoded.height),
                    (reference.width, reference.height),
                    "{name}: decoded dimensions"
                );
                // These pinned decoders differ by up to three RGB levels
                // before resizing. Check that boundary separately from resize.
                let maximum_error = decoded
                    .rgb8
                    .iter()
                    .zip(&reference.rgb8)
                    .map(|(a, b)| a.abs_diff(*b))
                    .max()
                    .unwrap();
                assert!(
                    maximum_error <= 3,
                    "{name}: JPEG decoder error {maximum_error}"
                );
                Some(reference)
            } else {
                None
            };
            let resized = prepare(decoded).unwrap();
            assert_eq!(
                (resized.width, resized.height),
                (
                    case["resized_width"].as_u64().unwrap() as usize,
                    case["resized_height"].as_u64().unwrap() as usize
                ),
                "{name}: dimensions"
            );
            let expected_bytes = std::fs::read(root.join(format!("{name}.expected.png"))).unwrap();
            let expected = super::super::decode_image(&expected_bytes, "image/png").unwrap();
            assert_eq!(
                hex::encode(Sha256::digest(&expected.rgb8)),
                case["resized_rgb_sha256"],
                "{name}: golden"
            );
            if let Some(reference_decoded) = reference_decoded {
                let reference_resized = prepare(reference_decoded).unwrap();
                assert_eq!(
                    (reference_resized.width, reference_resized.height),
                    (expected.width, expected.height),
                    "{name}: bicubic dimensions"
                );
                assert_eq!(
                    reference_resized.rgb8, expected.rgb8,
                    "{name}: exact bicubic from the same decoded RGB"
                );
                // The complete JPEG path has the same measured three-level
                // bound; its decoder allowance never applies to bicubic alone.
                let maximum_error = resized
                    .rgb8
                    .iter()
                    .zip(&expected.rgb8)
                    .map(|(a, b)| a.abs_diff(*b))
                    .max()
                    .unwrap();
                assert!(maximum_error <= 3, "{name}: JPEG RGB error {maximum_error}");
            } else {
                assert_eq!(resized.rgb8, expected.rgb8, "{name}: resized RGB");
            }
            let prepared = psionic_models::qwen38_preprocess_image(
                name,
                mime,
                &expected,
                super::super::image_limits(),
            )
            .unwrap();
            assert_eq!(
                serde_json::to_value(prepared.receipt.grid_thw).unwrap(),
                case["grid_thw"],
                "{name}: grid"
            );
            assert_eq!(
                prepared.receipt.pixel_values_sha256, case["pixel_values_sha256"],
                "{name}: normalization and patch layout"
            );
            let image = super::super::ClefImage {
                bytes: bytes.into(),
                mime,
                digest: name.into(),
                grid: prepared.receipt.grid_thw,
            };
            let newline: Vec<u32> =
                serde_json::from_value(reference["newline_input_ids"].clone()).unwrap();
            let mut tokens = Vec::new();
            super::super::EncodedMedia::append(&mut tokens, &[image], &|_| newline.clone())
                .unwrap();
            assert_eq!(
                serde_json::to_value(tokens).unwrap(),
                case["media_input_ids"],
                "{name}: media token placement"
            );
        }
    }

    #[test]
    fn clef_image_positions_match_qwen35_reference_without_model_weights() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/clef-images");
        let reference: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.join("reference.json")).unwrap()).unwrap();
        let mut images = Vec::new();
        for name in reference["position_case"]["images"].as_array().unwrap() {
            let case = reference["fixtures"]
                .as_array()
                .unwrap()
                .iter()
                .find(|case| case["name"] == *name)
                .unwrap();
            let bytes = std::fs::read(root.join(case["file"].as_str().unwrap())).unwrap();
            images.push(super::super::ClefImage {
                bytes: bytes.into(),
                mime: "image/png",
                digest: name.as_str().unwrap().into(),
                grid: serde_json::from_value(case["grid_thw"].clone()).unwrap(),
            });
        }
        let newline: Vec<u32> =
            serde_json::from_value(reference["newline_input_ids"].clone()).unwrap();
        let mut tokens = vec![11, 12];
        let mut media =
            super::super::EncodedMedia::append(&mut tokens, &images, &|_| newline.clone()).unwrap();
        tokens.extend([21, 22]);
        media.finish_positions(tokens.len());
        assert_eq!(
            serde_json::to_value(tokens).unwrap(),
            reference["position_case"]["input_ids"]
        );
        assert_eq!(
            serde_json::to_value(media.positions).unwrap(),
            reference["position_case"]["positions"]
        );
    }
}
