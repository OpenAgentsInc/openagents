use psionic_research::{
    tassadar_post_article_execution_semantics_proof_transport_audit_summary_path,
    write_tassadar_post_article_execution_semantics_proof_transport_audit_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_execution_semantics_proof_transport_audit_summary_path();
    let summary = write_tassadar_post_article_execution_semantics_proof_transport_audit_summary(
        &output_path,
    )?;
    println!(
        "wrote post-article execution-semantics proof-transport audit summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
