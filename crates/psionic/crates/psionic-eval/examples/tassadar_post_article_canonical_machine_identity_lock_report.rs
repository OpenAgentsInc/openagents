use psionic_eval::{
    tassadar_post_article_canonical_machine_identity_lock_report_path,
    write_tassadar_post_article_canonical_machine_identity_lock_report,
};

fn main() {
    let path = tassadar_post_article_canonical_machine_identity_lock_report_path();
    let report = write_tassadar_post_article_canonical_machine_identity_lock_report(&path)
        .expect("write post-article canonical machine identity lock report");
    println!(
        "wrote post-article canonical machine identity lock report to {} ({})",
        path.display(),
        report.report_digest
    );
}
