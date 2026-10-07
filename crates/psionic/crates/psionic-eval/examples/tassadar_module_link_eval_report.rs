use psionic_eval::{tassadar_module_link_eval_report_path, write_tassadar_module_link_eval_report};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_module_link_eval_report_path();
    let report = write_tassadar_module_link_eval_report(&output_path)?;
    println!(
        "wrote module-link eval report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
