use psionic_train::{
    tassadar_architecture_bakeoff_budget_bundle_path,
    write_tassadar_architecture_bakeoff_budget_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_architecture_bakeoff_budget_bundle_path();
    let bundle = write_tassadar_architecture_bakeoff_budget_bundle(&output_path)?;
    println!(
        "wrote architecture bakeoff budget bundle to {} ({})",
        output_path.display(),
        bundle.bundle_digest
    );
    Ok(())
}
