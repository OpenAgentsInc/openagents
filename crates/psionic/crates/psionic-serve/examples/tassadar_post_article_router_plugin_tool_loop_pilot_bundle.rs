use psionic_serve::{
    tassadar_post_article_router_plugin_tool_loop_pilot_bundle_path,
    write_tassadar_post_article_router_plugin_tool_loop_pilot_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_router_plugin_tool_loop_pilot_bundle_path();
    let bundle = write_tassadar_post_article_router_plugin_tool_loop_pilot_bundle(&output_path)?;
    println!(
        "wrote {} with digest {}",
        output_path.display(),
        bundle.bundle_digest
    );
    Ok(())
}
