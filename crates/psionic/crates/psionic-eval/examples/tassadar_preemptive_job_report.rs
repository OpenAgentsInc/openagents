use psionic_eval::{
    tassadar_preemptive_job_profile_report_path, write_tassadar_preemptive_job_profile_report,
};

fn main() {
    let path = tassadar_preemptive_job_profile_report_path();
    let report = write_tassadar_preemptive_job_profile_report(&path)
        .expect("preemptive-job profile report should write");
    println!(
        "wrote preemptive-job profile report to {} ({})",
        path.display(),
        report.report_id
    );
}
