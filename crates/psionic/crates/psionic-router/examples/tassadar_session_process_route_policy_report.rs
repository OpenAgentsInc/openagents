use psionic_router::{
    tassadar_session_process_route_policy_report_path,
    write_tassadar_session_process_route_policy_report,
};

fn main() {
    let path = tassadar_session_process_route_policy_report_path();
    let report = write_tassadar_session_process_route_policy_report(&path)
        .expect("session-process route policy report");
    println!(
        "wrote session-process route policy to {} ({})",
        path.display(),
        report.report_id
    );
}
