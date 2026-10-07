use psionic_eval::{
    tassadar_negative_invocation_report_path, write_tassadar_negative_invocation_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_negative_invocation_report_path();
    let report = write_tassadar_negative_invocation_report(&path)?;
    println!("wrote {} to {}", report.report_id, path.display());
    Ok(())
}
