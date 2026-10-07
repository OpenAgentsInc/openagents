use psionic_eval::{tassadar_decode_scaling_report_path, write_tassadar_decode_scaling_report};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_decode_scaling_report(tassadar_decode_scaling_report_path())?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
