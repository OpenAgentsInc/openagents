use psionic_research::{
    tassadar_approximate_attention_closure_summary_report_path,
    write_tassadar_approximate_attention_closure_summary_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_approximate_attention_closure_summary_report_path();
    let report = write_tassadar_approximate_attention_closure_summary_report(&output_path)?;
    println!(
        "wrote approximate attention closure summary to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
