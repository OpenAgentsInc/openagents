use psionic_runtime::{
    tassadar_virtual_fs_mount_runtime_bundle_path, write_tassadar_virtual_fs_mount_runtime_bundle,
};

fn main() {
    let path = tassadar_virtual_fs_mount_runtime_bundle_path();
    let bundle = write_tassadar_virtual_fs_mount_runtime_bundle(&path)
        .expect("virtual-fs runtime bundle should write");
    println!(
        "wrote virtual-fs runtime bundle to {} ({})",
        path.display(),
        bundle.bundle_id
    );
}
