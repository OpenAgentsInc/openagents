use std::path::Path;

use psionic_train::{
    TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_OUTPUT_DIR,
    execute_tassadar_learned_call_stack_heap_suite,
};

fn main() {
    let output_dir = Path::new(TASSADAR_LEARNED_CALL_STACK_HEAP_SUITE_OUTPUT_DIR);
    let bundle = execute_tassadar_learned_call_stack_heap_suite(output_dir).expect("write bundle");
    println!(
        "wrote {} case rows to {}",
        bundle.case_reports.len(),
        output_dir.display(),
    );
}
