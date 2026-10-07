use psionic_runtime::{
    tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle_path,
    write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle_path();
    let bundle =
        write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_bundle(
            &output_path,
        )?;
    println!("wrote {} ({})", output_path.display(), bundle.bundle_digest);
    println!(
        "controller_case_rows={} control_trace_rows={} host_negative_rows={}",
        bundle.controller_case_rows.len(),
        bundle.control_trace_rows.len(),
        bundle.host_negative_rows.len(),
    );
    Ok(())
}
