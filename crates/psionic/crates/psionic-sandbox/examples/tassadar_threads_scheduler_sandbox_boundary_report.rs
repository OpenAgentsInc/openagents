use psionic_sandbox::{
    tassadar_threads_scheduler_sandbox_boundary_report_path,
    write_tassadar_threads_scheduler_sandbox_boundary_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_threads_scheduler_sandbox_boundary_report_path();
    let report = write_tassadar_threads_scheduler_sandbox_boundary_report(&output_path)?;
    println!(
        "wrote threads scheduler sandbox boundary report to {} ({})",
        output_path.display(),
        report.report_id
    );
    Ok(())
}
