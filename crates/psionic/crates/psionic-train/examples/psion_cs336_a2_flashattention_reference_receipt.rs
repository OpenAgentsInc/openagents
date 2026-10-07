use std::path::Path;

use psionic_train::{
    CS336_A2_FLASHATTENTION_REFERENCE_RECEIPT_FIXTURE_PATH,
    write_cs336_a2_flashattention_reference_receipt,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or("missing repo root")?;
    let receipt = write_cs336_a2_flashattention_reference_receipt(repo_root)?;
    println!(
        "wrote {} forward_output_max_abs_diff={:.6} backward_d_query_max_abs_diff={:.6}",
        repo_root
            .join(CS336_A2_FLASHATTENTION_REFERENCE_RECEIPT_FIXTURE_PATH)
            .display(),
        receipt.forward_output_max_abs_diff,
        receipt.backward_d_query_max_abs_diff,
    );
    Ok(())
}
