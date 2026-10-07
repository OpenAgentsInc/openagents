use psionic_eval::{
    tassadar_post_article_anti_drift_stability_closeout_audit_report_path,
    write_tassadar_post_article_anti_drift_stability_closeout_audit_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_anti_drift_stability_closeout_audit_report_path();
    let report =
        write_tassadar_post_article_anti_drift_stability_closeout_audit_report(&output_path)?;
    println!(
        "wrote post-article anti-drift stability closeout audit report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
