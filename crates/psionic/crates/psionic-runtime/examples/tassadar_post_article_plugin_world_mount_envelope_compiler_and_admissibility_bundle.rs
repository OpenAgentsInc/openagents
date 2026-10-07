use psionic_runtime::{
    tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle_path,
    write_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle_path();
    let bundle =
        write_tassadar_post_article_plugin_world_mount_envelope_compiler_and_admissibility_bundle(
            &output_path,
        )?;
    println!("wrote {} ({})", output_path.display(), bundle.bundle_digest);
    println!(
        "candidate_sets={} equivalent_choice_classes={} envelopes={} case_receipts={}",
        bundle.candidate_set_rows.len(),
        bundle.equivalent_choice_rows.len(),
        bundle.envelope_rows.len(),
        bundle.case_receipts.len(),
    );
    Ok(())
}
