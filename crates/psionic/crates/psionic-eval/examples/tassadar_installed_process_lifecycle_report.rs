use psionic_eval::{
    tassadar_installed_process_lifecycle_report_path,
    write_tassadar_installed_process_lifecycle_report,
};

fn main() {
    let path = tassadar_installed_process_lifecycle_report_path();
    let report = write_tassadar_installed_process_lifecycle_report(&path).expect("write report");
    println!(
        "wrote {} with {} migration cases, {} rollback cases, and {} refusal rows",
        path.display(),
        report.exact_migration_case_count,
        report.exact_rollback_case_count,
        report.refusal_case_count,
    );
}
