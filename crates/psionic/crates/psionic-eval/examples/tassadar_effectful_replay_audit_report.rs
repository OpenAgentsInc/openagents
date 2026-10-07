use psionic_eval::{
    tassadar_effectful_replay_audit_report_path, write_tassadar_effectful_replay_audit_report,
};

fn main() {
    let path = tassadar_effectful_replay_audit_report_path();
    let report = write_tassadar_effectful_replay_audit_report(&path)
        .expect("effectful replay audit report should write");
    println!(
        "wrote effectful replay audit report to {} ({})",
        path.display(),
        report.report_id
    );
}
