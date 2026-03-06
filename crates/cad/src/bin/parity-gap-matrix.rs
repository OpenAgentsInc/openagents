#![allow(
    clippy::all,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::unwrap_used
)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::gap_matrix::build_gap_matrix;
use openagents_cad::parity::openagents_crawler::OpenagentsCapabilityInventory;
use openagents_cad::parity::vcad_crawler::VcadCapabilityInventory;

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
    vcad_inventory: PathBuf,
    openagents_inventory: PathBuf,
    output: PathBuf,
    check: bool,
}

fn main() -> Result<(), CliError> {
    let args = parse_args()?;
    let vcad: VcadCapabilityInventory =
        serde_json::from_str(&fs::read_to_string(&args.vcad_inventory)?)?;
    let openagents: OpenagentsCapabilityInventory =
        serde_json::from_str(&fs::read_to_string(&args.openagents_inventory)?)?;

    let matrix = build_gap_matrix(
        &vcad,
        &openagents,
        &display_path(&args.vcad_inventory),
        &display_path(&args.openagents_inventory),
    );
    let serialized = format!("{}\n", serde_json::to_string_pretty(&matrix)?);

    if args.check {
        let existing = fs::read_to_string(&args.output)?;
        if existing == serialized {
            println!("parity gap matrix is up to date: {}", args.output.display());
            return Ok(());
        }
        return Err(CliError::Usage(format!(
            "gap matrix drift detected: regenerate {}",
            args.output.display()
        )));
    }

    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.output, serialized)?;
    println!("wrote parity gap matrix: {}", args.output.display());
    Ok(())
}

fn parse_args() -> Result<CliArgs, CliError> {
    let root = repo_root();
    let mut vcad_inventory = root
        .join("crates/cad/parity")
        .join("vcad_capabilities_inventory.json");
    let mut openagents_inventory = root
        .join("crates/cad/parity")
        .join("openagents_capabilities_inventory.json");
    let mut output = root
        .join("crates/cad/parity")
        .join("vcad_openagents_gap_matrix.json");
    let mut check = false;

    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--vcad-inventory" => {
                let Some(value) = iter.next() else {
                    return Err(CliError::Usage(usage("missing value for --vcad-inventory")));
                };
                vcad_inventory = PathBuf::from(value);
            }
            "--openagents-inventory" => {
                let Some(value) = iter.next() else {
                    return Err(CliError::Usage(usage(
                        "missing value for --openagents-inventory",
                    )));
                };
                openagents_inventory = PathBuf::from(value);
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
        vcad_inventory,
        openagents_inventory,
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
    lines.push("  cargo run -p openagents-cad --bin parity-gap-matrix -- [options]".to_string());
    lines.push("Options:".to_string());
    lines.push("  --vcad-inventory <path>         Path to vcad inventory JSON".to_string());
    lines.push("  --openagents-inventory <path>   Path to openagents inventory JSON".to_string());
    lines.push("  --output <path>                 Gap matrix output JSON".to_string());
    lines.push(
        "  --check                         Fail if output differs from generated matrix"
            .to_string(),
    );
    lines.join("\n")
}
