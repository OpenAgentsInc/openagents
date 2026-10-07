use psionic_eval::{tassadar_memory64_profile_report_path, write_tassadar_memory64_profile_report};

fn main() {
    let path = tassadar_memory64_profile_report_path();
    let report = write_tassadar_memory64_profile_report(&path).expect("memory64 report");
    println!(
        "wrote memory64 profile report to {} ({})",
        path.display(),
        report.report_id
    );
}
