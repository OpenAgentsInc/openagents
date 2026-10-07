use psionic_eval::{
    tassadar_multi_memory_profile_report_path, write_tassadar_multi_memory_profile_report,
};

fn main() {
    let path = tassadar_multi_memory_profile_report_path();
    let report =
        write_tassadar_multi_memory_profile_report(&path).expect("write multi-memory report");
    println!(
        "wrote multi-memory profile report to {} ({})",
        path.display(),
        report.report_id
    );
}
