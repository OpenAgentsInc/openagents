use psionic_eval::{
    tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report_path,
    write_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report_path();
    let report =
        write_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_report(
            &output_path,
        )?;
    println!(
        "wrote post-article equivalent-choice neutrality and admissibility contract report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
