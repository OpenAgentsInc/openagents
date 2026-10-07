use std::process::ExitCode;

use psionic_eval::{
    tassadar_module_scale_workload_suite_report_path,
    write_tassadar_module_scale_workload_suite_report,
};

fn main() -> ExitCode {
    let output_path = tassadar_module_scale_workload_suite_report_path();
    match write_tassadar_module_scale_workload_suite_report(&output_path) {
        Ok(report) => {
            println!(
                "wrote module-scale workload suite report to {} ({})",
                output_path.display(),
                report.report_digest
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "failed to write module-scale workload suite report `{}`: {error}",
                output_path.display()
            );
            ExitCode::FAILURE
        }
    }
}
