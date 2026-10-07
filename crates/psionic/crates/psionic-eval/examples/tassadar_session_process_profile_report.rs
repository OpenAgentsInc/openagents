use std::process::ExitCode;

use psionic_eval::{
    tassadar_session_process_profile_report_path, write_tassadar_session_process_profile_report,
};

fn main() -> ExitCode {
    let output_path = tassadar_session_process_profile_report_path();
    match write_tassadar_session_process_profile_report(&output_path) {
        Ok(report) => {
            println!(
                "wrote session-process profile report to {} ({})",
                output_path.display(),
                report.report_digest
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "failed to write session-process profile report `{}`: {error}",
                output_path.display()
            );
            ExitCode::FAILURE
        }
    }
}
