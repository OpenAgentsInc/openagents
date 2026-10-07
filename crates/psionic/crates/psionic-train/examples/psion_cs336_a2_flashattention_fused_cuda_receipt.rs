use std::path::Path;

use psionic_train::{
    CS336_A2_FLASHATTENTION_FUSED_CUDA_RECEIPT_FIXTURE_PATH,
    write_cs336_a2_flashattention_fused_cuda_receipt,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or("missing repo root")?;
    let receipt = write_cs336_a2_flashattention_fused_cuda_receipt(repo_root)?;
    println!(
        "wrote {} supports_cuda={} refusal_reason={}",
        repo_root
            .join(CS336_A2_FLASHATTENTION_FUSED_CUDA_RECEIPT_FIXTURE_PATH)
            .display(),
        receipt.capability.supports_bounded_fused_attention,
        receipt
            .capability
            .refusal_reason
            .as_deref()
            .unwrap_or("none"),
    );
    Ok(())
}
