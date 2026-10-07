use psionic_eval::{
    tassadar_article_evaluation_independence_audit_report_path,
    write_tassadar_article_evaluation_independence_audit_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_article_evaluation_independence_audit_report_path();
    let report = write_tassadar_article_evaluation_independence_audit_report(&path)?;
    println!(
        "wrote {} with evaluation_case_count={} and evaluation_independence_green={}",
        path.display(),
        report.evaluation_case_rows.len(),
        report.evaluation_independence_green,
    );
    Ok(())
}
