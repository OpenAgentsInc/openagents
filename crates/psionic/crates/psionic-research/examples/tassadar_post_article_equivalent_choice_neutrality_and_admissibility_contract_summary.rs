use psionic_research::{
    tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary_path,
    write_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary_path();
    let summary =
        write_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract_summary(
            &output_path,
        )?;
    println!(
        "wrote post-article equivalent-choice neutrality and admissibility contract summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
