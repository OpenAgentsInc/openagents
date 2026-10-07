use std::path::Path;

use psionic_train::{
    TASSADAR_RECEIPT_SUPERVISION_OUTPUT_DIR, build_tassadar_receipt_supervision_report,
    execute_tassadar_receipt_supervision_bundle, write_tassadar_receipt_supervision_report,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new(TASSADAR_RECEIPT_SUPERVISION_OUTPUT_DIR);
    let bundle = execute_tassadar_receipt_supervision_bundle(output_dir)?;
    let report = build_tassadar_receipt_supervision_report(&bundle);
    write_tassadar_receipt_supervision_report(&report)?;
    println!(
        "wrote {} cases to {}",
        bundle.cases.len(),
        output_dir.display()
    );
    Ok(())
}
