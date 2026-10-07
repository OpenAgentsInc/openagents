use psionic_research::{
    tassadar_article_interpreter_ownership_gate_summary_path,
    write_tassadar_article_interpreter_ownership_gate_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_article_interpreter_ownership_gate_summary_path();
    let summary = write_tassadar_article_interpreter_ownership_gate_summary(&output_path)?;
    println!(
        "wrote {} ({})",
        output_path.display(),
        summary.report_digest
    );
    Ok(())
}
