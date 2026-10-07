use psionic_research::{
    run_tassadar_verifier_guided_search_architecture_report,
    TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_OUTPUT_DIR,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = run_tassadar_verifier_guided_search_architecture_report(
        std::path::Path::new(TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_OUTPUT_DIR),
    )?;
    println!(
        "wrote verifier-guided search architecture report to {}/{} ({})",
        TASSADAR_VERIFIER_GUIDED_SEARCH_ARCHITECTURE_OUTPUT_DIR,
        "tassadar_verifier_guided_search_architecture_report.json",
        report.report_digest
    );
    Ok(())
}
