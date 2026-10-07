use std::{env, error::Error, path::PathBuf, time::Instant};

use psionic_models::{
    Qwen38DecoderMediaInput, Qwen38NativeVisionRuntime, Qwen38PromptContentPart,
    Qwen38PromptMessage, Qwen38PromptOptions, Qwen38PromptRole, Qwen38RgbFrame, Qwen38Tokenizer,
    Qwen38VisionAdmissionLimits, Qwen38VisionRuntimeBackend, TokenId, TokenSequence,
    build_qwen38_multimodal_decoder_plan, qwen38_preprocess_image, qwen38_preprocess_video,
    render_qwen38_prompt,
};
use psionic_serve::{CudaGgufQwen35TextGenerationService, GenerationOptions, GenerationRequest};
use serde_json::json;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let official_model_dir = PathBuf::from(args.next().ok_or(
        "usage: qwen38_multimodal_cuda_smoke <official-model-dir> <decoder.gguf> <image|video>",
    )?);
    let gguf_path = PathBuf::from(args.next().ok_or("missing decoder GGUF path")?);
    let media_kind = args.next().ok_or("missing image or video media kind")?;
    if args.next().is_some() {
        return Err("unexpected trailing arguments".into());
    }

    let started = Instant::now();
    let mut limits = Qwen38VisionAdmissionLimits::default();
    limits.timeout_ms = 120_000;
    let (preprocessing, prompt_part, source_fps) = match media_kind.as_str() {
        "image" => (
            qwen38_preprocess_image(
                "qwen38-multimodal-cuda-smoke-image",
                "image/raw-rgb8",
                &deterministic_frame(0)?,
                limits,
            )?,
            Qwen38PromptContentPart::Image {
                image: json!({"attachment_id": "qwen38-multimodal-cuda-smoke-image"}),
            },
            None,
        ),
        "video" => {
            let frames = (0..8)
                .map(deterministic_frame)
                .collect::<Result<Vec<_>, _>>()?;
            (
                qwen38_preprocess_video(
                    "qwen38-multimodal-cuda-smoke-video",
                    "video/raw-rgb8-frames",
                    frames.as_slice(),
                    4.0,
                    limits,
                )?,
                Qwen38PromptContentPart::Video {
                    video: json!({"attachment_id": "qwen38-multimodal-cuda-smoke-video"}),
                },
                Some(4.0),
            )
        }
        other => return Err(format!("unsupported media kind `{other}`").into()),
    };

    let vision_started = Instant::now();
    let vision_runtime = Qwen38NativeVisionRuntime::from_official_model_dir(
        official_model_dir.as_path(),
        Qwen38VisionRuntimeBackend::Cuda { device_ordinal: 0 },
    )?;
    let vision_output = vision_runtime.encode(&preprocessing)?;
    let vision_runtime_receipt = vision_output.receipt.clone();
    let vision_duration_ns = duration_ns(vision_started.elapsed());
    drop(vision_runtime);

    let prompt = render_qwen38_prompt(
        &[Qwen38PromptMessage::parts(
            Qwen38PromptRole::User,
            vec![
                prompt_part,
                Qwen38PromptContentPart::Text {
                    text: String::from("Describe the visual input in one short sentence."),
                },
            ],
        )],
        &Qwen38PromptOptions {
            enable_thinking: false,
            ..Qwen38PromptOptions::default()
        },
    )?;
    let tokenizer = Qwen38Tokenizer::from_official_file(official_model_dir.join("tokenizer.json"))?;
    let media = match source_fps {
        Some(fps) => Qwen38DecoderMediaInput::video(preprocessing, vision_output, fps),
        None => Qwen38DecoderMediaInput::image(preprocessing, vision_output),
    };
    let plan = build_qwen38_multimodal_decoder_plan(&prompt, &tokenizer, &[media])?;

    let decoder_load_started = Instant::now();
    let mut service = CudaGgufQwen35TextGenerationService::from_gguf_path(&gguf_path)?;
    let decoder_load_duration_ns = duration_ns(decoder_load_started.elapsed());
    let request = GenerationRequest::new_tokens(
        format!("qwen38-multimodal-cuda-smoke-{media_kind}"),
        service.model_descriptor().clone(),
        None,
        TokenSequence::new(
            plan.token_ids()
                .iter()
                .copied()
                .map(TokenId)
                .collect::<Vec<_>>(),
        ),
        GenerationOptions::greedy(2),
    );
    let generation_started = Instant::now();
    let response = service.generate_qwen38_multimodal(&request, &plan)?;
    let generation_duration_ns = duration_ns(generation_started.elapsed());
    let retained_receipt = service
        .last_qwen38_multimodal_plan_receipt()
        .ok_or("CUDA service did not retain the admitted multimodal plan receipt")?;
    if retained_receipt != plan.receipt() {
        return Err("CUDA service retained a different multimodal plan receipt".into());
    }
    let decoder = service.model_descriptor();

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema_version": "psionic.qwen38.multimodal_cuda_smoke.v1",
            "media_kind": media_kind,
            "official_model_dir": official_model_dir,
            "decoder_gguf": gguf_path,
            "decoder_model": {
                "model": &decoder.model,
                "config": &decoder.config,
                "tokenizer_family": &decoder.tokenizer_family,
                "weights_digest": &decoder.weights.digest,
                "artifact_identity": &decoder.artifact_identity,
            },
            "decoder_runtime": service.cuda_runtime_contract(),
            "vision_runtime": vision_runtime_receipt,
            "multimodal_plan_receipt": retained_receipt,
            "output_token_ids": response.output.tokens.as_slice().iter().map(|token| token.as_u32()).collect::<Vec<_>>(),
            "output_text": response.output.text,
            "termination": response.termination,
            "generation_metrics": response.metrics,
            "vision_duration_ns": vision_duration_ns,
            "decoder_load_duration_ns": decoder_load_duration_ns,
            "generation_duration_ns": generation_duration_ns,
            "total_duration_ns": duration_ns(started.elapsed()),
            "fallback_policy": "refuse",
            "hidden_fallback_used": false,
        }))?
    );
    Ok(())
}

fn deterministic_frame(frame_index: usize) -> Result<Qwen38RgbFrame, Box<dyn Error>> {
    let rgb8 = (0..(256 * 256))
        .flat_map(|pixel| {
            let x = pixel % 256;
            let y = pixel / 256;
            [
                ((x + frame_index * 17) % 256) as u8,
                ((y + frame_index * 29) % 256) as u8,
                (((x + y) / 2 + frame_index * 11) % 256) as u8,
            ]
        })
        .collect::<Vec<_>>();
    Ok(Qwen38RgbFrame::new(256, 256, rgb8)?)
}

fn duration_ns(duration: std::time::Duration) -> u64 {
    duration.as_nanos().try_into().unwrap_or(u64::MAX)
}
