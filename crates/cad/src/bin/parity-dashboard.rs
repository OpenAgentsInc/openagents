use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::ci_artifacts::ParityCiArtifactManifest;
use openagents_cad::parity::dashboard::{build_dashboard, render_dashboard_markdown};
use openagents_cad::parity::risk_register::ParityRiskRegister;
use openagents_cad::parity::scorecard::ParityScorecard;

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
    scorecard: PathBuf,
    risk_register: PathBuf,
    ci_manifest: PathBuf,
    output_json: PathBuf,
    output_markdown: PathBuf,
    check: bool,
}

fn main() -> Result<(), CliError> {
    let args = parse_args()?;
    let scorecard: ParityScorecard = serde_json::from_str(&fs::read_to_string(&args.scorecard)?)?;
    let risk_register: ParityRiskRegister =
        serde_json::from_str(&fs::read_to_string(&args.risk_register)?)?;
    let ci_manifest: ParityCiArtifactManifest =
        serde_json::from_str(&fs::read_to_string(&args.ci_manifest)?)?;
    let dashboard = build_dashboard(
        &scorecard,
        &risk_register,
        &ci_manifest,
        &display_path(&args.scorecard),
        &display_path(&args.risk_register),
        &display_path(&args.ci_manifest),
    );
    let json_output = format!("{}\n", serde_json::to_string_pretty(&dashboard)?);
    let markdown_output = render_dashboard_markdown(&dashboard);

    if args.check {
        let existing_json = fs::read_to_string(&args.output_json)?;
        if existing_json != json_output {
            return Err(CliError::Usage(format!(
                "dashboard json drift detected: regenerate {}",
                args.output_json.display()
            )));
        }
        let existing_markdown = fs::read_to_string(&args.output_markdown)?;
        if existing_markdown != markdown_output {
            return Err(CliError::Usage(format!(
                "dashboard markdown drift detected: regenerate {}",
                args.output_markdown.display()
            )));
        }
        println!(
            "parity dashboard is up to date: {} and {}",
            args.output_json.display(),
            args.output_markdown.display()
        );
        return Ok(());
    }

    if let Some(parent) = args.output_json.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = args.output_markdown.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.output_json, json_output)?;
    fs::write(&args.output_markdown, markdown_output)?;
    println!(
        "wrote parity dashboard: {} and {}",
        args.output_json.display(),
        args.output_markdown.display()
    );
    Ok(())
}

fn parse_args() -> Result<CliArgs, CliError> {
    let root = repo_root();
    let mut scorecard = root.join("crates/cad/parity").join("parity_scorecard.json");
    let mut risk_register = root
        .join("crates/cad/parity")
        .join("parity_risk_register.json");
    let mut ci_manifest = root
        .join("crates/cad/parity")
        .join("parity_ci_artifact_manifest.json");
    let mut output_json = root.join("crates/cad/parity").join("parity_dashboard.json");
    let mut output_markdown = root
        .join("crates/cad/docs")
        .join("PARITY_BASELINE_DASHBOARD.md");
    let mut check = false;

    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--scorecard" => {
                let Some(value) = iter.next() else {
                    return Err(CliError::Usage(usage("missing value for --scorecard")));
                };
                scorecard = PathBuf::from(value);
            }
            "--risk-register" => {
                let Some(value) = iter.next() else {
                    return Err(CliError::Usage(usage("missing value for --risk-register")));
                };
                risk_register = PathBuf::from(value);
            }
            "--ci-manifest" => {
                let Some(value) = iter.next() else {
                    return Err(CliError::Usage(usage("missing value for --ci-manifest")));
                };
                ci_manifest = PathBuf::from(value);
            }
            "--output-json" => {
                let Some(value) = iter.next() else {
                    return Err(CliError::Usage(usage("missing value for --output-json")));
                };
                output_json = PathBuf::from(value);
            }
            "--output-markdown" => {
                let Some(value) = iter.next() else {
                    return Err(CliError::Usage(usage(
                        "missing value for --output-markdown",
                    )));
                };
                output_markdown = PathBuf::from(value);
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
        scorecard,
        risk_register,
        ci_manifest,
        output_json,
        output_markdown,
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
    lines.push("  cargo run -p openagents-cad --bin parity-dashboard -- [options]".to_string());
    lines.push("Options:".to_string());
    lines.push("  --scorecard <path>        Path to parity scorecard JSON".to_string());
    lines.push("  --risk-register <path>    Path to parity risk register JSON".to_string());
    lines.push("  --ci-manifest <path>      Path to parity CI artifact manifest JSON".to_string());
    lines.push("  --output-json <path>      Dashboard JSON output path".to_string());
    lines.push("  --output-markdown <path>  Dashboard markdown output path".to_string());
    lines.push(
        "  --check                   Fail if outputs differ from generated dashboard".to_string(),
    );
    lines.join("\n")
}
