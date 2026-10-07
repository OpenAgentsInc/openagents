use psionic_research::{
    tassadar_post_article_control_plane_decision_provenance_proof_summary_path,
    write_tassadar_post_article_control_plane_decision_provenance_proof_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_control_plane_decision_provenance_proof_summary_path();
    let summary =
        write_tassadar_post_article_control_plane_decision_provenance_proof_summary(&output_path)?;
    println!(
        "wrote post-article control-plane decision-provenance proof summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
