use std::process::ExitCode;

use psionic_eval::{
    tassadar_module_trace_abi_v2_report_path, write_tassadar_module_trace_abi_v2_report,
};

fn main() -> ExitCode {
    let output_path = tassadar_module_trace_abi_v2_report_path();
    match write_tassadar_module_trace_abi_v2_report(&output_path) {
        Ok(report) => {
            println!(
                "wrote module trace ABI v2 report to {} ({})",
                output_path.display(),
                report.report_digest
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "failed to write module trace ABI v2 report `{}`: {error}",
                output_path.display()
            );
            ExitCode::FAILURE
        }
    }
}
