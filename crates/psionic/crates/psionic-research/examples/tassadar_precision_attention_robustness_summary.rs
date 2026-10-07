use psionic_research::{
    tassadar_precision_attention_robustness_summary_report_path,
    write_tassadar_precision_attention_robustness_summary_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_precision_attention_robustness_summary_report_path();
    let report = write_tassadar_precision_attention_robustness_summary_report(&output_path)?;
    println!(
        "wrote Tassadar precision/attention robustness summary for {} workloads to {}",
        report.audit_report.workload_summaries.len(),
        output_path.display(),
    );
    Ok(())
}
