use psionic_eval::{
    tassadar_compiled_distillation_report_path, write_tassadar_compiled_distillation_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_compiled_distillation_report_path();
    let report = write_tassadar_compiled_distillation_report(&output_path)?;
    println!(
        "wrote compiled distillation report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
