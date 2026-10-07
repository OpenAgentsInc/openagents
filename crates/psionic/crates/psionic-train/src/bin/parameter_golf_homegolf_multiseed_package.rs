use std::{env, path::PathBuf};

use psionic_train::write_parameter_golf_homegolf_multiseed_package_report;

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
        .unwrap_or_else(|| PathBuf::from("/tmp/parameter_golf_homegolf_multiseed_package.json"));
    let report = write_parameter_golf_homegolf_multiseed_package_report(output_path.as_path())?;
    println!(
        "wrote {} seed_runs={} mean_val_bpb={:.8} stddev_val_bpb={:.8}",
        output_path.display(),
        report.seed_runs.len(),
        report.mean_validation_bits_per_byte,
        report.stddev_validation_bits_per_byte,
    );
    Ok(())
}
