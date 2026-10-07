use psionic_research::{
    tassadar_post_article_rebased_universality_verdict_split_summary_path,
    write_tassadar_post_article_rebased_universality_verdict_split_summary,
};

fn main() {
    let path = tassadar_post_article_rebased_universality_verdict_split_summary_path();
    let summary = write_tassadar_post_article_rebased_universality_verdict_split_summary(&path)
        .expect("write post-article rebased universality verdict split summary");
    println!(
        "wrote post-article rebased universality verdict split summary to {} ({})",
        path.display(),
        summary.summary_digest,
    );
}
