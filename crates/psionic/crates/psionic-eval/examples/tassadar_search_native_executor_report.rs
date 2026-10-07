use psionic_eval::{
    tassadar_search_native_executor_report_path, write_tassadar_search_native_executor_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_search_native_executor_report_path();
    let report = write_tassadar_search_native_executor_report(&output_path)?;
    println!(
        "wrote search-native executor report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
