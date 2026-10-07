use psionic_runtime::{
    tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle_path,
    write_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle_path();
    let bundle = write_tassadar_post_article_plugin_invocation_receipts_and_replay_classes_bundle(
        &output_path,
    )?;
    println!("wrote {} ({})", output_path.display(), bundle.bundle_digest);
    println!(
        "receipt_fields={} replay_classes={} failure_classes={} challenge_bound_cases={}",
        bundle.receipt_field_rows.len(),
        bundle.replay_class_rows.len(),
        bundle.failure_class_rows.len(),
        bundle.challenge_bound_case_count,
    );
    Ok(())
}
