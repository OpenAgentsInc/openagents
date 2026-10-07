use psionic_catalog::{
    tassadar_post_article_starter_plugin_catalog_report_path,
    write_tassadar_post_article_starter_plugin_catalog_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_starter_plugin_catalog_report_path();
    let report = write_tassadar_post_article_starter_plugin_catalog_report(&output_path)?;
    println!("wrote {} to {}", report.report_id, output_path.display());
    Ok(())
}
