use std::path::Path;

use psionic_train::{
    CS336_A2_DDP_INDIVIDUAL_PARAMETERS_RECEIPT_FIXTURE_PATH,
    write_cs336_a2_ddp_individual_parameters_receipt,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or("missing repo root")?;
    let receipt = write_cs336_a2_ddp_individual_parameters_receipt(repo_root)?;
    println!(
        "wrote {} steps={} matches_baseline={}",
        repo_root
            .join(CS336_A2_DDP_INDIVIDUAL_PARAMETERS_RECEIPT_FIXTURE_PATH)
            .display(),
        receipt.steps.len(),
        receipt.all_steps_match_baseline,
    );
    Ok(())
}
