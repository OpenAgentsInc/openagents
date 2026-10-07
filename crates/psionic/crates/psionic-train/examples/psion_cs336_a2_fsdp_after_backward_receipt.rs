use std::path::Path;

use psionic_train::write_cs336_a2_fsdp_after_backward_receipt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or("missing repo root")?;
    let receipt = write_cs336_a2_fsdp_after_backward_receipt(repo_root)?;
    let sync_count = receipt
        .compute_cases
        .iter()
        .map(|case| case.gradient_syncs.len())
        .sum::<usize>();
    println!(
        "wrote {} compute_cases={} gradient_syncs={} fp16_case={} baseline_match={}",
        repo_root
            .join(psionic_train::CS336_A2_FSDP_AFTER_BACKWARD_RECEIPT_FIXTURE_PATH)
            .display(),
        receipt.compute_cases.len(),
        sync_count,
        receipt
            .compute_cases
            .iter()
            .any(|case| case.compute_dtype == "fp16"),
        receipt.all_cases_match_non_parallel_baseline
    );
    Ok(())
}
