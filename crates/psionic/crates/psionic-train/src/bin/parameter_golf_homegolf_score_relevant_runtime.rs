use std::path::PathBuf;

use psionic_train::write_parameter_golf_homegolf_score_runtime_report;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from("/tmp/parameter_golf_homegolf_score_relevant_runtime.json")
        });
    let report = write_parameter_golf_homegolf_score_runtime_report(output_path.as_path())?;
    println!(
        "wrote {} report_digest={} projected_dataset_passes_within_cap={:.4}",
        output_path.display(),
        report.report_digest,
        report.projected_dataset_passes_within_cap,
    );
    Ok(())
}
