use psionic_research::{
    write_tassadar_article_transformer_weight_production_summary,
    TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_SUMMARY_REPORT_REF,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let summary = write_tassadar_article_transformer_weight_production_summary(
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_SUMMARY_REPORT_REF,
    )?;
    println!(
        "wrote {} with digest {}",
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_SUMMARY_REPORT_REF, summary.report_digest
    );
    Ok(())
}
