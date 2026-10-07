use std::path::Path;

use psionic_train::write_cs336_a2_sharded_optimizer_receipt;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or("missing repo root")?;
    let receipt = write_cs336_a2_sharded_optimizer_receipt(repo_root)?;
    println!(
        "wrote {} steps={} ranks={} matches_baseline={}",
        repo_root
            .join(psionic_train::CS336_A2_SHARDED_OPTIMIZER_RECEIPT_FIXTURE_PATH)
            .display(),
        receipt.steps.len(),
        receipt.layout.world_size,
        receipt.all_steps_match_baseline
    );
    Ok(())
}
