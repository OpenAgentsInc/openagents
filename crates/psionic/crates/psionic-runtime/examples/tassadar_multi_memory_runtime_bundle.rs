use psionic_runtime::{
    tassadar_multi_memory_runtime_bundle_path, write_tassadar_multi_memory_runtime_bundle,
};

fn main() {
    let path = tassadar_multi_memory_runtime_bundle_path();
    let bundle =
        write_tassadar_multi_memory_runtime_bundle(&path).expect("write multi-memory bundle");
    println!(
        "wrote multi-memory runtime bundle to {} ({})",
        path.display(),
        bundle.bundle_id
    );
}
