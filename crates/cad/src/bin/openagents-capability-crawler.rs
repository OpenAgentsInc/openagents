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

use openagents_cad::parity::openagents_crawler::{
    OPENAGENTS_PARITY_PINNED_COMMIT, OpenagentsCrawlerError, crawl_openagents_capabilities,
};

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Crawler(#[from] OpenagentsCrawlerError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

struct CliArgs {
    openagents_repo: PathBuf,
    commit: String,
    output: PathBuf,
    check: bool,
}

fn main() -> Result<(), CliError> {
    let args = parse_args()?;
    let inventory = crawl_openagents_capabilities(&args.openagents_repo, &args.commit)?;
    let serialized = format!("{}\n", serde_json::to_string_pretty(&inventory)?);

    if args.check {
        let existing = fs::read_to_string(&args.output)?;
        if existing == serialized {
            println!(
                "openagents capability inventory is up to date: {}",
                args.output.display()
            );
            return Ok(());
        }
        return Err(CliError::Usage(format!(
            "inventory drift detected: regenerate {}",
            args.output.display()
        )));
    }

    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.output, serialized)?;
    println!(
        "wrote openagents capability inventory: {}",
        args.output.display()
    );
    Ok(())
}

fn parse_args() -> Result<CliArgs, CliError> {
    let default_repo = repo_root();
    let default_output = repo_root()
        .join("crates/cad/parity")
        .join("openagents_capabilities_inventory.json");

    let mut openagents_repo = default_repo;
    let mut commit = OPENAGENTS_PARITY_PINNED_COMMIT.to_string();
    let mut output = default_output;
    let mut check = false;

    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--openagents-repo" => {
                let Some(value) = iter.next() else {
                    return Err(CliError::Usage(usage(
                        "missing value for --openagents-repo",
                    )));
                };
                openagents_repo = PathBuf::from(value);
            }
            "--commit" => {
                let Some(value) = iter.next() else {
                    return Err(CliError::Usage(usage("missing value for --commit")));
                };
                commit = value;
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
        openagents_repo,
        commit,
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

fn usage(error: &str) -> String {
    let mut lines = Vec::new();
    if !error.is_empty() {
        lines.push(error.to_string());
    }
    lines.push("Usage:".to_string());
    lines.push(
        "  cargo run -p openagents-cad --bin openagents-capability-crawler -- [options]"
            .to_string(),
    );
    lines.push("Options:".to_string());
    lines.push("  --openagents-repo <path>   Path to openagents repo".to_string());
    lines.push(format!(
        "  --commit <sha>             Commit to crawl (default: {OPENAGENTS_PARITY_PINNED_COMMIT})"
    ));
    lines.push("  --output <path>            Inventory JSON path".to_string());
    lines.push(
        "  --check                    Fail if output differs from generated inventory".to_string(),
    );
    lines.join("\n")
}
