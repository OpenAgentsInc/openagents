use psionic_eval::{
    tassadar_article_transformer_reference_linear_exactness_gate_report_path,
    write_tassadar_article_transformer_reference_linear_exactness_gate_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_article_transformer_reference_linear_exactness_gate_report_path();
    let report = write_tassadar_article_transformer_reference_linear_exactness_gate_report(&path)?;
    println!(
        "wrote {} with exact_case_count={} and reference_linear_exactness_green={}",
        path.display(),
        report.exact_case_count,
        report.reference_linear_exactness_green
    );
    Ok(())
}
