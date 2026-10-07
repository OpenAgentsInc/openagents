use psionic_eval::{
    tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report_path,
    write_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report_path();
    let report =
        write_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract_report(
            &output_path,
        )?;
    println!(
        "wrote post-article fast-route legitimacy and carrier-binding contract report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
