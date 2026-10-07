use psionic_sandbox::{
    tassadar_virtual_fs_mount_sandbox_boundary_report_path,
    write_tassadar_virtual_fs_mount_sandbox_boundary_report,
};

fn main() {
    let path = tassadar_virtual_fs_mount_sandbox_boundary_report_path();
    let report = write_tassadar_virtual_fs_mount_sandbox_boundary_report(&path)
        .expect("virtual-fs sandbox boundary report should write");
    println!(
        "wrote virtual-fs sandbox boundary report to {} ({})",
        path.display(),
        report.report_id
    );
}
