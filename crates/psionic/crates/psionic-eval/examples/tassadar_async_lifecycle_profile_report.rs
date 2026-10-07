use psionic_eval::{
    tassadar_async_lifecycle_profile_report_path, write_tassadar_async_lifecycle_profile_report,
};

fn main() {
    let path = tassadar_async_lifecycle_profile_report_path();
    let report = write_tassadar_async_lifecycle_profile_report(&path)
        .expect("async-lifecycle profile report should write");
    println!(
        "wrote async-lifecycle profile report to {} ({})",
        path.display(),
        report.report_id
    );
}
