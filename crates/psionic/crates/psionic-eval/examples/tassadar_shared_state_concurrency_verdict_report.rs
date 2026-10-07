use psionic_eval::{
    tassadar_shared_state_concurrency_verdict_report_path,
    write_tassadar_shared_state_concurrency_verdict_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_shared_state_concurrency_verdict_report(
        tassadar_shared_state_concurrency_verdict_report_path(),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
