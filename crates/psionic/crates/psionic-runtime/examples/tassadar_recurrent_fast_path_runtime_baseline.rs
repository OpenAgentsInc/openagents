use psionic_runtime::{
    tassadar_recurrent_fast_path_runtime_baseline_report_path,
    write_tassadar_recurrent_fast_path_runtime_baseline_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_recurrent_fast_path_runtime_baseline_report(
        tassadar_recurrent_fast_path_runtime_baseline_report_path(),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
