use psionic_eval::{
    tassadar_post_article_starter_plugin_catalog_eval_report_path,
    write_tassadar_post_article_starter_plugin_catalog_eval_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_starter_plugin_catalog_eval_report_path();
    let report = write_tassadar_post_article_starter_plugin_catalog_eval_report(&output_path)?;
    println!(
        "wrote post-article starter plugin catalog eval report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
