use psionic_sandbox::{
    tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report_path,
    write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report_path();
    let report =
        write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_report(
            &output_path,
        )?;
    println!("wrote {} ({})", output_path.display(), report.report_digest);
    println!(
        "contract_status={:?} conformance_rows={} workflow_rows={} isolation_negative_rows={} benchmark_rows={} validation_rows={} deferred_issue_ids={}",
        report.contract_status,
        report.conformance_rows.len(),
        report.workflow_rows.len(),
        report.isolation_negative_rows.len(),
        report.benchmark_rows.len(),
        report.validation_rows.len(),
        report.deferred_issue_ids.len(),
    );
    Ok(())
}
