use psionic_research::{
    tassadar_article_equivalence_final_audit_summary_path,
    write_tassadar_article_equivalence_final_audit_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let summary_path = tassadar_article_equivalence_final_audit_summary_path();
    let summary = write_tassadar_article_equivalence_final_audit_summary(&summary_path)?;
    println!(
        "wrote {} ({})",
        summary_path.display(),
        summary.summary_digest
    );
    println!(
        "matched_article_lines={} article_equivalence_green={}",
        summary.matched_article_line_count, summary.article_equivalence_green,
    );
    Ok(())
}
