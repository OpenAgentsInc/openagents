use psionic_eval::{
    write_tassadar_article_fixture_transformer_parity_report,
    TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_article_fixture_transformer_parity_report(
        TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF,
    )?;
    println!(
        "wrote {} with digest {}",
        TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF, report.report_digest
    );
    Ok(())
}
