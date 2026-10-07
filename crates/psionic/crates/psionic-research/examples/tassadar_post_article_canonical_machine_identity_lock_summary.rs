use psionic_research::{
    tassadar_post_article_canonical_machine_identity_lock_summary_path,
    write_tassadar_post_article_canonical_machine_identity_lock_summary,
};

fn main() {
    let path = tassadar_post_article_canonical_machine_identity_lock_summary_path();
    let summary = write_tassadar_post_article_canonical_machine_identity_lock_summary(&path)
        .expect("write post-article canonical machine identity lock summary");
    println!(
        "wrote post-article canonical machine identity lock summary to {} ({})",
        path.display(),
        summary.summary_digest
    );
}
