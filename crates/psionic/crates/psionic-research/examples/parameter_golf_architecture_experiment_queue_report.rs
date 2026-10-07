use psionic_research::{
    parameter_golf_architecture_experiment_queue_report_path,
    write_parameter_golf_architecture_experiment_queue_report,
};

fn main() {
    let output_path = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(parameter_golf_architecture_experiment_queue_report_path);
    let report = write_parameter_golf_architecture_experiment_queue_report(&output_path)
        .expect("write Parameter Golf architecture experiment queue report");
    println!(
        "wrote {} to {}",
        report.report_digest,
        output_path.display()
    );
}
