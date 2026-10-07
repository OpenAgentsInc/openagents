use psionic_eval::{
    tassadar_internal_component_abi_report_path, write_tassadar_internal_component_abi_report,
};

fn main() {
    let path = tassadar_internal_component_abi_report_path();
    let report = write_tassadar_internal_component_abi_report(&path)
        .expect("internal component ABI report should write");
    println!(
        "wrote internal component ABI report to {} ({})",
        path.display(),
        report.report_id
    );
}
