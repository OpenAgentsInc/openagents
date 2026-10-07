use psionic_research::{
    tassadar_article_route_minimality_audit_summary_path,
    write_tassadar_article_route_minimality_audit_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let summary_path = tassadar_article_route_minimality_audit_summary_path();
    let summary = write_tassadar_article_route_minimality_audit_summary(&summary_path)?;
    println!(
        "wrote {} ({})",
        summary_path.display(),
        summary.summary_digest
    );
    println!(
        "blocked_frontier={} route_minimality_audit_green={}",
        summary.blocked_issue_frontier, summary.route_minimality_audit_green,
    );
    Ok(())
}
