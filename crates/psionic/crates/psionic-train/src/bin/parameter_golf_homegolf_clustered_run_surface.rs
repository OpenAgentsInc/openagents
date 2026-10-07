use std::{env, path::PathBuf};

use psionic_train::write_parameter_golf_homegolf_clustered_run_surface_report;

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
            PathBuf::from("/tmp/parameter_golf_homegolf_clustered_run_surface.json")
        });
    let report = write_parameter_golf_homegolf_clustered_run_surface_report(output_path.as_path())?;
    println!(
        "wrote {} observed_cluster_wallclock_ms={} final_validation_bits_per_byte={:.8} model_artifact_bytes={}",
        output_path.display(),
        report.observed_cluster_wallclock_ms,
        report.final_validation_bits_per_byte,
        report.model_artifact_bytes,
    );
    Ok(())
}
