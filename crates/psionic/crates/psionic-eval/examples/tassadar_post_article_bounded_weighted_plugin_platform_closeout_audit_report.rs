use psionic_eval::{
    tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report_path,
    write_tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report,
};

fn main() {
    let path = tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report_path();
    let report = write_tassadar_post_article_bounded_weighted_plugin_platform_closeout_audit_report(
        &path,
    )
    .expect("write post-article bounded weighted plugin-platform closeout audit report");
    println!(
        "wrote post-article bounded weighted plugin-platform closeout audit report to {} ({})",
        path.display(),
        report.report_digest
    );
}
