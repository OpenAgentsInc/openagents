use psionic_eval::{tassadar_simd_profile_report_path, write_tassadar_simd_profile_report};

fn main() {
    let path = tassadar_simd_profile_report_path();
    let report = write_tassadar_simd_profile_report(&path).expect("write simd report");
    println!(
        "wrote SIMD profile report to {} ({})",
        path.display(),
        report.report_id
    );
}
