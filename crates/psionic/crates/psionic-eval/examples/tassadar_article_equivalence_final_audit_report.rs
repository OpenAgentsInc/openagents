use psionic_eval::{
    tassadar_article_equivalence_final_audit_report_path,
    write_tassadar_article_equivalence_final_audit_report,
};

fn main() {
    let report = write_tassadar_article_equivalence_final_audit_report(
        tassadar_article_equivalence_final_audit_report_path(),
    )
    .expect("write article-equivalence final audit report");
    println!(
        "wrote {} with matched_article_lines={} and article_equivalence_green={}",
        report.report_id, report.matched_article_line_count, report.article_equivalence_green
    );
}
