#![allow(clippy::all, clippy::expect_used, clippy::panic, clippy::pedantic, clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::gap_matrix::ParityGapMatrix;
use openagents_cad::parity::scorecard::build_scorecard;

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("{0}")]
    Usage(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

struct CliArgs {
    gap_matrix: PathBuf,
    output: PathBuf,
    check: bool,
}

fn main() -> Result<(), CliError> {
    let args = parse_args()?;
    let matrix: ParityGapMatrix = serde_json::from_str(&fs::read_to_string(&args.gap_matrix)?)?;
    let scorecard = build_scorecard(&matrix, &display_path(&args.gap_matrix));
    let serialized = format!("{}\n", serde_json::to_string_pretty(&scorecard)?);

    if args.check {
        let existing = fs::read_to_string(&args.output)?;
        if existing == serialized {
            println!("parity scorecard is up to date: {}", args.output.display());
            return Ok(());
        }
        return Err(CliError::Usage(format!(
            "scorecard drift detected: regenerate {}",
            args.output.display()
        )));
    }

    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.output, serialized)?;
    println!("wrote parity scorecard: {}", args.output.display());
    Ok(())
}

fn parse_args() -> Result<CliArgs, CliError> {
    let root = repo_root();
    let mut gap_matrix = root
        .join("crates/cad/parity")
        .join("vcad_openagents_gap_matrix.json");
    let mut output = root.join("crates/cad/parity").join("parity_scorecard.json");
    let mut check = false;

    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--gap-matrix" => {
                let Some(value) = iter.next() else {
                    return Err(CliError::Usage(usage("missing value for --gap-matrix")));
                };
                gap_matrix = PathBuf::from(value);
            }
            "--output" => {
                let Some(value) = iter.next() else {
                    return Err(CliError::Usage(usage("missing value for --output")));
                };
                output = PathBuf::from(value);
            }
            "--check" => {
                check = true;
            }
            "--help" | "-h" => {
                return Err(CliError::Usage(usage("")));
            }
            _ => {
                return Err(CliError::Usage(usage(&format!("unknown argument: {arg}"))));
            }
        }
    }

    Ok(CliArgs {
        gap_matrix,
        output,
        check,
    })
}

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(|path| path.to_path_buf())
        .unwrap_or(manifest_dir)
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn usage(error: &str) -> String {
    let mut lines = Vec::new();
    if !error.is_empty() {
        lines.push(error.to_string());
    }
    lines.push("Usage:".to_string());
    lines.push("  cargo run -p openagents-cad --bin parity-scorecard -- [options]".to_string());
    lines.push("Options:".to_string());
    lines.push("  --gap-matrix <path>   Path to parity gap matrix JSON".to_string());
    lines.push("  --output <path>       Scorecard output JSON".to_string());
    lines.push(
        "  --check               Fail if output differs from generated scorecard".to_string(),
    );
    lines.join("\n")
}
