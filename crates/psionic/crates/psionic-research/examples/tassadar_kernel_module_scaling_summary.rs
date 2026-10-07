use psionic_research::{
    tassadar_kernel_module_scaling_summary_report_path,
    write_tassadar_kernel_module_scaling_summary_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_kernel_module_scaling_summary_report_path();
    let report = write_tassadar_kernel_module_scaling_summary_report(&output_path)?;
    println!(
        "wrote kernel-module scaling summary to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
