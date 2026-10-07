use psionic_eval::{
    tassadar_spill_tape_store_report_path, write_tassadar_spill_tape_store_report,
};

fn main() {
    let output_path = tassadar_spill_tape_store_report_path();
    let report =
        write_tassadar_spill_tape_store_report(&output_path).expect("spill/tape report writes");
    println!(
        "wrote spill/tape report to {} ({})",
        output_path.display(),
        report.report_digest
    );
}
