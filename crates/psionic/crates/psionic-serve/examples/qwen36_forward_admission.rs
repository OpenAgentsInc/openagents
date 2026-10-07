use std::{env, error::Error, fs, io, path::PathBuf};

use psionic_models::{run_qwen36_forward_admission_with_command_line, QWEN36_27B_REAL_MODEL_DIR};

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();
    let model_dir = optional_flag(&args, "--model-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(QWEN36_27B_REAL_MODEL_DIR));
    let prompt = required_flag(&args, "--prompt")?;
    let backend = optional_flag(&args, "--backend").unwrap_or_else(|| String::from("local"));
    let report = run_qwen36_forward_admission_with_command_line(
        &model_dir,
        PathBuf::from(prompt),
        &backend,
        args.clone(),
    )?;
    let json = serde_json::to_string_pretty(&report)?;
    if let Some(out) = optional_flag(&args, "--out") {
        let out = PathBuf::from(out);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(out, json)?;
    } else {
        println!("{json}");
    }
    Ok(())
}

fn required_flag(args: &[String], flag: &str) -> Result<String, io::Error> {
    optional_flag(args, flag).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: qwen36_forward_admission --prompt fixtures/legal/smoke.prompt [--model-dir target/models/qwen/Qwen3.6-27B] [--backend local-header-admission|local-sampled-projection|local-full-forward] [--out path]",
        )
    })
}

fn optional_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find_map(|window| (window[0] == flag).then(|| window[1].clone()))
}
