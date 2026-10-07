use psionic_research::{
    tassadar_workload_capability_frontier_summary_report_path,
    write_tassadar_workload_capability_frontier_summary_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_workload_capability_frontier_summary_report_path();
    let report = write_tassadar_workload_capability_frontier_summary_report(&output_path)?;
    println!(
        "wrote Tassadar workload capability frontier summary for {} families to {}",
        report.frontier_report.frontier_rows.len(),
        output_path.display(),
    );
    Ok(())
}
