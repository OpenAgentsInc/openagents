use psionic_eval::{
    tassadar_post_article_rebased_universality_verdict_split_report_path,
    write_tassadar_post_article_rebased_universality_verdict_split_report,
};

fn main() {
    let path = tassadar_post_article_rebased_universality_verdict_split_report_path();
    let report = write_tassadar_post_article_rebased_universality_verdict_split_report(&path)
        .expect("write post-article rebased universality verdict split report");
    println!(
        "wrote post-article rebased universality verdict split report to {} ({})",
        path.display(),
        report.report_digest,
    );
}
