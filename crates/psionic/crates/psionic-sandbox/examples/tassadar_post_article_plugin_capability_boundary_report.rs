use psionic_sandbox::{
    tassadar_post_article_plugin_capability_boundary_report_path,
    write_tassadar_post_article_plugin_capability_boundary_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_plugin_capability_boundary_report_path();
    let report = write_tassadar_post_article_plugin_capability_boundary_report(&output_path)?;
    println!(
        "wrote post-article plugin-capability boundary report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
