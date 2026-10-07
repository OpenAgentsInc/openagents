use psionic_eval::{
    tassadar_exception_profile_report_path, write_tassadar_exception_profile_report,
};

fn main() {
    let path = tassadar_exception_profile_report_path();
    let report = write_tassadar_exception_profile_report(&path).expect("exception profile report");
    println!(
        "wrote exception profile report to {} ({})",
        path.display(),
        report.report_id
    );
}
