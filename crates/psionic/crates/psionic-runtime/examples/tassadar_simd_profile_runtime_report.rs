use psionic_runtime::{
    tassadar_simd_profile_runtime_report_path, write_tassadar_simd_profile_runtime_report,
};

fn main() {
    let path = tassadar_simd_profile_runtime_report_path();
    let report =
        write_tassadar_simd_profile_runtime_report(&path).expect("write simd runtime report");
    println!(
        "wrote SIMD profile runtime report to {} ({})",
        path.display(),
        report.report_id
    );
}
