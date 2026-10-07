use psionic_research::{
    tassadar_article_kv_activation_discipline_audit_summary_path,
    write_tassadar_article_kv_activation_discipline_audit_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_article_kv_activation_discipline_audit_summary_path();
    let summary = write_tassadar_article_kv_activation_discipline_audit_summary(&output_path)?;
    println!(
        "wrote {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
