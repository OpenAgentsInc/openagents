use psionic_eval::{
    tassadar_article_route_minimality_audit_report_path,
    write_tassadar_article_route_minimality_audit_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_path = tassadar_article_route_minimality_audit_report_path();
    let report = write_tassadar_article_route_minimality_audit_report(&report_path)?;
    println!(
        "wrote {} with route_minimality_audit_green={} and blocked_frontier={}",
        report_path.display(),
        report.route_minimality_audit_green,
        report
            .acceptance_gate_tie
            .blocked_issue_ids
            .first()
            .map(String::as_str)
            .unwrap_or("none"),
    );
    Ok(())
}
