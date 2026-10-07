use psionic_router::{
    tassadar_async_lifecycle_route_policy_report_path,
    write_tassadar_async_lifecycle_route_policy_report,
};

fn main() {
    let path = tassadar_async_lifecycle_route_policy_report_path();
    let report = write_tassadar_async_lifecycle_route_policy_report(&path)
        .expect("async-lifecycle route policy report should write");
    println!(
        "wrote async-lifecycle route policy report to {} ({})",
        path.display(),
        report.report_id
    );
}
