use psionic_research::{
    tassadar_post_article_canonical_route_semantic_preservation_summary_path,
    write_tassadar_post_article_canonical_route_semantic_preservation_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_canonical_route_semantic_preservation_summary_path();
    let summary =
        write_tassadar_post_article_canonical_route_semantic_preservation_summary(&output_path)?;
    println!(
        "wrote post-article canonical-route semantic-preservation summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
