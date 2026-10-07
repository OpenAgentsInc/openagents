use psionic_eval::{
    tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report_path,
    write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report_path();
    let report =
        write_tassadar_post_article_plugin_conformance_sandbox_and_benchmark_harness_eval_report(
            &output_path,
        )?;
    println!("wrote {} ({})", output_path.display(), report.report_digest);
    println!(
        "dependency_rows={} conformance_rows={} workflow_rows={} isolation_negative_rows={} benchmark_rows={} validation_rows={}",
        report.dependency_rows.len(),
        report.conformance_rows.len(),
        report.workflow_rows.len(),
        report.isolation_negative_rows.len(),
        report.benchmark_rows.len(),
        report.validation_rows.len(),
    );
    Ok(())
}
