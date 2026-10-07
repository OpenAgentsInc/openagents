use psionic_eval::{
    tassadar_threads_research_profile_report_path, write_tassadar_threads_research_profile_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_threads_research_profile_report_path();
    let report = write_tassadar_threads_research_profile_report(&path)?;
    println!(
        "wrote threads research profile report to {} ({})",
        path.display(),
        report.report_id
    );
    Ok(())
}
