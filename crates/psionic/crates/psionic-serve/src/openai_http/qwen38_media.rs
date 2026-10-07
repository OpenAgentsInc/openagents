use std::io::Cursor;
use std::path::Path;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use image::{
    AnimationDecoder, ImageDecoder, ImageFormat, ImageReader, Limits, codecs::gif::GifDecoder,
};
use psionic_models::{
    Qwen38DecoderMediaInput, Qwen38MultimodalDecoderPlan, Qwen38MultimodalDecoderPlanReceipt,
    Qwen38NativeVisionRuntime, Qwen38RenderedPrompt, Qwen38RgbFrame, Qwen38Tokenizer,
    Qwen38VisionAdmissionLimits, Qwen38VisionMediaKind, Qwen38VisionPreprocessedInput,
    Qwen38VisionPreprocessingReceipt, Qwen38VisionRuntimeBackend, Qwen38VisionRuntimeReceipt,
    build_qwen38_multimodal_decoder_plan, qwen38_preprocess_image, qwen38_preprocess_video,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub(super) const QWEN38_OPENAI_MAX_MEDIA_PER_REQUEST: usize = 4;
const QWEN38_OPENAI_MAX_TOTAL_ATTACHMENT_BYTES: usize = 16 * 1024 * 1024;
const QWEN38_OPENAI_MAX_GIF_SOURCE_FRAMES: usize = 32;
const QWEN38_OPENAI_MAX_IMAGE_DIMENSION: u32 = 4_096;
const QWEN38_OPENAI_MAX_CODEC_ALLOCATION_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Qwen38OpenAiMediaKind {
    Image,
    Video,
}

impl Qwen38OpenAiMediaKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Qwen38PreparedMediaInput {
    attachment: Qwen38OpenAiAttachmentReceipt,
    preprocessing: Qwen38VisionPreprocessedInput,
    source_fps: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct Qwen38OpenAiAttachmentReceipt {
    schema_version: &'static str,
    attachment_id: String,
    media_kind: &'static str,
    source_transport: &'static str,
    source_mime_type: String,
    source_bytes: usize,
    source_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(super) struct Qwen38OpenAiMultimodalReceipt {
    schema_version: &'static str,
    input_contract: &'static str,
    vision_backend: &'static str,
    vision_execution_engine: &'static str,
    attachment_count: usize,
    attachments: Vec<Qwen38OpenAiAttachmentReceipt>,
    preprocessing: Vec<Qwen38VisionPreprocessingReceipt>,
    vision_runtime: Vec<Qwen38VisionRuntimeReceipt>,
    decoder_plan: Qwen38MultimodalDecoderPlanReceipt,
    hidden_fallback_used: bool,
}

impl Qwen38OpenAiMultimodalReceipt {
    pub(super) const fn attachment_count(&self) -> usize {
        self.attachment_count
    }

    pub(super) const fn vision_backend(&self) -> &'static str {
        self.vision_backend
    }

    pub(super) fn attachment_sha256_csv(&self) -> String {
        self.attachments
            .iter()
            .map(|attachment| attachment.source_sha256.as_str())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub(super) const fn decoder_plan(&self) -> &Qwen38MultimodalDecoderPlanReceipt {
        &self.decoder_plan
    }
}

pub(super) struct Qwen38OpenAiMultimodalRuntime {
    vision: Qwen38NativeVisionRuntime,
    tokenizer: Qwen38Tokenizer,
}

impl Qwen38OpenAiMultimodalRuntime {
    pub(super) fn from_official_model_dir(model_dir: &Path) -> Result<Self, String> {
        let tokenizer = Qwen38Tokenizer::from_official_file(model_dir.join("tokenizer.json"))
            .map_err(|error| format!("failed to load Qwen3.8 official tokenizer: {error}"))?;
        let vision = Qwen38NativeVisionRuntime::from_official_model_dir(
            model_dir,
            Qwen38VisionRuntimeBackend::Cpu,
        )
        .map_err(|error| format!("failed to load Qwen3.8 native vision runtime: {error}"))?;
        Ok(Self { vision, tokenizer })
    }

    pub(super) fn build_plan(
        &self,
        prompt: &Qwen38RenderedPrompt,
        prepared: &[Qwen38PreparedMediaInput],
    ) -> Result<(Qwen38MultimodalDecoderPlan, Qwen38OpenAiMultimodalReceipt), String> {
        let mut media = Vec::with_capacity(prepared.len());
        let mut vision_receipts = Vec::with_capacity(prepared.len());
        for input in prepared {
            let output = self
                .vision
                .encode(&input.preprocessing)
                .map_err(|error| format!("Qwen3.8 native vision execution failed: {error}"))?;
            vision_receipts.push(output.receipt.clone());
            media.push(match input.preprocessing.receipt.media_kind {
                Qwen38VisionMediaKind::Image => {
                    Qwen38DecoderMediaInput::image(input.preprocessing.clone(), output)
                }
                Qwen38VisionMediaKind::Video => Qwen38DecoderMediaInput::video(
                    input.preprocessing.clone(),
                    output,
                    input.source_fps.ok_or_else(|| {
                        String::from("Qwen3.8 prepared video input is missing source fps")
                    })?,
                ),
            });
        }
        let plan = build_qwen38_multimodal_decoder_plan(prompt, &self.tokenizer, media.as_slice())
            .map_err(|error| format!("failed to build Qwen3.8 multimodal decoder plan: {error}"))?;
        let receipt = Qwen38OpenAiMultimodalReceipt {
            schema_version: "psionic.qwen38.openai_multimodal_receipt.v1",
            input_contract: "bounded_base64_data_urls",
            vision_backend: self.vision.backend().backend_label(),
            vision_execution_engine: self.vision.backend().execution_engine(),
            attachment_count: prepared.len(),
            attachments: prepared
                .iter()
                .map(|input| input.attachment.clone())
                .collect(),
            preprocessing: prepared
                .iter()
                .map(|input| input.preprocessing.receipt.clone())
                .collect(),
            vision_runtime: vision_receipts,
            decoder_plan: plan.receipt().clone(),
            hidden_fallback_used: false,
        };
        Ok((plan, receipt))
    }
}

pub(super) fn prepare_qwen38_openai_media(
    kind: Qwen38OpenAiMediaKind,
    url: &str,
    existing: &[Qwen38PreparedMediaInput],
) -> Result<Qwen38PreparedMediaInput, String> {
    if existing.len() >= QWEN38_OPENAI_MAX_MEDIA_PER_REQUEST {
        return Err(format!(
            "Qwen3.8 OpenAI media requests admit at most {QWEN38_OPENAI_MAX_MEDIA_PER_REQUEST} attachments"
        ));
    }
    let (mime_type, source_bytes) = decode_bounded_data_url(url)?;
    let total_bytes = existing
        .iter()
        .map(|input| input.attachment.source_bytes)
        .sum::<usize>()
        .checked_add(source_bytes.len())
        .ok_or_else(|| String::from("Qwen3.8 attachment byte accounting overflowed"))?;
    if total_bytes > QWEN38_OPENAI_MAX_TOTAL_ATTACHMENT_BYTES {
        return Err(format!(
            "Qwen3.8 OpenAI media requests admit at most {QWEN38_OPENAI_MAX_TOTAL_ATTACHMENT_BYTES} encoded attachment bytes"
        ));
    }
    let source_sha256 = hex::encode(Sha256::digest(source_bytes.as_slice()));
    let source_byte_count = source_bytes.len();
    let attachment_id = format!("{}-{source_sha256}", kind.label());
    let mut limits = Qwen38VisionAdmissionLimits::default();
    limits.timeout_ms = 120_000;
    let (preprocessing, source_fps) = match kind {
        Qwen38OpenAiMediaKind::Image => {
            let frame = decode_image(mime_type.as_str(), source_bytes.as_slice())?;
            let preprocessing = qwen38_preprocess_image(
                attachment_id.clone(),
                mime_type.clone(),
                &frame,
                limits,
            )
            .map_err(|error| format!("Qwen3.8 image preprocessing refused input: {error}"))?;
            (preprocessing, None)
        }
        Qwen38OpenAiMediaKind::Video => {
            let (frames, source_fps) = decode_animated_gif(mime_type.as_str(), source_bytes)?;
            let preprocessing = qwen38_preprocess_video(
                attachment_id.clone(),
                mime_type.clone(),
                frames.as_slice(),
                source_fps,
                limits,
            )
            .map_err(|error| format!("Qwen3.8 video preprocessing refused input: {error}"))?;
            (preprocessing, Some(source_fps))
        }
    };
    Ok(Qwen38PreparedMediaInput {
        attachment: Qwen38OpenAiAttachmentReceipt {
            schema_version: "psionic.qwen38.openai_attachment.v1",
            attachment_id,
            media_kind: kind.label(),
            source_transport: "data_url_base64",
            source_mime_type: mime_type,
            source_bytes: source_byte_count,
            source_sha256,
        },
        preprocessing,
        source_fps,
    })
}

fn decode_bounded_data_url(url: &str) -> Result<(String, Vec<u8>), String> {
    let data_url = url.strip_prefix("data:").ok_or_else(|| {
        String::from(
            "Qwen3.8 media input supports bounded base64 data URLs only; remote URLs are refused",
        )
    })?;
    let (metadata, payload) = data_url.split_once(',').ok_or_else(|| {
        String::from("Qwen3.8 media input must be a base64 data URL with a comma separator")
    })?;
    let mut metadata_parts = metadata.split(';');
    let mime_type = metadata_parts
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| String::from("Qwen3.8 media data URL is missing a MIME type"))?;
    if !metadata_parts.any(|part| part.eq_ignore_ascii_case("base64")) {
        return Err(String::from(
            "Qwen3.8 media data URL must use base64 encoding",
        ));
    }
    let maximum_encoded_bytes = Qwen38VisionAdmissionLimits::default()
        .maximum_attachment_bytes
        .saturating_mul(4)
        .div_ceil(3)
        .saturating_add(4);
    if payload.len() > maximum_encoded_bytes {
        return Err(format!(
            "Qwen3.8 media attachment exceeds the bounded base64 payload limit of {maximum_encoded_bytes} bytes"
        ));
    }
    let bytes = BASE64_STANDARD
        .decode(payload)
        .map_err(|error| format!("Qwen3.8 media data URL has invalid base64: {error}"))?;
    let maximum_attachment_bytes = Qwen38VisionAdmissionLimits::default().maximum_attachment_bytes;
    if bytes.len() > maximum_attachment_bytes {
        return Err(format!(
            "Qwen3.8 media attachment exceeds the bounded decoded limit of {maximum_attachment_bytes} bytes"
        ));
    }
    Ok((mime_type.to_ascii_lowercase(), bytes))
}

fn decode_image(mime_type: &str, bytes: &[u8]) -> Result<Qwen38RgbFrame, String> {
    let format = match mime_type {
        "image/png" => ImageFormat::Png,
        "image/jpeg" | "image/jpg" => ImageFormat::Jpeg,
        "image/webp" => ImageFormat::WebP,
        other => {
            return Err(format!(
                "Qwen3.8 image input MIME type `{other}` is unsupported; expected image/png, image/jpeg, or image/webp"
            ));
        }
    };
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(codec_limits());
    let image = reader
        .decode()
        .map_err(|error| format!("failed to decode Qwen3.8 image input: {error}"))?
        .into_rgb8();
    Qwen38RgbFrame::new(
        image.width() as usize,
        image.height() as usize,
        image.into_raw(),
    )
    .map_err(|error| format!("failed to construct Qwen3.8 RGB image frame: {error}"))
}

fn decode_animated_gif(
    mime_type: &str,
    bytes: Vec<u8>,
) -> Result<(Vec<Qwen38RgbFrame>, f64), String> {
    if mime_type != "image/gif" {
        return Err(format!(
            "Qwen3.8 video input MIME type `{mime_type}` is unsupported; the bounded lane accepts animated image/gif data URLs only"
        ));
    }
    let mut decoder = GifDecoder::new(Cursor::new(bytes))
        .map_err(|error| format!("failed to decode Qwen3.8 GIF video header: {error}"))?;
    decoder
        .set_limits(codec_limits())
        .map_err(|error| format!("Qwen3.8 GIF video exceeds codec limits: {error}"))?;
    let mut frames = Vec::new();
    let mut total_duration_ms = 0.0_f64;
    for frame in decoder.into_frames() {
        if frames.len() >= QWEN38_OPENAI_MAX_GIF_SOURCE_FRAMES {
            return Err(format!(
                "Qwen3.8 GIF video admits at most {QWEN38_OPENAI_MAX_GIF_SOURCE_FRAMES} source frames"
            ));
        }
        let frame =
            frame.map_err(|error| format!("failed to decode Qwen3.8 GIF frame: {error}"))?;
        let (delay_numerator_ms, delay_denominator) = frame.delay().numer_denom_ms();
        if delay_denominator == 0 {
            return Err(String::from(
                "Qwen3.8 GIF video contains an invalid frame delay denominator",
            ));
        }
        total_duration_ms += f64::from(delay_numerator_ms) / f64::from(delay_denominator);
        let buffer = frame.into_buffer();
        let width = buffer.width() as usize;
        let height = buffer.height() as usize;
        let rgba = buffer.into_raw();
        let mut rgb = Vec::with_capacity(width.saturating_mul(height).saturating_mul(3));
        for pixel in rgba.chunks_exact(4) {
            rgb.extend_from_slice(&pixel[..3]);
        }
        frames.push(
            Qwen38RgbFrame::new(width, height, rgb)
                .map_err(|error| format!("failed to construct Qwen3.8 GIF RGB frame: {error}"))?,
        );
    }
    if frames.is_empty() || !total_duration_ms.is_finite() || total_duration_ms <= 0.0 {
        return Err(String::from(
            "Qwen3.8 GIF video requires at least one frame and a finite positive duration",
        ));
    }
    let source_fps = frames.len() as f64 * 1_000.0 / total_duration_ms;
    Ok((frames, source_fps))
}

fn codec_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(QWEN38_OPENAI_MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(QWEN38_OPENAI_MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(QWEN38_OPENAI_MAX_CODEC_ALLOCATION_BYTES);
    limits
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use image::{Delay, Frame, ImageBuffer, Rgba, codecs::gif::GifEncoder};

    use super::{Qwen38OpenAiMediaKind, prepare_qwen38_openai_media};

    #[test]
    fn bounded_png_data_url_preserves_attachment_and_preprocessing_identity() {
        let image = ImageBuffer::from_pixel(256, 256, Rgba([17, 31, 47, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
            .expect("encode png");
        let url = format!(
            "data:image/png;base64,{}",
            BASE64_STANDARD.encode(bytes.as_slice())
        );

        let prepared = prepare_qwen38_openai_media(Qwen38OpenAiMediaKind::Image, &url, &[])
            .expect("prepare image");

        assert_eq!(prepared.attachment.media_kind, "image");
        assert_eq!(prepared.attachment.source_bytes, bytes.len());
        assert_eq!(prepared.preprocessing.receipt.width, 256);
        assert_eq!(prepared.preprocessing.receipt.height, 256);
        assert_eq!(
            prepared.preprocessing.receipt.attachment_id,
            prepared.attachment.attachment_id
        );
        assert_ne!(
            prepared.preprocessing.receipt.source_sha256, prepared.attachment.source_sha256,
            "encoded attachment and decoded RGB identities must stay distinct"
        );
    }

    #[test]
    fn bounded_animated_gif_data_url_prepares_video_frames_and_fps() {
        let mut bytes = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut bytes);
            for value in [20, 40, 60, 80] {
                let buffer = ImageBuffer::from_pixel(256, 256, Rgba([value, 0, 0, 255]));
                encoder
                    .encode_frame(Frame::from_parts(
                        buffer,
                        0,
                        0,
                        Delay::from_numer_denom_ms(100, 1),
                    ))
                    .expect("encode gif frame");
            }
        }
        let url = format!(
            "data:image/gif;base64,{}",
            BASE64_STANDARD.encode(bytes.as_slice())
        );

        let prepared = prepare_qwen38_openai_media(Qwen38OpenAiMediaKind::Video, &url, &[])
            .expect("prepare video");

        assert_eq!(prepared.attachment.media_kind, "video");
        assert_eq!(prepared.preprocessing.receipt.source_frame_count, 4);
        assert_eq!(
            prepared.preprocessing.receipt.sampled_frame_indices.len(),
            4
        );
        assert_eq!(prepared.source_fps, Some(10.0));
    }

    #[test]
    fn media_lane_refuses_remote_urls_and_unsupported_video_containers() {
        let remote = prepare_qwen38_openai_media(
            Qwen38OpenAiMediaKind::Image,
            "https://example.invalid/image.png",
            &[],
        )
        .expect_err("remote URL should refuse");
        assert!(remote.contains("data URLs only"));

        let mp4 = format!(
            "data:video/mp4;base64,{}",
            BASE64_STANDARD.encode(b"not-an-mp4")
        );
        let unsupported = prepare_qwen38_openai_media(Qwen38OpenAiMediaKind::Video, &mp4, &[])
            .expect_err("MP4 should refuse");
        assert!(unsupported.contains("animated image/gif"));
    }

    #[test]
    fn media_lane_refuses_malformed_oversized_and_excess_attachments() {
        let malformed = prepare_qwen38_openai_media(
            Qwen38OpenAiMediaKind::Image,
            "data:image/png;base64,not-valid-base64!",
            &[],
        )
        .expect_err("malformed base64 should refuse");
        assert!(malformed.contains("invalid base64"));

        let maximum_encoded_bytes = psionic_models::Qwen38VisionAdmissionLimits::default()
            .maximum_attachment_bytes
            .saturating_mul(4)
            .div_ceil(3)
            .saturating_add(4);
        let oversized_url = format!(
            "data:image/png;base64,{}",
            "A".repeat(maximum_encoded_bytes + 1)
        );
        let oversized =
            prepare_qwen38_openai_media(Qwen38OpenAiMediaKind::Image, oversized_url.as_str(), &[])
                .expect_err("oversized encoded attachment should refuse");
        assert!(oversized.contains("base64 payload limit"));

        let existing = vec![
            super::Qwen38PreparedMediaInput {
                attachment: super::Qwen38OpenAiAttachmentReceipt {
                    schema_version: "psionic.qwen38.openai_attachment.v1",
                    attachment_id: String::from("image-test"),
                    media_kind: "image",
                    source_transport: "data_url_base64",
                    source_mime_type: String::from("image/png"),
                    source_bytes: 1,
                    source_sha256: String::from("00"),
                },
                preprocessing: qwen38_preprocess_image_for_limit_test(),
                source_fps: None,
            };
            super::QWEN38_OPENAI_MAX_MEDIA_PER_REQUEST
        ];
        let excess = prepare_qwen38_openai_media(
            Qwen38OpenAiMediaKind::Image,
            "data:image/png;base64,AA==",
            existing.as_slice(),
        )
        .expect_err("excess attachment should refuse");
        assert!(excess.contains("at most 4 attachments"));
    }

    fn qwen38_preprocess_image_for_limit_test() -> psionic_models::Qwen38VisionPreprocessedInput {
        let frame = psionic_models::Qwen38RgbFrame::new(256, 256, vec![0; 256 * 256 * 3])
            .expect("construct image frame");
        psionic_models::qwen38_preprocess_image(
            "image-test",
            "image/png",
            &frame,
            psionic_models::Qwen38VisionAdmissionLimits::default(),
        )
        .expect("preprocess image fixture")
    }
}
