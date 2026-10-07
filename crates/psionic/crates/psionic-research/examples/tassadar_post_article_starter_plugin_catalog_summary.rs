use psionic_research::{
    tassadar_post_article_starter_plugin_catalog_summary_path,
    write_tassadar_post_article_starter_plugin_catalog_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_starter_plugin_catalog_summary_path();
    let summary = write_tassadar_post_article_starter_plugin_catalog_summary(&output_path)?;
    println!(
        "wrote post-article starter plugin catalog summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
