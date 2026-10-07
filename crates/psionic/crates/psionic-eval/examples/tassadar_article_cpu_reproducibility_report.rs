use psionic_eval::{
    tassadar_article_cpu_reproducibility_report_path,
    write_tassadar_article_cpu_reproducibility_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_path = tassadar_article_cpu_reproducibility_report_path();
    let report = write_tassadar_article_cpu_reproducibility_report(&report_path)?;
    println!("wrote {} ({})", report_path.display(), report.report_digest);
    Ok(())
}
