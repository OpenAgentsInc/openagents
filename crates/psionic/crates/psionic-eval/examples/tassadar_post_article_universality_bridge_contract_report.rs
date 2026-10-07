use psionic_eval::{
    tassadar_post_article_universality_bridge_contract_report_path,
    write_tassadar_post_article_universality_bridge_contract_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_post_article_universality_bridge_contract_report_path();
    let report = write_tassadar_post_article_universality_bridge_contract_report(&output_path)?;
    println!(
        "wrote post-article universality bridge contract report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
