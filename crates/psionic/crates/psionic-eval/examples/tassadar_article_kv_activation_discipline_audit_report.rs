use psionic_eval::{
    tassadar_article_kv_activation_discipline_audit_report_path,
    write_tassadar_article_kv_activation_discipline_audit_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_article_kv_activation_discipline_audit_report_path();
    let report = write_tassadar_article_kv_activation_discipline_audit_report(&path)?;
    println!(
        "wrote {} with verdict={:?} and kv_activation_discipline_green={}",
        path.display(),
        report.dominance_verdict.verdict,
        report.kv_activation_discipline_green
    );
    Ok(())
}
