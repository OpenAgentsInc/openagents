use psionic_eval::{
    tassadar_universality_witness_suite_report_path,
    write_tassadar_universality_witness_suite_report,
};

fn main() {
    let path = tassadar_universality_witness_suite_report_path();
    let report = write_tassadar_universality_witness_suite_report(&path)
        .expect("write witness suite report");
    println!(
        "wrote universality witness suite report to {} ({})",
        path.display(),
        report.report_digest
    );
}
