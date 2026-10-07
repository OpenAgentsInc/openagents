use psionic_cluster::{
    tassadar_preemptive_job_fairness_report_path, write_tassadar_preemptive_job_fairness_report,
};

fn main() {
    let path = tassadar_preemptive_job_fairness_report_path();
    let report = write_tassadar_preemptive_job_fairness_report(&path)
        .expect("preemptive-job fairness report should write");
    println!(
        "wrote preemptive-job fairness report to {} ({})",
        path.display(),
        report.report_id
    );
}
