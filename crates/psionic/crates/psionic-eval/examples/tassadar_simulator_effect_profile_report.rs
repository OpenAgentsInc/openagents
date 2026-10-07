use psionic_eval::{
    tassadar_simulator_effect_profile_report_path, write_tassadar_simulator_effect_profile_report,
};

fn main() {
    let path = tassadar_simulator_effect_profile_report_path();
    let report = write_tassadar_simulator_effect_profile_report(&path)
        .expect("simulator-effect profile report should write");
    println!(
        "wrote simulator-effect profile report to {} ({})",
        path.display(),
        report.report_id
    );
}
