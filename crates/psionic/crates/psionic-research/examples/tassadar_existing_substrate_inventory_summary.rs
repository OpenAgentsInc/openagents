use psionic_research::{
    tassadar_existing_substrate_inventory_summary_path,
    write_tassadar_existing_substrate_inventory_summary,
};

fn main() {
    let report = write_tassadar_existing_substrate_inventory_summary(
        tassadar_existing_substrate_inventory_summary_path(),
    )
    .expect("write existing substrate inventory summary");
    println!(
        "wrote {} with blocker_surface_count={} and tied_requirement_satisfied={}",
        report.report_id,
        report.blocker_surface_count,
        report.tied_requirement_satisfied
    );
}
