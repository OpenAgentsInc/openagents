use psionic_research::{
    tassadar_post_article_turing_completeness_closeout_summary_path,
    write_tassadar_post_article_turing_completeness_closeout_summary,
};

fn main() {
    let path = tassadar_post_article_turing_completeness_closeout_summary_path();
    let summary = write_tassadar_post_article_turing_completeness_closeout_summary(&path)
        .expect("write post-article turing-completeness closeout summary");
    println!(
        "wrote post-article turing-completeness closeout summary to {} ({})",
        path.display(),
        summary.summary_digest,
    );
}
