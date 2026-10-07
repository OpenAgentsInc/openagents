use psionic_eval::{
    tassadar_kernel_module_scaling_report_path, write_tassadar_kernel_module_scaling_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_kernel_module_scaling_report_path();
    let report = write_tassadar_kernel_module_scaling_report(&output_path)?;
    println!(
        "wrote kernel-module scaling report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
