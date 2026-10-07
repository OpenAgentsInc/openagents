use psionic_eval::{
    tassadar_numeric_portability_report_path, write_tassadar_numeric_portability_report,
};

fn main() {
    let path = tassadar_numeric_portability_report_path();
    let report =
        write_tassadar_numeric_portability_report(&path).expect("numeric portability report");
    println!(
        "wrote numeric portability report to {} ({})",
        path.display(),
        report.report_id
    );
}
