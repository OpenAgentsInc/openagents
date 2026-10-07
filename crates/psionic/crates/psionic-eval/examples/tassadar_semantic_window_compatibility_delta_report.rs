use psionic_eval::{
    tassadar_semantic_window_compatibility_delta_report_path,
    write_tassadar_semantic_window_compatibility_delta_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_semantic_window_compatibility_delta_report(
        tassadar_semantic_window_compatibility_delta_report_path(),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
