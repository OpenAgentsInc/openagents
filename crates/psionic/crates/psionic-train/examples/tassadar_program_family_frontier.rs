use std::path::Path;

use psionic_train::{
    execute_tassadar_program_family_frontier, TASSADAR_PROGRAM_FAMILY_FRONTIER_OUTPUT_DIR,
};

fn main() {
    let output_dir = Path::new(TASSADAR_PROGRAM_FAMILY_FRONTIER_OUTPUT_DIR);
    let bundle = execute_tassadar_program_family_frontier(output_dir).expect("write bundle");
    println!(
        "wrote {} case rows to {}",
        bundle.case_reports.len(),
        output_dir.display(),
    );
}
