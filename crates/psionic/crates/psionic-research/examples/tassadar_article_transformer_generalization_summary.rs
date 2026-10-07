use psionic_research::{
    tassadar_article_transformer_generalization_summary_path,
    write_tassadar_article_transformer_generalization_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_article_transformer_generalization_summary_path();
    let report = write_tassadar_article_transformer_generalization_summary(&path)?;
    println!(
        "wrote {} with case_count={} and generalization_green={}",
        path.display(),
        report.case_count,
        report.generalization_green,
    );
    Ok(())
}
