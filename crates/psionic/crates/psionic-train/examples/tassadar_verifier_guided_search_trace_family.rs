use psionic_train::{
    execute_tassadar_verifier_guided_search_trace_family,
    TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_OUTPUT_DIR,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = execute_tassadar_verifier_guided_search_trace_family(std::path::Path::new(
        TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_OUTPUT_DIR,
    ))?;
    println!(
        "wrote verifier-guided search trace-family report to {}/{} ({})",
        TASSADAR_VERIFIER_GUIDED_SEARCH_TRACE_FAMILY_OUTPUT_DIR,
        "search_trace_family_report.json",
        report.report_digest
    );
    Ok(())
}
