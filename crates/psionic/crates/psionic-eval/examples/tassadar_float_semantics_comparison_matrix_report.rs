use psionic_eval::{
    tassadar_float_semantics_report_path, write_tassadar_float_semantics_comparison_matrix_report,
};

fn main() {
    let path = tassadar_float_semantics_report_path();
    let report = write_tassadar_float_semantics_comparison_matrix_report(&path)
        .expect("float semantics report should write");
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("report should serialize")
    );
}
