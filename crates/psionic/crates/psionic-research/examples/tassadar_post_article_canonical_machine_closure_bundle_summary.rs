use psionic_research::{
    tassadar_post_article_canonical_machine_closure_bundle_summary_path,
    write_tassadar_post_article_canonical_machine_closure_bundle_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_canonical_machine_closure_bundle_summary_path();
    let summary =
        write_tassadar_post_article_canonical_machine_closure_bundle_summary(&output_path)?;
    println!(
        "wrote post-article canonical machine closure bundle summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
