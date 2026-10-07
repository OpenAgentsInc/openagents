use psionic_runtime::{
    tassadar_post_article_starter_plugin_workflow_controller_bundle_path,
    write_starter_plugin_workflow_controller_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_starter_plugin_workflow_controller_bundle_path();
    let bundle = write_starter_plugin_workflow_controller_bundle(&output_path)?;
    println!("wrote {} ({})", output_path.display(), bundle.bundle_digest);
    println!(
        "workflow_graph_id={} case_count={}",
        bundle.workflow_graph_id,
        bundle.case_rows.len(),
    );
    Ok(())
}
