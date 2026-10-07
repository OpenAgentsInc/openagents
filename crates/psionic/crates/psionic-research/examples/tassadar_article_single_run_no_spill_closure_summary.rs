use psionic_research::{
    tassadar_article_single_run_no_spill_closure_summary_path,
    write_tassadar_article_single_run_no_spill_closure_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_article_single_run_no_spill_closure_summary_path();
    let summary = write_tassadar_article_single_run_no_spill_closure_summary(&output_path)?;
    println!(
        "wrote {} ({})",
        output_path.display(),
        summary.report_digest
    );
    Ok(())
}
