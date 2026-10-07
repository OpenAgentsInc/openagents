use psionic_train::{
    execute_tassadar_weak_supervision_executor, TASSADAR_WEAK_SUPERVISION_OUTPUT_DIR,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = execute_tassadar_weak_supervision_executor(std::path::Path::new(
        TASSADAR_WEAK_SUPERVISION_OUTPUT_DIR,
    ))?;
    println!(
        "wrote weak-supervision evidence bundle to {}/{} ({})",
        TASSADAR_WEAK_SUPERVISION_OUTPUT_DIR,
        "weak_supervision_evidence_bundle.json",
        report.report_digest
    );
    Ok(())
}
