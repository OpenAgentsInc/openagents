use psionic_eval::{
    tassadar_component_linking_profile_report_path,
    write_tassadar_component_linking_profile_report,
};

fn main() {
    let path = tassadar_component_linking_profile_report_path();
    let report = write_tassadar_component_linking_profile_report(&path)
        .expect("write component-linking report");
    println!(
        "wrote component-linking profile report to {} ({})",
        path.display(),
        report.report_id
    );
}
