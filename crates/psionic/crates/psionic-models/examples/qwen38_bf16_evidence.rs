use std::{env, fs, path::PathBuf};

use psionic_models::{Qwen38Bf16EvidenceBackend, run_qwen38_bf16_evidence};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command_line = env::args().collect::<Vec<_>>();
    let mut model_dir = PathBuf::from("target/models/qwen/Qwen3.8-27B");
    let mut backend = Qwen38Bf16EvidenceBackend::HeaderAdmission;
    let mut json_out = None::<PathBuf>;
    let mut args = command_line.iter().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--model-dir" => {
                model_dir = PathBuf::from(args.next().ok_or("--model-dir requires a path")?);
            }
            "--backend" => {
                backend = args.next().ok_or("--backend requires a value")?.parse()?;
            }
            "--json-out" => {
                json_out = Some(PathBuf::from(
                    args.next().ok_or("--json-out requires a path")?,
                ));
            }
            "--help" | "-h" => {
                println!(
                    "usage: qwen38_bf16_evidence [--model-dir PATH] [--backend header-admission|sampled-projection|bounded-row-sparse-traversal] [--json-out PATH]"
                );
                return Ok(());
            }
            other => return Err(format!("unknown argument `{other}`").into()),
        }
    }
    let report = run_qwen38_bf16_evidence(model_dir, backend, command_line)?;
    let bytes = serde_json::to_vec_pretty(&report)?;
    if let Some(path) = json_out {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, &bytes)?;
        println!("{}", path.display());
    } else {
        println!("{}", String::from_utf8(bytes)?);
    }
    Ok(())
}
