use psionic_research::{
    tassadar_post_article_bounded_weighted_plugin_platform_closeout_summary_path,
    write_tassadar_post_article_bounded_weighted_plugin_platform_closeout_summary,
};

fn main() {
    let path = tassadar_post_article_bounded_weighted_plugin_platform_closeout_summary_path();
    let summary = write_tassadar_post_article_bounded_weighted_plugin_platform_closeout_summary(
        &path,
    )
    .expect("write post-article bounded weighted plugin-platform closeout summary");
    println!(
        "wrote post-article bounded weighted plugin-platform closeout summary to {} ({})",
        path.display(),
        summary.summary_digest
    );
}
