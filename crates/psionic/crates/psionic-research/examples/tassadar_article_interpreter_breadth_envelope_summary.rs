use psionic_research::{
    tassadar_article_interpreter_breadth_envelope_summary_path,
    write_tassadar_article_interpreter_breadth_envelope_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let summary = write_tassadar_article_interpreter_breadth_envelope_summary(
        tassadar_article_interpreter_breadth_envelope_summary_path(),
    )?;
    println!(
        "wrote {} ({})",
        tassadar_article_interpreter_breadth_envelope_summary_path().display(),
        summary.report_digest
    );
    Ok(())
}
