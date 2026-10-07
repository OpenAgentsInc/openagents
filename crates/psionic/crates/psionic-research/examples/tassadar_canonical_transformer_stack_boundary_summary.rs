use psionic_research::{
    tassadar_canonical_transformer_stack_boundary_summary_path,
    write_tassadar_canonical_transformer_stack_boundary_summary,
};

fn main() {
    let report = write_tassadar_canonical_transformer_stack_boundary_summary(
        tassadar_canonical_transformer_stack_boundary_summary_path(),
    )
    .expect("write canonical transformer stack boundary summary");
    println!(
        "wrote {} with interface_count={} and tied_requirement_satisfied={}",
        report.report_id, report.interface_count, report.tied_requirement_satisfied
    );
}
