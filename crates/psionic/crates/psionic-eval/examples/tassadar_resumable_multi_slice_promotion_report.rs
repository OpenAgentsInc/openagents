use psionic_eval::{
    tassadar_resumable_multi_slice_promotion_report_path,
    write_tassadar_resumable_multi_slice_promotion_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report_path = tassadar_resumable_multi_slice_promotion_report_path();
    let report = write_tassadar_resumable_multi_slice_promotion_report(&report_path)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
