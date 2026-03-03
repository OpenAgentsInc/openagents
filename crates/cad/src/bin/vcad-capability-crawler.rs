use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::vcad_crawler::{
    VCAD_PARITY_PINNED_COMMIT, VcadCrawlerError, crawl_vcad_capabilities,
};

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Crawler(#[from] VcadCrawlerError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

struct CliArgs {
    vcad_repo: PathBuf,
    commit: String,
    output: PathBuf,
    check: bool,
}

fn main() -> Result<(), CliError> {
    let args = parse_args()?;
    let inventory = crawl_vcad_capabilities(&args.vcad_repo, &args.commit)?;
    let serialized = format!("{}\n", serde_json::to_string_pretty(&inventory)?);

    if args.check {
        let existing = fs::read_to_string(&args.output)?;
        if existing == serialized {
            println!(
                "vcad capability inventory is up to date: {}",
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
    println!("wrote vcad capability inventory: {}", args.output.display());
    Ok(())
}

fn parse_args() -> Result<CliArgs, CliError> {
    let default_repo = default_vcad_repo();
    let default_output = repo_root()
        .join("crates/cad/parity")
        .join("vcad_capabilities_inventory.json");

    let mut vcad_repo = default_repo;
    let mut commit = VCAD_PARITY_PINNED_COMMIT.to_string();
    let mut output = default_output;
    let mut check = false;

    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--vcad-repo" => {
                let Some(value) = iter.next() else {
                    return Err(CliError::Usage(usage("missing value for --vcad-repo")));
                };
                vcad_repo = PathBuf::from(value);
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
        vcad_repo,
        commit,
        output,
        check,
    })
}

fn default_vcad_repo() -> PathBuf {
    if let Ok(path) = env::var("VCAD_REPO") {
        return PathBuf::from(path);
    }
    if let Ok(home) = env::var("HOME") {
        return PathBuf::from(home).join("code/vcad");
    }
    PathBuf::from("/home/christopherdavid/code/vcad")
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
        "  cargo run -p openagents-cad --bin vcad-capability-crawler -- [options]".to_string(),
    );
    lines.push("Options:".to_string());
    lines.push(
        "  --vcad-repo <path>   Path to vcad repo (default: $VCAD_REPO or $HOME/code/vcad)"
            .to_string(),
    );
    lines.push(format!(
        "  --commit <sha>       Commit to crawl (default: {VCAD_PARITY_PINNED_COMMIT})"
    ));
    lines.push("  --output <path>      Inventory JSON path".to_string());
    lines
        .push("  --check              Fail if output differs from generated inventory".to_string());
    lines.join("\n")
}
