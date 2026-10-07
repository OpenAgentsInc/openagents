use psionic_eval::{
    tassadar_article_interpreter_ownership_gate_report_path,
    write_tassadar_article_interpreter_ownership_gate_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_article_interpreter_ownership_gate_report_path();
    let report = write_tassadar_article_interpreter_ownership_gate_report(&output_path)?;
    println!(
        "wrote {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
