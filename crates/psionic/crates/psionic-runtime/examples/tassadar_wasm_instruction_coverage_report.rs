use std::{env, path::PathBuf, process::ExitCode};

use psionic_runtime::{
    tassadar_wasm_instruction_coverage_report_path,
    write_tassadar_wasm_instruction_coverage_report, TASSADAR_WASM_INSTRUCTION_COVERAGE_REPORT_REF,
};

fn main() -> ExitCode {
    let output_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(tassadar_wasm_instruction_coverage_report_path);
    let output_path = if output_path.is_dir() {
        let file_name = PathBuf::from(TASSADAR_WASM_INSTRUCTION_COVERAGE_REPORT_REF)
            .file_name()
            .expect("canonical coverage report ref should have a file name")
            .to_owned();
        output_path.join(file_name)
    } else {
        output_path
    };

    match write_tassadar_wasm_instruction_coverage_report(&output_path) {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("Tassadar Wasm instruction coverage report should serialize"),
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "failed to write Tassadar Wasm instruction coverage report `{}`: {error}",
                output_path.display()
            );
            ExitCode::FAILURE
        }
    }
}
