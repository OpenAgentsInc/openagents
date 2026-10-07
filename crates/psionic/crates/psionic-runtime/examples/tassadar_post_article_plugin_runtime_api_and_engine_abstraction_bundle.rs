use psionic_runtime::{
    tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle_path,
    write_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle_path();
    let bundle =
        write_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_bundle(&output_path)?;
    println!("wrote {} ({})", output_path.display(), bundle.bundle_digest);
    println!(
        "success_cases={} refusal_cases={} failure_cases={} cancellation_cases={}",
        bundle.exact_success_case_count,
        bundle.exact_typed_refusal_case_count,
        bundle.exact_runtime_failure_case_count,
        bundle.exact_cancellation_case_count,
    );
    Ok(())
}
