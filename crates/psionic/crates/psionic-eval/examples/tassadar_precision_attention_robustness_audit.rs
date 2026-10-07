use psionic_eval::{
    tassadar_precision_attention_robustness_audit_report_path,
    write_tassadar_precision_attention_robustness_audit_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_precision_attention_robustness_audit_report_path();
    let report = write_tassadar_precision_attention_robustness_audit_report(&output_path)?;
    println!(
        "wrote Tassadar precision/attention robustness audit with {} regime summaries to {}",
        report.regime_summaries.len(),
        output_path.display(),
    );
    Ok(())
}
