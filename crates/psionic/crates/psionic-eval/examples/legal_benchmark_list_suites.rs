use std::error::Error;

use psionic_eval::legal_benchmark_public_suite_catalog;

fn main() -> Result<(), Box<dyn Error>> {
    for suite in legal_benchmark_public_suite_catalog() {
        println!(
            "{}\t{}\ttasks={}\tmaterialized={}\tpromotion_target={}\t{}",
            suite.suite_level,
            suite.path,
            suite.task_count,
            suite.materialized,
            suite.promotion_target,
            suite.notes
        );
    }
    Ok(())
}
