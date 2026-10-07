use psionic_research::{
    tassadar_trace_state_ablation_summary_path, write_tassadar_trace_state_ablation_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_trace_state_ablation_summary_path();
    let summary = write_tassadar_trace_state_ablation_summary(&output_path)?;
    println!(
        "wrote trace/state ablation summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
