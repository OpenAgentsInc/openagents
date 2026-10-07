use psionic_runtime::{
    tassadar_universal_machine_simulation_bundle_path,
    write_tassadar_universal_machine_simulation_bundle,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_path = tassadar_universal_machine_simulation_bundle_path();
    let bundle = write_tassadar_universal_machine_simulation_bundle(&output_path)?;
    println!(
        "wrote universal-machine simulation bundle to {} ({})",
        output_path.display(),
        bundle.bundle_digest
    );
    Ok(())
}
