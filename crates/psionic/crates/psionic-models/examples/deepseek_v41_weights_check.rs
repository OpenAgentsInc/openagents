//! Check a DeepSeek-V4.1-Flash checkpoint directory against the pinned revision.
//!
//! ```text
//! cargo run --release -p psionic-models --example deepseek_v41_weights_check -- \
//!     DIR [--oracle MANIFEST] [--verify-sha256] [--headers-only] [--ranges 0-4,10-14,35-39]
//! ```
//!
//! `DIR` holds `model.safetensors.index.json` and the 48 shards. The command
//! opens every tensor (its first and last bytes, and the first and last rows
//! of every weight dequantized), totals bytes per layer and per range, and,
//! with `--oracle`, compares dequantized blocks with the reference's values
//! (`psionic.deepseek_v41.dequant_parity.v1`). `--verify-sha256` hashes all
//! 510 GB against the pin. `--headers-only` reads only the shard headers, so
//! `DIR` may hold header-only copies. It prints one JSON document and exits
//! nonzero when a check fails.

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::{env, path::PathBuf, process::ExitCode};

use psionic_models::deepseek_v41_weights::{WeightsCheckOptions, check_checkpoint};

fn main() -> Result<ExitCode, Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let mut dir = None;
    let mut options = WeightsCheckOptions::default();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--oracle" => {
                options.oracle = Some(PathBuf::from(args.next().ok_or("--oracle needs a path")?))
            }
            "--verify-sha256" => options.verify_sha256 = true,
            "--headers-only" => options.headers_only = true,
            "--ranges" => {
                let value = args
                    .next()
                    .ok_or("--ranges needs a list such as 0-4,10-14")?;
                options.ranges = value
                    .split(',')
                    .map(|range| {
                        let (first, last) =
                            range.split_once('-').ok_or("a range looks like 10-14")?;
                        Ok::<_, Box<dyn std::error::Error>>((first.parse()?, last.parse()?))
                    })
                    .collect::<Result<_, _>>()?;
            }
            other if dir.is_none() && !other.starts_with("--") => dir = Some(PathBuf::from(other)),
            other => return Err(format!("unexpected argument `{other}`").into()),
        }
    }
    let dir = dir.ok_or("pass the checkpoint directory")?;
    let outcome = check_checkpoint(&dir, &options)?;
    println!("{}", serde_json::to_string_pretty(&outcome)?);
    if outcome.passed() {
        Ok(ExitCode::SUCCESS)
    } else {
        eprintln!("The checkpoint check failed; see `failures`, `nan_values`, and `parity`.");
        Ok(ExitCode::FAILURE)
    }
}
