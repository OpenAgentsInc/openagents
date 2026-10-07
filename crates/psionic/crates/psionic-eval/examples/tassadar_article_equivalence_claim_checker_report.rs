use psionic_eval::{
    tassadar_article_equivalence_claim_checker_report_path,
    write_tassadar_article_equivalence_claim_checker_report,
};

fn main() {
    let report = write_tassadar_article_equivalence_claim_checker_report(
        tassadar_article_equivalence_claim_checker_report_path(),
    )
    .expect("write article-equivalence claim checker report");
    println!(
        "wrote {} with green_prerequisites={}/{} and article_equivalence_green={}",
        report.report_id,
        report.green_prerequisite_ids.len(),
        report.prerequisite_rows.len(),
        report.article_equivalence_green
    );
}
