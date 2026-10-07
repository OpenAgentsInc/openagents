use psionic_runtime::{
    tassadar_component_linking_runtime_bundle_path,
    write_tassadar_component_linking_runtime_bundle,
};

fn main() {
    let path = tassadar_component_linking_runtime_bundle_path();
    let bundle = write_tassadar_component_linking_runtime_bundle(&path)
        .expect("write component-linking bundle");
    println!(
        "wrote component-linking runtime bundle to {} ({})",
        path.display(),
        bundle.bundle_id
    );
}
