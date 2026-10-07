use psionic_train::{
    write_tassadar_learnability_gap_evidence_bundle, TASSADAR_LEARNABILITY_GAP_OUTPUT_DIR,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bundle =
        write_tassadar_learnability_gap_evidence_bundle(TASSADAR_LEARNABILITY_GAP_OUTPUT_DIR)?;
    println!(
        "wrote {}/{} ({})",
        TASSADAR_LEARNABILITY_GAP_OUTPUT_DIR,
        psionic_train::TASSADAR_LEARNABILITY_GAP_REPORT_FILE,
        bundle.bundle_digest
    );
    Ok(())
}
