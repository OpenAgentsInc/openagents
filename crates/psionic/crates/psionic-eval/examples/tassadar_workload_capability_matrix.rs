use std::{env, path::PathBuf, process::ExitCode};

use psionic_eval::{
    tassadar_workload_capability_matrix_report_path,
    write_tassadar_workload_capability_matrix_report,
    TASSADAR_WORKLOAD_CAPABILITY_MATRIX_REPORT_REF,
};

fn main() -> ExitCode {
    let output_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(tassadar_workload_capability_matrix_report_path);
    let output_path = if output_path.is_dir() {
        let file_name = PathBuf::from(TASSADAR_WORKLOAD_CAPABILITY_MATRIX_REPORT_REF)
            .file_name()
            .expect("canonical capability matrix ref should have a file name")
            .to_owned();
        output_path.join(file_name)
    } else {
        output_path
    };

    match write_tassadar_workload_capability_matrix_report(&output_path) {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("workload capability matrix report should serialize"),
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "failed to write workload capability matrix report `{}`: {error}",
                output_path.display()
            );
            ExitCode::FAILURE
        }
    }
}
