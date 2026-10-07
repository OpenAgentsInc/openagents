use psionic_runtime::{
    tassadar_tcm_v1_runtime_contract_report_path, write_tassadar_tcm_v1_runtime_contract_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_tcm_v1_runtime_contract_report_path();
    let report = write_tassadar_tcm_v1_runtime_contract_report(&output_path)?;
    println!(
        "wrote TCM.v1 runtime contract report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
