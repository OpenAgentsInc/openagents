use std::env;
use std::fs;
use std::path::PathBuf;

use psionic_research::{ResearchRunner, ResearchRunnerError, ResearchRunnerInvocation};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), ResearchRunnerError> {
    let mut args = env::args().skip(1);
    let mut invocation_path = None;
    let mut result_path = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--invocation" => invocation_path = args.next().map(PathBuf::from),
            "--result" => result_path = args.next().map(PathBuf::from),
            _ => {
                return Err(ResearchRunnerError::InvalidInvocation(format!(
                    "unknown argument `{arg}`"
                )));
            }
        }
    }
    let Some(invocation_path) = invocation_path else {
        return Err(ResearchRunnerError::InvalidInvocation(String::from(
            "missing --invocation <path>",
        )));
    };
    let Some(result_path) = result_path else {
        return Err(ResearchRunnerError::InvalidInvocation(String::from(
            "missing --result <path>",
        )));
    };
    let invocation_bytes =
        fs::read(&invocation_path).map_err(|error| ResearchRunnerError::PersistFailure {
            path: invocation_path.display().to_string(),
            detail: error.to_string(),
        })?;
    let invocation: ResearchRunnerInvocation =
        serde_json::from_slice(&invocation_bytes).map_err(|error| {
            ResearchRunnerError::InvalidInvocation(format!(
                "failed to parse invocation at {}: {error}",
                invocation_path.display()
            ))
        })?;
    let record = ResearchRunner::execute_local(&invocation)?;
    ResearchRunner::persist(&record, &result_path)?;
    Ok(())
}
