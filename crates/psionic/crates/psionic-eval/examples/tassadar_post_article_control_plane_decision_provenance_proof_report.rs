use psionic_eval::{
    tassadar_post_article_control_plane_decision_provenance_proof_report_path,
    write_tassadar_post_article_control_plane_decision_provenance_proof_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_control_plane_decision_provenance_proof_report_path();
    let report =
        write_tassadar_post_article_control_plane_decision_provenance_proof_report(&output_path)?;
    println!(
        "wrote post-article control-plane decision-provenance proof report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
