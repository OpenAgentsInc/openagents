use psionic_research::{
    parameter_golf_research_harness_report_path, write_parameter_golf_research_harness_report,
};

fn main() {
    let path = parameter_golf_research_harness_report_path();
    let report =
        write_parameter_golf_research_harness_report(&path).expect("write Parameter Golf report");
    println!(
        "wrote {} with digest {}",
        path.display(),
        report.report_digest
    );
}
