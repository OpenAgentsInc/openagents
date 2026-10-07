use psionic_eval::{
    tassadar_existing_substrate_inventory_report_path,
    write_tassadar_existing_substrate_inventory_report,
};

fn main() {
    let report = write_tassadar_existing_substrate_inventory_report(
        tassadar_existing_substrate_inventory_report_path(),
    )
    .expect("write existing substrate inventory report");
    println!(
        "wrote {} with surface_count={} and tied_requirement_satisfied={}",
        report.report_id,
        report.surface_count,
        report.acceptance_gate_tie.tied_requirement_satisfied
    );
}
