use std::{env, path::PathBuf};

use psionic_train::write_parameter_golf_homegolf_artifact_accounting_report;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from("/tmp/parameter_golf_homegolf_artifact_accounting.json")
        });
    let report = write_parameter_golf_homegolf_artifact_accounting_report(output_path.as_path())?;
    println!(
        "wrote {} total_counted_bytes={} cap_delta_bytes={} budget_status={:?}",
        output_path.display(),
        report.total_counted_bytes,
        report.cap_delta_bytes,
        report.budget_status,
    );
    Ok(())
}
