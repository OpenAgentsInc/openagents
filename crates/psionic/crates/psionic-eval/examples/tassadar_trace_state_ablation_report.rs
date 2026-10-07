use psionic_eval::{
    tassadar_trace_state_ablation_report_path, write_tassadar_trace_state_ablation_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_trace_state_ablation_report_path();
    let report = write_tassadar_trace_state_ablation_report(&output_path)?;
    println!(
        "wrote trace/state ablation report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
