use psionic_eval::{
    tassadar_conditional_masking_report_path, write_tassadar_conditional_masking_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report =
        write_tassadar_conditional_masking_report(tassadar_conditional_masking_report_path())?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
