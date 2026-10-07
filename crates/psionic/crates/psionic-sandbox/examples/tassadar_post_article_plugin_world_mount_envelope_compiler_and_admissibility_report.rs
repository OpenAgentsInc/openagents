use psionic_sandbox::{
    tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report_path,
    write_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report_path();
    let report =
        write_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_report(
            &output_path,
        )?;
    println!("wrote {} ({})", output_path.display(), report.report_digest);
    println!(
        "contract_status={:?} candidate_set_rows={} envelope_rows={} validation_rows={} deferred_issue_ids={}",
        report.contract_status,
        report.candidate_set_rows.len(),
        report.envelope_rows.len(),
        report.validation_rows.len(),
        report.deferred_issue_ids.len(),
    );
    Ok(())
}
