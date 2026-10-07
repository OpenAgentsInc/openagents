use psionic_eval::{
    tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report_path,
    write_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report_path();
    let report = write_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_report(
        &output_path,
    )?;
    println!("wrote {} ({})", output_path.display(), report.report_digest);
    println!(
        "dependency_rows={} receipt_identity_rows={} replay_class_rows={} failure_class_rows={} validation_rows={}",
        report.dependency_rows.len(),
        report.receipt_identity_rows.len(),
        report.replay_class_rows.len(),
        report.failure_class_rows.len(),
        report.validation_rows.len(),
    );
    Ok(())
}
