use psionic_research::{
    tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary_path,
    write_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary_path();
    let summary =
        write_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_summary(
            &output_path,
        )?;
    println!(
        "wrote post-article fast-route legitimacy and carrier-binding contract summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
