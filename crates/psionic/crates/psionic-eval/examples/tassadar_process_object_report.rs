use std::process::ExitCode;

use psionic_eval::{tassadar_process_object_report_path, write_tassadar_process_object_report};

fn main() -> ExitCode {
    let output_path = tassadar_process_object_report_path();
    match write_tassadar_process_object_report(&output_path) {
        Ok(report) => {
            println!(
                "wrote process-object report to {} ({})",
                output_path.display(),
                report.report_digest
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "failed to write process-object report `{}`: {error}",
                output_path.display()
            );
            ExitCode::FAILURE
        }
    }
}
