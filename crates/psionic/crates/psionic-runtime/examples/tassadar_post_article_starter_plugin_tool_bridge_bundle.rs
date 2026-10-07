use psionic_runtime::{
    tassadar_post_article_starter_plugin_tool_bridge_bundle_path,
    write_starter_plugin_tool_bridge_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_starter_plugin_tool_bridge_bundle_path();
    let bundle = write_starter_plugin_tool_bridge_bundle(&output_path)?;
    println!("wrote {} ({})", output_path.display(), bundle.bundle_digest);
    println!(
        "projection_rows={} execution_cases={}",
        bundle.projection_rows.len(),
        bundle.execution_cases.len(),
    );
    Ok(())
}
