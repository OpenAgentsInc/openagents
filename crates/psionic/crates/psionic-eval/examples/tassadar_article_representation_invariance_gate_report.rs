use psionic_eval::{
    tassadar_article_representation_invariance_gate_report_path,
    write_tassadar_article_representation_invariance_gate_report,
};

fn main() {
    let report = write_tassadar_article_representation_invariance_gate_report(
        tassadar_article_representation_invariance_gate_report_path(),
    )
    .expect("write article representation invariance gate report");
    println!(
        "wrote {} with case_count={} and article_equivalence_green={}",
        report.report_id,
        report.representation_equivalence_review.case_count,
        report.article_equivalence_green
    );
}
