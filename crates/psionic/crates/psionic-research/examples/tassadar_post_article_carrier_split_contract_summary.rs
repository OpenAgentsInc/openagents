use psionic_research::{
    tassadar_post_article_carrier_split_contract_summary_path,
    write_tassadar_post_article_carrier_split_contract_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_carrier_split_contract_summary_path();
    let summary = write_tassadar_post_article_carrier_split_contract_summary(&output_path)?;
    println!(
        "wrote post-article carrier split contract summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
