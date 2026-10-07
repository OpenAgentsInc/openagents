use psionic_eval::{
    tassadar_post_article_universal_machine_proof_rebinding_report_path,
    write_tassadar_post_article_universal_machine_proof_rebinding_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_universal_machine_proof_rebinding_report_path();
    let report =
        write_tassadar_post_article_universal_machine_proof_rebinding_report(&output_path)?;
    println!(
        "wrote post-article universal-machine proof rebinding report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
