use psionic_eval::{
    tassadar_turing_completeness_closeout_audit_report_path,
    write_tassadar_turing_completeness_closeout_audit_report,
};

fn main() {
    let report = write_tassadar_turing_completeness_closeout_audit_report(
        tassadar_turing_completeness_closeout_audit_report_path(),
    )
    .expect("write Turing-completeness closeout audit report");
    println!(
        "wrote {} with claim_status={:?}",
        report.report_id, report.claim_status
    );
}
