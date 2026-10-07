use psionic_eval::{
    write_tassadar_article_transformer_weight_production_evidence_bundle,
    TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_EVIDENCE_BUNDLE_REF,
};
use psionic_train::{
    build_tassadar_article_transformer_weight_production_evidence_bundle,
    run_tassadar_article_transformer_weight_production,
    TassadarArticleTransformerWeightProductionConfig,
    TassadarArticleTransformerWeightProductionSuite,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let suite = TassadarArticleTransformerWeightProductionSuite::reference()?;
    let config = TassadarArticleTransformerWeightProductionConfig::reference()?;
    let outcome = run_tassadar_article_transformer_weight_production(&suite, &config)?;
    let bundle = build_tassadar_article_transformer_weight_production_evidence_bundle(
        &suite, &config, &outcome,
    );
    let bundle = write_tassadar_article_transformer_weight_production_evidence_bundle(
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_EVIDENCE_BUNDLE_REF,
        &bundle,
    )?;
    println!(
        "wrote {} with digest {}",
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_PRODUCTION_EVIDENCE_BUNDLE_REF, bundle.bundle_digest
    );
    Ok(())
}
