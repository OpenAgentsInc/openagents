use std::path::Path;

use psionic_train::write_cs336_a2_fsdp_full_params_receipt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or("missing repo root")?;
    let receipt = write_cs336_a2_fsdp_full_params_receipt(repo_root)?;
    let step_count = receipt
        .compute_cases
        .iter()
        .map(|case| case.step_receipts.len())
        .sum::<usize>();
    println!(
        "wrote {} compute_cases={} steps={} parameters={} baseline_match={}",
        repo_root
            .join(psionic_train::CS336_A2_FSDP_FULL_PARAMS_RECEIPT_FIXTURE_PATH)
            .display(),
        receipt.compute_cases.len(),
        step_count,
        receipt.expected_parameter_names.len(),
        receipt.all_cases_match_non_parallel_baseline
    );
    Ok(())
}
