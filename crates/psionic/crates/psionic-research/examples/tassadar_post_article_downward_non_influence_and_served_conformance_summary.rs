use psionic_research::{
    tassadar_post_article_downward_non_influence_and_served_conformance_summary_path,
    write_tassadar_post_article_downward_non_influence_and_served_conformance_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_downward_non_influence_and_served_conformance_summary_path();
    let summary = write_tassadar_post_article_downward_non_influence_and_served_conformance_summary(
        &output_path,
    )?;
    println!(
        "wrote post-article downward non-influence and served conformance summary to {} ({})",
        output_path.display(),
        summary.summary_digest
    );
    Ok(())
}
