use psionic_eval::{
    tassadar_execution_checkpoint_report_path, write_tassadar_execution_checkpoint_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_path = tassadar_execution_checkpoint_report_path();
    let report = write_tassadar_execution_checkpoint_report(&report_path)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
