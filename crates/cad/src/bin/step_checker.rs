use std::path::PathBuf;

use openagents_cad::step_checker::{
    CadStepCheckerBackend, CadStepCheckerReport, check_step_file_with_backend,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut backend = "structural".to_string();
    let mut opencascade_program: Option<String> = None;
    let mut opencascade_script: Option<String> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            "--input" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--input requires a path argument".to_string())?;
                input = Some(PathBuf::from(value));
            }
            "--output" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--output requires a path argument".to_string())?;
                output = Some(PathBuf::from(value));
            }
            "--backend" => {
                backend = args.next().ok_or_else(|| {
                    "--backend requires one of: structural, opencascade".to_string()
                })?;
            }
            "--opencascade-program" => {
                opencascade_program = Some(
                    args.next()
                        .ok_or_else(|| "--opencascade-program requires a value".to_string())?,
                );
            }
            "--opencascade-script" => {
                opencascade_script = Some(
                    args.next()
                        .ok_or_else(|| "--opencascade-script requires a value".to_string())?,
                );
            }
            other => {
                return Err(format!("unknown argument: {other}"));
            }
        }
    }

    let input = input.ok_or_else(|| "missing --input <path>".to_string())?;
    let backend = match backend.as_str() {
        "structural" => CadStepCheckerBackend::Structural,
        "opencascade" => {
            let program = opencascade_program.unwrap_or_else(|| "python3".to_string());
            let script = opencascade_script
                .unwrap_or_else(|| "scripts/cad/opencascade_step_checker.py".to_string());
            CadStepCheckerBackend::OpenCascadeCommand {
                program,
                args: vec![script],
            }
        }
        other => {
            return Err(format!(
                "invalid --backend value {other}; expected structural or opencascade"
            ));
        }
    };

    let report = check_step_file_with_backend(&input, backend)
        .map_err(|error| format!("step checker failed: {error}"))?;
    emit_report(&report, output.as_ref())?;

    if report.passed {
        Ok(())
    } else {
        Err("step checker reported failures".to_string())
    }
}

fn emit_report(report: &CadStepCheckerReport, output: Option<&PathBuf>) -> Result<(), String> {
    let json = serde_json::to_string_pretty(report)
        .map_err(|error| format!("failed to encode report json: {error}"))?;
    if let Some(path) = output {
        std::fs::write(path, &json)
            .map_err(|error| format!("failed to write report {}: {error}", path.display()))?;
    }
    println!("{json}");
    Ok(())
}

fn print_help() {
    println!(
        "Usage: step_checker --input <path> [--output <path>] [--backend structural|opencascade] [--opencascade-program <cmd>] [--opencascade-script <path>]"
    );
}
