use psionic_eval::{
    tassadar_locality_scratchpad_report_path, write_tassadar_locality_scratchpad_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_locality_scratchpad_report(tassadar_locality_scratchpad_report_path())?;
    println!(
        "wrote locality scratchpad report to {} ({})",
        tassadar_locality_scratchpad_report_path().display(),
        report.report_digest
    );
    Ok(())
}
