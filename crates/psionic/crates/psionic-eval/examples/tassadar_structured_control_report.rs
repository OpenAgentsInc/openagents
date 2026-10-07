use psionic_eval::{
    tassadar_structured_control_report_path, write_tassadar_structured_control_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report =
        write_tassadar_structured_control_report(tassadar_structured_control_report_path())?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
