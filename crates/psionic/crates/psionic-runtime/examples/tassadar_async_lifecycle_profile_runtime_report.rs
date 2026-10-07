use psionic_runtime::{
    tassadar_async_lifecycle_profile_runtime_report_path,
    write_tassadar_async_lifecycle_profile_runtime_report,
};

fn main() {
    let path = tassadar_async_lifecycle_profile_runtime_report_path();
    let report = write_tassadar_async_lifecycle_profile_runtime_report(&path)
        .expect("async-lifecycle runtime report should write");
    println!(
        "wrote async-lifecycle runtime report to {} ({})",
        path.display(),
        report.report_id
    );
}
