use std::{fs, path::Path};

use psionic_serve::{
    TASSADAR_DIRECT_MODEL_WEIGHT_EXECUTION_PROOF_REPORT_REF,
    write_tassadar_direct_model_weight_execution_proof_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(TASSADAR_DIRECT_MODEL_WEIGHT_EXECUTION_PROOF_REPORT_REF);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let report = write_tassadar_direct_model_weight_execution_proof_report(path)?;
    println!(
        "wrote {} ({})",
        TASSADAR_DIRECT_MODEL_WEIGHT_EXECUTION_PROOF_REPORT_REF, report.report_digest
    );
    Ok(())
}
