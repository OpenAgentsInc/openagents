use psionic_sandbox::{
    tassadar_simulator_effect_sandbox_boundary_report_path,
    write_tassadar_simulator_effect_sandbox_boundary_report,
};

fn main() {
    let path = tassadar_simulator_effect_sandbox_boundary_report_path();
    let report = write_tassadar_simulator_effect_sandbox_boundary_report(&path)
        .expect("simulator-effect sandbox boundary report should write");
    println!(
        "wrote simulator-effect sandbox boundary report to {} ({})",
        path.display(),
        report.report_id
    );
}
