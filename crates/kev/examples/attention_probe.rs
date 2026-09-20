//! Compare attention and padding on fixed encodings with one loaded checkpoint.

use std::path::Path;
use std::time::Instant;

use candle_core::{DType, Device};
use kev::model::AttentionBackend;
use kev::{DecisionModel, Record, SystemOneRequest, to_record};
use serde_json::{Value, json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [base, adapter, fixtures, precision] = args.as_slice() else {
        return Err("usage: attention_probe BASE ADAPTER FIXTURES f32|bf16".into());
    };
    let dtype = match precision.as_str() {
        "f32" => DType::F32,
        "bf16" => DType::BF16,
        _ => return Err("precision must be f32 or bf16".into()),
    };
    let mut model = DecisionModel::load_with_dtype(
        Path::new(base),
        Path::new(adapter),
        Device::new_metal(0)?,
        dtype,
    )?;
    let mut records = Vec::<(String, Record, bool)>::new();
    let mut paths: Vec<_> = std::fs::read_dir(Path::new(fixtures).join("encodings"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<_>>()?;
    paths.sort();
    for path in paths {
        let body: Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        records.push((
            path.file_stem().unwrap().to_string_lossy().into_owned(),
            serde_json::from_value(body["record"].clone())?,
            false,
        ));
    }
    for (length, copies) in [("short", 2), ("long", 48)] {
        for count in [1, 5] {
            let mut questions = serde_json::Map::new();
            for q in 0..count {
                questions.insert(format!("route_{q}"), json!({
                    "type": "choice", "instructions": "Which team should handle the request?",
                    "criteria": {"returns": "Exchanges and refunds", "shipping": "Delivery delays", "billing": "Payment problems"},
                }));
            }
            let request: SystemOneRequest = serde_json::from_value(json!({
                "state": "The customer received the wrong size shoes and requests an exchange. ".repeat(copies),
                "questions": questions,
            }))?;
            records.push((
                format!("benchmark_{length}_{count}"),
                to_record(&request)?.0,
                true,
            ));
        }
    }
    let mut modes = Vec::new();
    for backend in [AttentionBackend::Eager, AttentionBackend::MetalSdpa] {
        for bucket in [0, 64] {
            model.backbone.set_attention(backend)?;
            model.set_bucket_size(bucket)?;
            let mut rows = Vec::new();
            for (name, record, profile) in &records {
                let enc = model.encode(record, 8192, 8192)?;
                let mut times = Vec::new();
                let mut probs = Vec::new();
                for _ in 0..2 {
                    let start = Instant::now();
                    probs = model.probs(&enc)?;
                    model.device.synchronize()?;
                    times.push(start.elapsed().as_secs_f64() * 1000.0);
                }
                let stages = if *profile {
                    let (profiled_probs, stages) = model.profile_probs(&enc)?;
                    let drift = probs
                        .iter()
                        .flatten()
                        .zip(profiled_probs.iter().flatten())
                        .map(|(a, b)| (a - b).abs())
                        .fold(0.0_f64, f64::max);
                    if drift > 1e-6 {
                        return Err(format!("profiling changed {name}: {drift}").into());
                    }
                    Some(stages)
                } else {
                    None
                };
                rows.push(json!({"name": name, "tokens": enc.ids.len(),
                    "forward_tokens": model.forward_tokens(enc.ids.len()), "probs": probs,
                    "first_ms": times[0], "second_ms": times[1], "intrusive_profile": stages}));
            }
            modes.push(
                json!({"attention": backend.identity(), "bucket_size": bucket, "records": rows}),
            );
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "artifact_identity": model.artifacts, "dtype": precision, "backend": "metal", "modes": modes,
        }))?
    );
    Ok(())
}
