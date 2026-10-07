use psionic_research::{
    tassadar_article_transformer_reference_linear_exactness_summary_path,
    write_tassadar_article_transformer_reference_linear_exactness_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_article_transformer_reference_linear_exactness_summary_path();
    let summary = write_tassadar_article_transformer_reference_linear_exactness_summary(&path)?;
    println!(
        "wrote {} with exact_case_count={} and reference_linear_exactness_green={}",
        path.display(),
        summary.exact_case_count,
        summary.reference_linear_exactness_green
    );
    Ok(())
}
