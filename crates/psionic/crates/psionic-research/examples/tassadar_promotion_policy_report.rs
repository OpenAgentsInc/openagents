use psionic_research::{
    tassadar_promotion_policy_report_path, write_tassadar_promotion_policy_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_promotion_policy_report(tassadar_promotion_policy_report_path())?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
