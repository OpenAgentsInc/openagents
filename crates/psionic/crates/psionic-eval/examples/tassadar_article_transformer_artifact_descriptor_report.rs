use psionic_eval::{
    tassadar_article_transformer_artifact_descriptor_report_path,
    write_tassadar_article_transformer_artifact_descriptor_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_article_transformer_artifact_descriptor_report(
        tassadar_article_transformer_artifact_descriptor_report_path(),
    )?;
    println!("{}", report.report_digest);
    Ok(())
}
