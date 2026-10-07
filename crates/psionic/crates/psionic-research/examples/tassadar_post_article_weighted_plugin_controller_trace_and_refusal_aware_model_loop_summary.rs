use psionic_research::{
    tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary_path,
    write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary_path();
    let summary =
        write_tassadar_post_article_weighted_plugin_controller_trace_and_refusal_aware_model_loop_summary(
            &output_path,
        )?;
    println!(
        "wrote {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    println!(
        "contract_status={:?} controller_case_rows={} control_trace_rows={} validation_rows={} weighted_plugin_control_allowed={} deferred_issue_ids={}",
        summary.contract_status,
        summary.controller_case_row_count,
        summary.control_trace_row_count,
        summary.validation_row_count,
        summary.weighted_plugin_control_allowed,
        summary.deferred_issue_ids.len(),
    );
    Ok(())
}
