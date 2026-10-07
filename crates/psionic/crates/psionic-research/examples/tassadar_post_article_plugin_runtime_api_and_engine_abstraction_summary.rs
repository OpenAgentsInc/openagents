use psionic_research::{
    tassadar_post_article_plugin_runtime_api_and_engine_abstraction_summary_path,
    write_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_plugin_runtime_api_and_engine_abstraction_summary_path();
    let summary = write_tassadar_post_article_plugin_runtime_api_and_engine_abstraction_summary(
        &output_path,
    )?;
    println!(
        "wrote {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    println!(
        "contract_status={:?} runtime_api_rows={} deferred_issue_ids={}",
        summary.contract_status,
        summary.runtime_api_row_count,
        summary.deferred_issue_ids.len(),
    );
    Ok(())
}
