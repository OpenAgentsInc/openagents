//! `dump`: load one checkpoint and print raw per-question logits and act
//! logits for a request, for parity debugging.
//!
//! ```text
//! cargo run --example dump -- ~/work/laya-artifacts/english request.json
//! ```

use std::process::ExitCode;

use candle_core::IndexOp;
use laya::api::SystemOneRequest;
use laya::decision::DecisionModel;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: dump MODEL_DIR REQUEST_JSON");
        return ExitCode::from(2);
    }
    let model = match DecisionModel::load(std::path::Path::new(&args[1]), candle_core::Device::Cpu)
    {
        Ok(m) => m,
        Err(e) => {
            eprintln!("load: {e}");
            return ExitCode::from(1);
        }
    };
    let body = std::fs::read_to_string(&args[2]).expect("request file");
    let request: SystemOneRequest = serde_json::from_str(&body).expect("request json");
    let (batch, metas, counts) = model.encode(&request).expect("encode");
    eprintln!(
        "batch: {} rows, n_tokens {}",
        batch.input_ids.len(),
        batch.n_tokens
    );
    let hidden = model.hidden(&batch).expect("hidden");
    let (logits, acts) = model.forward(&batch).expect("forward");
    let logits: Vec<Vec<f32>> = logits.to_vec2().expect("logits");
    let acts: Vec<Vec<f32>> = acts.to_vec2().expect("acts");
    for (i, meta) in metas.iter().enumerate() {
        let markers = &batch.marker_pos[i];
        for (j, &m) in markers.iter().enumerate() {
            if batch.marker_mask[i][j] {
                let row: Vec<f32> = hidden
                    .i((i, m as usize, ..6))
                    .expect("h")
                    .to_vec1()
                    .expect("h vec");
                println!("{} marker {}: h {:?}", meta.id, m, row);
            }
        }
    }
    for (i, meta) in metas.iter().enumerate() {
        println!(
            "{}: logits {:?} act {:?}",
            meta.id,
            &logits[i][..counts[i]],
            acts[i]
        );
    }
    ExitCode::SUCCESS
}
