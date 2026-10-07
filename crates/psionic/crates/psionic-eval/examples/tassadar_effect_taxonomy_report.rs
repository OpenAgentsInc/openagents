use psionic_eval::{tassadar_effect_taxonomy_report_path, write_tassadar_effect_taxonomy_report};

fn main() {
    let path = tassadar_effect_taxonomy_report_path();
    let report = write_tassadar_effect_taxonomy_report(&path).expect("effect taxonomy report");
    println!(
        "wrote {} with {} cases",
        path.display(),
        report.case_reports.len()
    );
}
