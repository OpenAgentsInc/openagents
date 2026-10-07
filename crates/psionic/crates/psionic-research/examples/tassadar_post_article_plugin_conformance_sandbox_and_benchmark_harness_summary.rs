use psionic_research::{
    tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_summary_path,
    write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_summary_path();
    let summary =
        write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_summary(
            &output_path,
        )?;
    println!(
        "wrote {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    println!(
        "contract_status={:?} conformance_rows={} benchmark_rows={} validation_rows={} deferred_issue_ids={}",
        summary.contract_status,
        summary.conformance_row_count,
        summary.benchmark_row_count,
        summary.validation_row_count,
        summary.deferred_issue_ids.len(),
    );
    Ok(())
}
