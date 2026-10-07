use psionic_research::{
    tassadar_shared_state_concurrency_summary_report_path,
    write_tassadar_shared_state_concurrency_summary_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_shared_state_concurrency_summary_report(
        tassadar_shared_state_concurrency_summary_report_path(),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
