use std::path::Path;

use psionic_train::write_cs336_a2_fsdp_wrapper_receipt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or("missing repo root")?;
    let receipt = write_cs336_a2_fsdp_wrapper_receipt(repo_root)?;
    println!(
        "wrote {} sharded_parameters={} replicated_parameters={} fp16_supported={}",
        repo_root
            .join(psionic_train::CS336_A2_FSDP_WRAPPER_RECEIPT_FIXTURE_PATH)
            .display(),
        receipt.sharded_parameter_count,
        receipt.replicated_parameter_count,
        receipt.fp16_compute_dtype_supported_in_reference
    );
    Ok(())
}
