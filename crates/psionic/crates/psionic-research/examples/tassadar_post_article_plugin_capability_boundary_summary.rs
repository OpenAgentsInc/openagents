use psionic_research::{
    tassadar_post_article_plugin_capability_boundary_summary_path,
    write_tassadar_post_article_plugin_capability_boundary_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_plugin_capability_boundary_summary_path();
    let summary = write_tassadar_post_article_plugin_capability_boundary_summary(&output_path)?;
    println!(
        "wrote post-article plugin-capability boundary summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
