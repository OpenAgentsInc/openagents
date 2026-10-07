use psionic_research::{
    tassadar_article_equivalence_blocker_matrix_summary_path,
    write_tassadar_article_equivalence_blocker_matrix_summary,
};

fn main() {
    let report = write_tassadar_article_equivalence_blocker_matrix_summary(
        tassadar_article_equivalence_blocker_matrix_summary_path(),
    )
    .expect("write article-equivalence blocker matrix summary");
    println!(
        "wrote {} with open_blocker_count={} and article_equivalence_green={}",
        report.report_id, report.open_blocker_count, report.article_equivalence_green
    );
}
