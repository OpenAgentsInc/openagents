use std::process::ExitCode;

use psionic_eval::{
    tassadar_dynamic_memory_resume_report_path, write_tassadar_dynamic_memory_resume_report,
};

fn main() -> ExitCode {
    let output_path = tassadar_dynamic_memory_resume_report_path();
    match write_tassadar_dynamic_memory_resume_report(&output_path) {
        Ok(report) => {
            println!(
                "wrote dynamic-memory resume report to {} ({})",
                output_path.display(),
                report.report_digest
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "failed to write dynamic-memory resume report `{}`: {error}",
                output_path.display()
            );
            ExitCode::FAILURE
        }
    }
}
