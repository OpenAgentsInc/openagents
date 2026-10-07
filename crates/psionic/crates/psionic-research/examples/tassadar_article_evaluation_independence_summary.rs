use psionic_research::{
    tassadar_article_evaluation_independence_summary_path,
    write_tassadar_article_evaluation_independence_summary,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_article_evaluation_independence_summary_path();
    let report = write_tassadar_article_evaluation_independence_summary(&path)?;
    println!(
        "wrote {} with evaluation_case_count={} and evaluation_independence_green={}",
        path.display(),
        report.evaluation_case_count,
        report.evaluation_independence_green,
    );
    Ok(())
}
