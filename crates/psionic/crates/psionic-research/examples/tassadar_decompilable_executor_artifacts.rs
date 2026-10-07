use psionic_research::{
    tassadar_decompilable_executor_artifacts_report_path,
    write_tassadar_decompilable_executor_artifacts_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_decompilable_executor_artifacts_report_path();
    let report = write_tassadar_decompilable_executor_artifacts_report(&output_path)?;
    println!("wrote {} ({})", output_path.display(), report.report_digest);
    Ok(())
}
