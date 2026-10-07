use psionic_eval::{
    tassadar_float_profile_acceptance_gate_report_path,
    write_tassadar_float_profile_acceptance_gate_report,
};

fn main() {
    let path = tassadar_float_profile_acceptance_gate_report_path();
    let report =
        write_tassadar_float_profile_acceptance_gate_report(&path).expect("gate report");
    println!(
        "wrote float profile acceptance gate to {} ({})",
        path.display(),
        report.report_id
    );
}
