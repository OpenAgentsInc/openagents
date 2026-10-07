use psionic_runtime::{
    tassadar_post_article_plugin_text_stats_runtime_bundle_path, write_text_stats_runtime_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_plugin_text_stats_runtime_bundle_path();
    let bundle = write_text_stats_runtime_bundle(&output_path)?;
    println!("wrote {} ({})", output_path.display(), bundle.bundle_digest);
    println!(
        "plugin_id={} case_count={}",
        bundle.plugin_id,
        bundle.case_rows.len(),
    );
    Ok(())
}
