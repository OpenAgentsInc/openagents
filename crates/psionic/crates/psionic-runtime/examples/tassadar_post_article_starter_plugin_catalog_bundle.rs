use psionic_runtime::{
    tassadar_post_article_starter_plugin_catalog_bundle_path,
    write_tassadar_post_article_starter_plugin_catalog_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_starter_plugin_catalog_bundle_path();
    let bundle = write_tassadar_post_article_starter_plugin_catalog_bundle(&output_path)?;
    println!("wrote {} ({})", output_path.display(), bundle.bundle_digest);
    println!(
        "plugin_count={} local_deterministic_plugin_count={} read_only_network_plugin_count={} bounded_flow_count={}",
        bundle.plugin_count,
        bundle.local_deterministic_plugin_count,
        bundle.read_only_network_plugin_count,
        bundle.bounded_flow_count,
    );
    Ok(())
}
