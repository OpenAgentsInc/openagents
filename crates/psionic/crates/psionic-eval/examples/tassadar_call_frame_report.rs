use psionic_eval::{tassadar_call_frame_report_path, write_tassadar_call_frame_report};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_call_frame_report(tassadar_call_frame_report_path())?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
