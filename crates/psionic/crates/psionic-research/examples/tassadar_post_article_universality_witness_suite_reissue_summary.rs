use psionic_research::{
    tassadar_post_article_universality_witness_suite_reissue_summary_path,
    write_tassadar_post_article_universality_witness_suite_reissue_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_universality_witness_suite_reissue_summary_path();
    let summary =
        write_tassadar_post_article_universality_witness_suite_reissue_summary(&output_path)?;
    println!(
        "wrote post-article universality witness-suite reissue summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
