use psionic_research::{
    tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary_path,
    write_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary_path();
    let summary =
        write_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_summary(
            &output_path,
        )?;
    println!(
        "wrote {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    println!(
        "dependency_rows={} receipt_identity_rows={} replay_class_rows={} failure_class_rows={} validation_rows={}",
        summary.dependency_row_count,
        summary.receipt_identity_row_count,
        summary.replay_class_row_count,
        summary.failure_class_row_count,
        summary.validation_row_count,
    );
    Ok(())
}
