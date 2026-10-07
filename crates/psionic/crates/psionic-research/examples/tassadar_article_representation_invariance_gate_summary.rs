use psionic_research::{
    tassadar_article_representation_invariance_gate_summary_path,
    write_tassadar_article_representation_invariance_gate_summary,
};

fn main() {
    let summary = write_tassadar_article_representation_invariance_gate_summary(
        tassadar_article_representation_invariance_gate_summary_path(),
    )
    .expect("write article representation invariance summary");
    println!(
        "wrote {} with case_count={} and article_equivalence_green={}",
        summary.report_id, summary.case_count, summary.article_equivalence_green
    );
}
