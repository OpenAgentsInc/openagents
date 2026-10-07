use psionic_eval::{
    tassadar_canonical_transformer_stack_boundary_report_path,
    write_tassadar_canonical_transformer_stack_boundary_report,
};

fn main() {
    let report = write_tassadar_canonical_transformer_stack_boundary_report(
        tassadar_canonical_transformer_stack_boundary_report_path(),
    )
    .expect("write canonical transformer stack boundary report");
    println!(
        "wrote {} with dependency_checks={} and tied_requirement_satisfied={}",
        report.report_id,
        report.dependency_checks.len(),
        report.acceptance_gate_tie.tied_requirement_satisfied
    );
}
