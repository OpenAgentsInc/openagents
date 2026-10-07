use psionic_catalog::{
    tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report_path,
    write_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report_path();
    let report =
        write_tassadar_post_article_plugin_authority_promotion_publication_and_trust_tier_gate_report(
            &output_path,
        )?;
    println!("wrote {} to {}", report.report_id, output_path.display());
    Ok(())
}
