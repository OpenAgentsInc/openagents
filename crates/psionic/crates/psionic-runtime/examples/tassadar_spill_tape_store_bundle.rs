use psionic_runtime::{
    tassadar_spill_tape_store_runtime_bundle_path, write_tassadar_spill_tape_store_runtime_bundle,
};

fn main() {
    let path = tassadar_spill_tape_store_runtime_bundle_path();
    let bundle = write_tassadar_spill_tape_store_runtime_bundle(&path)
        .expect("spill/tape runtime bundle should write");
    println!(
        "wrote spill/tape runtime bundle to {} ({})",
        path.display(),
        bundle.bundle_id
    );
}
