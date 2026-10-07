use psionic_compiler::{
    tassadar_universal_machine_encoding_report_path,
    write_tassadar_universal_machine_encoding_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_universal_machine_encoding_report_path();
    let report = write_tassadar_universal_machine_encoding_report(&output_path)?;
    println!(
        "wrote universal-machine encoding report to {} ({})",
        output_path.display(),
        report.report_digest
    );
    Ok(())
}
