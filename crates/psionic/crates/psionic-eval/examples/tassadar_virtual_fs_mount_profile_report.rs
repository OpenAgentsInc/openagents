use psionic_eval::{
    tassadar_virtual_fs_mount_profile_report_path,
    write_tassadar_virtual_fs_mount_profile_report,
};

fn main() {
    let path = tassadar_virtual_fs_mount_profile_report_path();
    let report = write_tassadar_virtual_fs_mount_profile_report(&path)
        .expect("virtual-fs profile report should write");
    println!(
        "wrote virtual-fs profile report to {} ({})",
        path.display(),
        report.report_id
    );
}
