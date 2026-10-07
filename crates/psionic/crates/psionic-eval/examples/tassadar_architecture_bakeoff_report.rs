use psionic_eval::{
    tassadar_architecture_bakeoff_report_path, write_tassadar_architecture_bakeoff_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_architecture_bakeoff_report_path();
    let report = write_tassadar_architecture_bakeoff_report(&output_path)?;
    println!(
        "wrote architecture bakeoff report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
