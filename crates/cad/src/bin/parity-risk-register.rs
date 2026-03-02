use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use openagents_cad::parity::fixture_corpus::ParityFixtureCorpus;
use openagents_cad::parity::risk_register::build_risk_register;
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
    fixture_corpus: PathBuf,
    scorecard: PathBuf,
    output: PathBuf,
    check: bool,
    enforce_profile: Option<String>,
}

fn main() -> Result<(), CliError> {
    let args = parse_args()?;
    let fixture_corpus: ParityFixtureCorpus =
        serde_json::from_str(&fs::read_to_string(&args.fixture_corpus)?)?;
    let scorecard: ParityScorecard = serde_json::from_str(&fs::read_to_string(&args.scorecard)?)?;
    let register = build_risk_register(
        &fixture_corpus,
        &scorecard,
        &display_path(&args.fixture_corpus),
        &display_path(&args.scorecard),
    );

    if let Some(profile_id) = args.enforce_profile.as_deref() {
        let evaluation = register
            .workflow
            .evaluations
            .iter()
            .find(|evaluation| evaluation.profile_id == profile_id)
            .ok_or_else(|| CliError::Usage(format!("unknown profile id: {profile_id}")))?;
        if !evaluation.pass {
            return Err(CliError::Usage(format!(
                "blocker profile failed: {} (open_hard_blockers={}, open_total={})",
                profile_id, evaluation.open_hard_blockers, evaluation.open_total
            )));
        }
    }

    let serialized = format!("{}\n", serde_json::to_string_pretty(&register)?);
    if args.check {
        let existing = fs::read_to_string(&args.output)?;
        if existing == serialized {
            println!(
                "parity risk register is up to date: {}",
                args.output.display()
            );
            return Ok(());
        }
        return Err(CliError::Usage(format!(
            "risk register drift detected: regenerate {}",
            args.output.display()
        )));
    }

    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.output, serialized)?;
    println!("wrote parity risk register: {}", args.output.display());
    Ok(())
}

fn parse_args() -> Result<CliArgs, CliError> {
    let root = repo_root();
    let mut fixture_corpus = root
        .join("crates/cad/parity/fixtures")
        .join("parity_fixture_corpus.json");
    let mut scorecard = root.join("crates/cad/parity").join("parity_scorecard.json");
    let mut output = root
        .join("crates/cad/parity")
        .join("parity_risk_register.json");
    let mut check = false;
    let mut enforce_profile = None;

    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--fixture-corpus" => {
                let Some(value) = iter.next() else {
                    return Err(CliError::Usage(usage("missing value for --fixture-corpus")));
                };
                fixture_corpus = PathBuf::from(value);
            }
            "--scorecard" => {
                let Some(value) = iter.next() else {
                    return Err(CliError::Usage(usage("missing value for --scorecard")));
                };
                scorecard = PathBuf::from(value);
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
            "--enforce-profile" => {
                let Some(value) = iter.next() else {
                    return Err(CliError::Usage(usage(
                        "missing value for --enforce-profile",
                    )));
                };
                enforce_profile = Some(value);
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
        fixture_corpus,
        scorecard,
        output,
        check,
        enforce_profile,
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
    lines.push("  cargo run -p openagents-cad --bin parity-risk-register -- [options]".to_string());
    lines.push("Options:".to_string());
    lines.push("  --fixture-corpus <path>  Path to parity fixture corpus JSON".to_string());
    lines.push("  --scorecard <path>       Path to parity scorecard JSON".to_string());
    lines.push("  --output <path>          Risk register output JSON".to_string());
    lines.push(
        "  --check                  Fail if output differs from generated register".to_string(),
    );
    lines.push("  --enforce-profile <id>   Enforce blocker profile pass/fail".to_string());
    lines.join("\n")
}
