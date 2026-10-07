use psionic_research::{
    tassadar_article_cross_machine_reproducibility_matrix_summary_path,
    write_tassadar_article_cross_machine_reproducibility_matrix_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let summary_path = tassadar_article_cross_machine_reproducibility_matrix_summary_path();
    let summary = write_tassadar_article_cross_machine_reproducibility_matrix_summary(
        &summary_path,
    )?;
    println!(
        "wrote {} ({})",
        summary_path.display(),
        summary.summary_digest
    );
    println!(
        "blocked_frontier={} reproducibility_matrix_green={}",
        summary.blocked_issue_frontier,
        summary.reproducibility_matrix_green,
    );
    Ok(())
}
