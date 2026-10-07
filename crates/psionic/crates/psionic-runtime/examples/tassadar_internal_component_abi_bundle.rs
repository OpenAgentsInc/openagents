use psionic_runtime::{
    tassadar_internal_component_abi_bundle_path, write_tassadar_internal_component_abi_bundle,
};

fn main() {
    let path = tassadar_internal_component_abi_bundle_path();
    let bundle = write_tassadar_internal_component_abi_bundle(&path)
        .expect("internal component ABI bundle should write");
    println!(
        "wrote internal component ABI bundle to {} ({})",
        path.display(),
        bundle.bundle_id
    );
}
