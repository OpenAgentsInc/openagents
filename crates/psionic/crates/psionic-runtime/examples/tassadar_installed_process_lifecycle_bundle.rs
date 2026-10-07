use psionic_runtime::{
    tassadar_installed_process_lifecycle_runtime_bundle_path,
    write_tassadar_installed_process_lifecycle_runtime_bundle,
};

fn main() {
    let path = tassadar_installed_process_lifecycle_runtime_bundle_path();
    let bundle =
        write_tassadar_installed_process_lifecycle_runtime_bundle(&path).expect("write bundle");
    println!(
        "wrote {} with {} migration cases, {} rollback cases, and {} refusal rows",
        path.display(),
        bundle.exact_migration_case_count,
        bundle.exact_rollback_case_count,
        bundle.refusal_case_count,
    );
}
