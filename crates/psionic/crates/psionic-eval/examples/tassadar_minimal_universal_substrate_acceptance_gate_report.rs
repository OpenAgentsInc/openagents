use psionic_eval::{
    tassadar_minimal_universal_substrate_acceptance_gate_report_path,
    write_tassadar_minimal_universal_substrate_acceptance_gate_report,
};

fn main() {
    let path = tassadar_minimal_universal_substrate_acceptance_gate_report_path();
    let report = write_tassadar_minimal_universal_substrate_acceptance_gate_report(&path)
        .expect("write minimal universal substrate acceptance gate report");
    println!(
        "wrote minimal universal substrate acceptance gate report to {} ({})",
        path.display(),
        report.report_digest
    );
}
