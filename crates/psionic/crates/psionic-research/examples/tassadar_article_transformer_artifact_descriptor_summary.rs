use psionic_research::{
    tassadar_article_transformer_artifact_descriptor_summary_path,
    write_tassadar_article_transformer_artifact_descriptor_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_article_transformer_artifact_descriptor_summary(
        tassadar_article_transformer_artifact_descriptor_summary_path(),
    )?;
    println!("{}", report.report_digest);
    Ok(())
}
