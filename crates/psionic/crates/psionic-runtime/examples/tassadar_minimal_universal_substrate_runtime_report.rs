use psionic_runtime::{
    tassadar_minimal_universal_substrate_runtime_report_path,
    write_tassadar_minimal_universal_substrate_runtime_report,
};

fn main() {
    let path = tassadar_minimal_universal_substrate_runtime_report_path();
    let report = write_tassadar_minimal_universal_substrate_runtime_report(&path)
        .expect("write minimal universal substrate runtime report");
    println!(
        "wrote minimal universal substrate runtime report to {} ({})",
        path.display(),
        report.report_digest
    );
}
