use psionic_eval::{
    tassadar_article_equivalence_blocker_matrix_report_path,
    write_tassadar_article_equivalence_blocker_matrix_report,
};

fn main() {
    let report = write_tassadar_article_equivalence_blocker_matrix_report(
        tassadar_article_equivalence_blocker_matrix_report_path(),
    )
    .expect("write article-equivalence blocker matrix report");
    println!(
        "wrote {} with blocker_count={} and article_equivalence_green={}",
        report.report_id, report.blocker_count, report.article_equivalence_green
    );
}
