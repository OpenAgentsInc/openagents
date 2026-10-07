use psionic_runtime::{
    tassadar_session_process_profile_runtime_report_path,
    write_tassadar_session_process_profile_runtime_report,
};

fn main() {
    let path = tassadar_session_process_profile_runtime_report_path();
    let report = write_tassadar_session_process_profile_runtime_report(&path)
        .expect("write session-process runtime report");
    println!(
        "wrote session-process runtime report to {} ({})",
        path.display(),
        report.report_id
    );
}
