use psionic_research::{
    tassadar_universality_witness_suite_summary_path,
    write_tassadar_universality_witness_suite_summary,
};

fn main() {
    let path = tassadar_universality_witness_suite_summary_path();
    let summary = write_tassadar_universality_witness_suite_summary(&path)
        .expect("write universality witness suite summary");
    println!(
        "wrote universality witness suite summary to {} ({})",
        path.display(),
        summary.report_digest
    );
}
