use psionic_research::{
    tassadar_learned_call_stack_heap_suite_summary_path,
    write_tassadar_learned_call_stack_heap_suite_summary,
};

fn main() {
    let path = tassadar_learned_call_stack_heap_suite_summary_path();
    let summary =
        write_tassadar_learned_call_stack_heap_suite_summary(&path).expect("write summary");
    println!(
        "wrote {} with {} strong in-family workloads and {} held-out recoverable workloads",
        path.display(),
        summary.strong_in_family_workloads.len(),
        summary.held_out_recoverable_workloads.len(),
    );
}
