use psionic_eval::{
    tassadar_article_runtime_closeout_report_path, write_tassadar_article_runtime_closeout_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_path = tassadar_article_runtime_closeout_report_path();
    let report = write_tassadar_article_runtime_closeout_report(&report_path)?;
    println!("wrote {} ({})", report_path.display(), report.report_digest);
    Ok(())
}
