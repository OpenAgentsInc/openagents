use psionic_research::{
    tassadar_article_cpu_reproducibility_summary_report_path,
    write_tassadar_article_cpu_reproducibility_summary_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_path = tassadar_article_cpu_reproducibility_summary_report_path();
    let report = write_tassadar_article_cpu_reproducibility_summary_report(&report_path)?;
    println!("wrote {} ({})", report_path.display(), report.report_digest);
    Ok(())
}
