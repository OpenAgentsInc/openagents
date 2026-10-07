use psionic_eval::{
    tassadar_workload_capability_frontier_report_path,
    write_tassadar_workload_capability_frontier_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_workload_capability_frontier_report_path();
    let report = write_tassadar_workload_capability_frontier_report(&output_path)?;
    println!(
        "wrote Tassadar workload capability frontier report with {} families to {}",
        report.frontier_rows.len(),
        output_path.display(),
    );
    Ok(())
}
