use std::{env, error::Error, path::PathBuf};

use psionic_train::write_qwen_legal_checkpoint_recovery_rehearsal;

fn main() -> Result<(), Box<dyn Error>> {
    let mut out = PathBuf::from("target/legal/qwen_checkpoint_recovery/rehearsal-001");
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => {
                let value = args.next().ok_or("--out requires a path")?;
                out = PathBuf::from(value);
            }
            "--help" | "-h" => {
                eprintln!(
                    "usage: cargo run -p psionic-train --example qwen_legal_checkpoint_recovery_rehearsal -- [--out <dir>]"
                );
                return Ok(());
            }
            other => return Err(format!("unsupported argument `{other}`").into()),
        }
    }
    let report = write_qwen_legal_checkpoint_recovery_rehearsal(out.as_path())?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
