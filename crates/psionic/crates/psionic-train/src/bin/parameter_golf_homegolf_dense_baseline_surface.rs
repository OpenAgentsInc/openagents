use std::{env, path::PathBuf};

use psionic_train::{
    PARAMETER_GOLF_HOMEGOLF_DENSE_BASELINE_SURFACE_REPORT_REF,
    write_parameter_golf_homegolf_dense_baseline_surface_report,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(PARAMETER_GOLF_HOMEGOLF_DENSE_BASELINE_SURFACE_REPORT_REF)
    });
    let report = write_parameter_golf_homegolf_dense_baseline_surface_report(&output_path)?;
    println!(
        "wrote {} baseline_model_id={} val_bpb={:.8} wallclock_ms={} artifact_bytes={}",
        output_path.display(),
        report.baseline_model_id,
        report.final_metric.val_bpb,
        report.observed_wallclock_ms,
        report.compressed_model_bytes,
    );
    Ok(())
}
