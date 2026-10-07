use psionic_eval::{
    write_tassadar_article_transformer_training_evidence_bundle,
    TASSADAR_ARTICLE_TRANSFORMER_TRAINING_EVIDENCE_BUNDLE_REF,
};
use psionic_train::{
    build_tassadar_article_transformer_training_evidence_bundle,
    train_tassadar_article_transformer_toy_suite, TassadarArticleTransformerToyTaskSuite,
    TassadarArticleTransformerTrainingConfig,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let suite = TassadarArticleTransformerToyTaskSuite::reference();
    let config = TassadarArticleTransformerTrainingConfig::reference()?;
    let outcome = train_tassadar_article_transformer_toy_suite(&suite, &config)?;
    let bundle = build_tassadar_article_transformer_training_evidence_bundle(
        &suite,
        &config,
        &outcome,
    );
    let bundle = write_tassadar_article_transformer_training_evidence_bundle(
        TASSADAR_ARTICLE_TRANSFORMER_TRAINING_EVIDENCE_BUNDLE_REF,
        &bundle,
    )?;
    println!(
        "wrote {} with digest {}",
        TASSADAR_ARTICLE_TRANSFORMER_TRAINING_EVIDENCE_BUNDLE_REF,
        bundle.bundle_digest
    );
    Ok(())
}
