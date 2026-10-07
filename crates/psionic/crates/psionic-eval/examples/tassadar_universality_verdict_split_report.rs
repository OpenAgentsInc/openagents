use psionic_eval::{
    tassadar_universality_verdict_split_report_path,
    write_tassadar_universality_verdict_split_report,
};

fn main() {
    let report = write_tassadar_universality_verdict_split_report(
        tassadar_universality_verdict_split_report_path(),
    )
    .expect("write universality verdict split report");
    println!(
        "wrote {} with theory_green={}, operator_green={}, served_green={}",
        report.report_id, report.theory_green, report.operator_green, report.served_green
    );
}
