use psionic_eval::{
    tassadar_effect_safe_resume_report_path, write_tassadar_effect_safe_resume_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_effect_safe_resume_report_path();
    let report = write_tassadar_effect_safe_resume_report(&output_path)?;
    println!(
        "wrote effect-safe resume report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
