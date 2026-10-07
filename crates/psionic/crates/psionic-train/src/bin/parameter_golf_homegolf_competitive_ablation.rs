use std::path::PathBuf;

use psionic_train::write_parameter_golf_homegolf_competitive_ablation_report;

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
        .unwrap_or_else(|| PathBuf::from("/tmp/parameter_golf_homegolf_competitive_ablation.json"));
    let report = write_parameter_golf_homegolf_competitive_ablation_report(output_path.as_path())?;
    println!(
        "wrote {} report_digest={} best_known_variant={}",
        output_path.display(),
        report.report_digest,
        report.best_known_lane.model_variant.as_str(),
    );
    Ok(())
}
