use std::{env, path::PathBuf};

use psionic_train::write_parameter_golf_homegolf_public_comparison_report;

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
        .unwrap_or_else(|| PathBuf::from("/tmp/parameter_golf_homegolf_public_comparison.json"));
    let report = write_parameter_golf_homegolf_public_comparison_report(output_path.as_path())?;
    println!(
        "wrote {} delta_vs_baseline_val_bpb={:.8} delta_vs_leader_val_bpb={:.8}",
        output_path.display(),
        report.delta_vs_public_naive_baseline.delta_val_bpb,
        report
            .delta_vs_current_public_leaderboard_best
            .delta_val_bpb,
    );
    Ok(())
}
