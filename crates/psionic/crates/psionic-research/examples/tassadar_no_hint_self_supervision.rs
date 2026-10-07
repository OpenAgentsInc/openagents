use std::path::Path;

use psionic_research::{
    run_tassadar_no_hint_self_supervised_report, TASSADAR_NO_HINT_SELF_SUPERVISION_OUTPUT_DIR,
    TASSADAR_NO_HINT_SELF_SUPERVISION_REPORT_FILE,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = run_tassadar_no_hint_self_supervised_report(Path::new(
        TASSADAR_NO_HINT_SELF_SUPERVISION_OUTPUT_DIR,
    ))?;
    println!(
        "wrote {}/{} ({})",
        TASSADAR_NO_HINT_SELF_SUPERVISION_OUTPUT_DIR,
        TASSADAR_NO_HINT_SELF_SUPERVISION_REPORT_FILE,
        report.report_digest
    );
    Ok(())
}
