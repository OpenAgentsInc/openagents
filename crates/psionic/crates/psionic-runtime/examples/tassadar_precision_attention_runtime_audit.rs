use psionic_runtime::{
    tassadar_precision_attention_runtime_audit_report_path,
    write_tassadar_precision_attention_runtime_audit_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_precision_attention_runtime_audit_report_path();
    let report = write_tassadar_precision_attention_runtime_audit_report(&output_path)?;
    println!(
        "wrote Tassadar precision/attention runtime audit with {} receipts to {}",
        report.receipts.len(),
        output_path.display(),
    );
    Ok(())
}
