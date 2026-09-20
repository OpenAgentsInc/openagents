//! Export probabilities and execution identity for a pinned fixture corpus.

use std::path::Path;
use std::time::Instant;

use candle_core::{DType, Device};
use kev::{DecisionModel, Record};
use serde_json::{Value, json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [base, adapter, fixtures, backend, precision] = args.as_slice() else {
        return Err("usage: precision_probe BASE ADAPTER FIXTURES cpu|metal f32|bf16".into());
    };
    let device = match backend.as_str() {
        "cpu" => Device::Cpu,
        "metal" => Device::new_metal(0)?,
        _ => return Err("backend must be cpu or metal".into()),
    };
    let dtype = match precision.as_str() {
        "f32" => DType::F32,
        "bf16" => DType::BF16,
        _ => return Err("precision must be f32 or bf16".into()),
    };
    let start = Instant::now();
    let model = DecisionModel::load_with_dtype(Path::new(base), Path::new(adapter), device, dtype)?;
    model.device.synchronize()?;
    let load_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut records = Vec::new();
    if fixtures != "load-only" {
        let mut paths: Vec<_> = std::fs::read_dir(Path::new(fixtures).join("encodings"))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<_>>()?;
        paths.sort();
        for path in paths {
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let body: Value = serde_json::from_slice(&std::fs::read(&path)?)?;
            let record: Record = serde_json::from_value(body["record"].clone())?;
            let enc = model.encode(&record, 8192, 8192)?;
            let start = Instant::now();
            let probs = model.probs(&enc)?;
            model.device.synchronize()?;
            records.push(json!({
                "name": path.file_stem().and_then(|name| name.to_str()),
                "tokens": enc.ids.len(), "probs": probs,
                "elapsed_ms": start.elapsed().as_secs_f64() * 1000.0,
            }));
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "artifact_identity": model.artifacts, "backend": backend,
            "dtype": precision, "head_dtype": format!("{:?}", model.head.q.weight.dtype()),
            "merge": "fp32-before-cast-v1", "load_ms": load_ms, "records": records,
        }))?
    );
    Ok(())
}
