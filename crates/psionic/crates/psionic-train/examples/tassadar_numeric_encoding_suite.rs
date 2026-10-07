use psionic_train::{
    execute_tassadar_numeric_encoding_suite, TASSADAR_NUMERIC_ENCODING_SUITE_OUTPUT_DIR,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = execute_tassadar_numeric_encoding_suite(std::path::Path::new(
        TASSADAR_NUMERIC_ENCODING_SUITE_OUTPUT_DIR,
    ))?;
    println!(
        "wrote numeric encoding suite to {}/{} ({})",
        TASSADAR_NUMERIC_ENCODING_SUITE_OUTPUT_DIR,
        "numeric_encoding_suite.json",
        report.report_digest
    );
    Ok(())
}
