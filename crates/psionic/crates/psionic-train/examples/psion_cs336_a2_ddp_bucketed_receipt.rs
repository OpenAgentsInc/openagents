use std::path::Path;

use psionic_train::{
    CS336_A2_DDP_BUCKETED_RECEIPT_FIXTURE_PATH, write_cs336_a2_ddp_bucketed_receipt,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or("missing repo root")?;
    let receipt = write_cs336_a2_ddp_bucketed_receipt(repo_root)?;
    println!(
        "wrote {} steps={} bucket_cases={} matches_baseline={}",
        repo_root
            .join(CS336_A2_DDP_BUCKETED_RECEIPT_FIXTURE_PATH)
            .display(),
        receipt.steps.len(),
        receipt.bucket_cases.len(),
        receipt.all_steps_match_baseline,
    );
    Ok(())
}
