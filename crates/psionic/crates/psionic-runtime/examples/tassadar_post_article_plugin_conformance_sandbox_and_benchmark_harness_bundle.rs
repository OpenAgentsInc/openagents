use psionic_runtime::{
    tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle_path,
    write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle_path();
    let bundle =
        write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_bundle(
            &output_path,
        )?;
    println!("wrote {} ({})", output_path.display(), bundle.bundle_digest);
    println!(
        "conformance_rows={} workflow_rows={} isolation_negative_rows={} benchmark_rows={} trace_receipts={}",
        bundle.conformance_rows.len(),
        bundle.workflow_rows.len(),
        bundle.isolation_negative_rows.len(),
        bundle.benchmark_rows.len(),
        bundle.trace_receipts.len(),
    );
    Ok(())
}
