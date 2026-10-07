use psionic_eval::{
    write_tassadar_article_transformer_model_closure_report,
    TASSADAR_ARTICLE_TRANSFORMER_MODEL_CLOSURE_REPORT_REF,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report =
        write_tassadar_article_transformer_model_closure_report(
            TASSADAR_ARTICLE_TRANSFORMER_MODEL_CLOSURE_REPORT_REF,
        )?;
    println!(
        "wrote {} with digest {}",
        TASSADAR_ARTICLE_TRANSFORMER_MODEL_CLOSURE_REPORT_REF,
        report.report_digest
    );
    Ok(())
}
