use psionic_research::{
    tassadar_post_article_anti_drift_stability_closeout_summary_path,
    write_tassadar_post_article_anti_drift_stability_closeout_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_anti_drift_stability_closeout_summary_path();
    let summary = write_tassadar_post_article_anti_drift_stability_closeout_summary(&output_path)?;
    println!(
        "wrote post-article anti-drift stability closeout summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
