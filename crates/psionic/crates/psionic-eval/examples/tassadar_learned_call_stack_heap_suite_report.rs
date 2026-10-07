use psionic_eval::{
    tassadar_learned_call_stack_heap_suite_report_path,
    write_tassadar_learned_call_stack_heap_suite_report,
};

fn main() {
    let path = tassadar_learned_call_stack_heap_suite_report_path();
    let report = write_tassadar_learned_call_stack_heap_suite_report(&path).expect("write report");
    println!(
        "wrote {} with {} workload rows and {} structured-recoverable workloads",
        path.display(),
        report.workload_summaries.len(),
        report.structured_recoverable_workloads.len(),
    );
}
