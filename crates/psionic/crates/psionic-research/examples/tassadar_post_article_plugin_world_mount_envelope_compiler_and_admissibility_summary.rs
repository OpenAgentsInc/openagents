use psionic_research::{
    tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_summary_path,
    write_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_summary_path();
    let summary =
        write_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_summary(
            &output_path,
        )?;
    println!(
        "wrote {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    println!(
        "contract_status={:?} candidate_set_rows={} envelope_rows={} validation_rows={} deferred_issue_ids={}",
        summary.contract_status,
        summary.candidate_set_row_count,
        summary.envelope_row_count,
        summary.validation_row_count,
        summary.deferred_issue_ids.len(),
    );
    Ok(())
}
