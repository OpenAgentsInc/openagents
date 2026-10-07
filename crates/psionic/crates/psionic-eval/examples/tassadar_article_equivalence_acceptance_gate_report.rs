use psionic_eval::{
    tassadar_article_equivalence_acceptance_gate_report_path,
    write_tassadar_article_equivalence_acceptance_gate_report,
};

fn main() {
    let report = write_tassadar_article_equivalence_acceptance_gate_report(
        tassadar_article_equivalence_acceptance_gate_report_path(),
    )
    .expect("write article-equivalence acceptance gate report");
    println!(
        "wrote {} with acceptance_status={:?} and blocked_issues={}",
        report.report_id,
        report.acceptance_status,
        report.blocked_issue_ids.len()
    );
}
