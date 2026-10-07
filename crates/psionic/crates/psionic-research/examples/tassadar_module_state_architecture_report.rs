use std::process::ExitCode;

use psionic_research::{
    tassadar_module_state_architecture_report_path,
    write_tassadar_module_state_architecture_report,
};

fn main() -> ExitCode {
    let output_path = tassadar_module_state_architecture_report_path();
    match write_tassadar_module_state_architecture_report(&output_path) {
        Ok(report) => {
            println!(
                "wrote module-state architecture report to {} ({})",
                output_path.display(),
                report.report_digest
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "failed to write module-state architecture report `{}`: {error}",
                output_path.display()
            );
            ExitCode::FAILURE
        }
    }
}
