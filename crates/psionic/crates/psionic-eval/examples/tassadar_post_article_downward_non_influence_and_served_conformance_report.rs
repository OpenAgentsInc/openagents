use psionic_eval::{
    tassadar_post_article_downward_non_influence_and_served_conformance_report_path,
    write_tassadar_post_article_downward_non_influence_and_served_conformance_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path =
        tassadar_post_article_downward_non_influence_and_served_conformance_report_path();
    let report = write_tassadar_post_article_downward_non_influence_and_served_conformance_report(
        &output_path,
    )?;
    println!(
        "wrote post-article downward non-influence and served conformance report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
