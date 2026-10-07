use psionic_research::{
    tassadar_post_article_universal_machine_proof_rebinding_summary_path,
    write_tassadar_post_article_universal_machine_proof_rebinding_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_universal_machine_proof_rebinding_summary_path();
    let summary =
        write_tassadar_post_article_universal_machine_proof_rebinding_summary(&output_path)?;
    println!(
        "wrote post-article universal-machine proof rebinding summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
