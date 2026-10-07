use psionic_eval::{
    tassadar_exactness_refusal_artifact_report_path,
    write_tassadar_exactness_refusal_artifact_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_exactness_refusal_artifact_report(
        tassadar_exactness_refusal_artifact_report_path(),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
