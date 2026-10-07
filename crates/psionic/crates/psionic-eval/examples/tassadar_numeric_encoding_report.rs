use psionic_eval::{tassadar_numeric_encoding_report_path, write_tassadar_numeric_encoding_report};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = write_tassadar_numeric_encoding_report(tassadar_numeric_encoding_report_path())?;
    println!(
        "wrote numeric encoding report to {} ({})",
        tassadar_numeric_encoding_report_path().display(),
        report.report_digest
    );
    Ok(())
}
