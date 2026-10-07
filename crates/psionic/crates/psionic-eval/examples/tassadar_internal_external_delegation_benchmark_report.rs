use psionic_eval::{
    tassadar_internal_external_delegation_benchmark_report_path,
    write_tassadar_internal_external_delegation_benchmark_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = tassadar_internal_external_delegation_benchmark_report_path();
    let report = write_tassadar_internal_external_delegation_benchmark_report(&path)?;
    println!("wrote {} to {}", report.report_id, path.display());
    Ok(())
}
