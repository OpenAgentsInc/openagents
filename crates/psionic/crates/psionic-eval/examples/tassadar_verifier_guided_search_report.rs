use psionic_eval::{
    tassadar_verifier_guided_search_evaluation_report_path,
    write_tassadar_verifier_guided_search_evaluation_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_verifier_guided_search_evaluation_report(
        tassadar_verifier_guided_search_evaluation_report_path(),
    )?;
    println!(
        "wrote verifier-guided search report to {} ({})",
        tassadar_verifier_guided_search_evaluation_report_path().display(),
        report.report_digest
    );
    Ok(())
}
