use psionic_research::{
    tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary_path,
    write_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary_path();
    let summary =
        write_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_summary(
            &output_path,
        )?;
    println!(
        "wrote {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    println!(
        "contract_status={:?} binding_rows={} composition_rows={} validation_rows={} deferred_issue_ids={}",
        summary.contract_status,
        summary.binding_row_count,
        summary.composition_row_count,
        summary.validation_row_count,
        summary.deferred_issue_ids.len(),
    );
    Ok(())
}
