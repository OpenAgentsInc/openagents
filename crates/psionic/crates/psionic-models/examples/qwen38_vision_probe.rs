use std::{env, error::Error};

use psionic_models::{
    Qwen38NativeVisionRuntime, Qwen38RgbFrame, Qwen38VisionAdmissionLimits,
    Qwen38VisionRuntimeBackend, qwen38_preprocess_image, qwen38_preprocess_video,
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let model_dir = args
        .next()
        .ok_or("usage: qwen38_vision_probe <official-model-dir> <cpu|cuda> [image|video]")?;
    let backend = match args.next().as_deref() {
        Some("cpu") => Qwen38VisionRuntimeBackend::Cpu,
        #[cfg(feature = "qwen38-vision-cuda")]
        Some("cuda") => Qwen38VisionRuntimeBackend::Cuda { device_ordinal: 0 },
        Some(other) => return Err(format!("unsupported backend `{other}`").into()),
        None => return Err("missing backend argument".into()),
    };
    let media = args.next().unwrap_or_else(|| String::from("image"));
    if args.next().is_some() {
        return Err("unexpected trailing arguments".into());
    }

    let mut limits = Qwen38VisionAdmissionLimits::default();
    limits.timeout_ms = 120_000;
    let input = match media.as_str() {
        "image" => {
            let frame = deterministic_frame(0)?;
            qwen38_preprocess_image("qwen38-gradient-256", "image/raw-rgb8", &frame, limits)?
        }
        "video" => {
            let frames = (0..8)
                .map(deterministic_frame)
                .collect::<Result<Vec<_>, _>>()?;
            qwen38_preprocess_video(
                "qwen38-gradient-video-8x256",
                "video/raw-rgb8-frames",
                frames.as_slice(),
                4.0,
                limits,
            )?
        }
        other => return Err(format!("unsupported media kind `{other}`").into()),
    };
    let runtime = Qwen38NativeVisionRuntime::from_official_model_dir(model_dir, backend)?;
    let output = runtime.encode(&input)?;
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "preprocessing": input.receipt,
            "pixel_values": input.pixel_values,
            "runtime": output.receipt,
            "embeddings": output.embeddings,
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
