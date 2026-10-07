use psionic_runtime::{
    tassadar_threads_research_profile_report_path,
    write_tassadar_threads_research_profile_runtime_report,
};

fn main() {
    let path = tassadar_threads_research_profile_report_path();
    let report = write_tassadar_threads_research_profile_runtime_report(&path)
        .expect("write threads report");
    println!(
        "wrote threads research runtime report to {} ({})",
        path.display(),
        report.report_id
    );
}
