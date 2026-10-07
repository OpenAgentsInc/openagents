use psionic_eval::{
    tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report_path,
    write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report_path();
    let report =
        write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_eval_report(
            &output_path,
        )?;
    println!("wrote {} ({})", output_path.display(), report.report_digest);
    println!(
        "contract_status={:?} controller_case_rows={} control_trace_rows={} host_negative_rows={} validation_rows={} deferred_issue_ids={}",
        report.contract_status,
        report.controller_case_rows.len(),
        report.control_trace_rows.len(),
        report.host_negative_rows.len(),
        report.validation_rows.len(),
        report.deferred_issue_ids.len(),
    );
    Ok(())
}
