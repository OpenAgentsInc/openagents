use psionic_runtime::{
    tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle_path,
    write_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle_path();
    let bundle =
        write_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle(
            &output_path,
        )?;
    println!("wrote {} ({})", output_path.display(), bundle.bundle_digest);
    println!(
        "binding_rows={} evidence_boundary_rows={} composition_rows={} negative_rows={}",
        bundle.binding_rows.len(),
        bundle.evidence_boundary_rows.len(),
        bundle.composition_rows.len(),
        bundle.negative_rows.len(),
    );
    Ok(())
}
