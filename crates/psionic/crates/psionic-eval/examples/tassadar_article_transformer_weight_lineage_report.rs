use psionic_eval::{
    write_tassadar_article_transformer_weight_lineage_contract,
    write_tassadar_article_transformer_weight_lineage_report,
    TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF,
    TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_REPORT_REF,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let contract = write_tassadar_article_transformer_weight_lineage_contract(
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF,
    )?;
    let report = write_tassadar_article_transformer_weight_lineage_report(
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_REPORT_REF,
    )?;
    println!(
        "wrote {} with digest {} and {} with digest {}",
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF,
        contract.contract_digest,
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_REPORT_REF,
        report.report_digest
    );
    Ok(())
}
