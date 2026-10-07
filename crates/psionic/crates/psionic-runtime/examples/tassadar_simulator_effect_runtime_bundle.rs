use psionic_runtime::{
    tassadar_simulator_effect_runtime_bundle_path, write_tassadar_simulator_effect_runtime_bundle,
};

fn main() {
    let path = tassadar_simulator_effect_runtime_bundle_path();
    let bundle = write_tassadar_simulator_effect_runtime_bundle(&path)
        .expect("simulator-effect runtime bundle should write");
    println!(
        "wrote simulator-effect runtime bundle to {} ({})",
        path.display(),
        bundle.bundle_id
    );
}
