use std::{env, path::PathBuf, process::ExitCode};

use psionic_eval::{
    tassadar_hull_cache_closure_report_path, write_tassadar_hull_cache_closure_report,
    TASSADAR_HULL_CACHE_CLOSURE_REPORT_REF,
};

fn main() -> ExitCode {
    let output_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(tassadar_hull_cache_closure_report_path);
    let output_path = if output_path.is_dir() {
        let file_name = PathBuf::from(TASSADAR_HULL_CACHE_CLOSURE_REPORT_REF)
            .file_name()
            .expect("canonical hull-cache closure report ref should have a file name")
            .to_owned();
        output_path.join(file_name)
    } else {
        output_path
    };

    match write_tassadar_hull_cache_closure_report(&output_path) {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("HullCache closure report should serialize"),
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "failed to write HullCache closure report `{}`: {error}",
                output_path.display()
            );
            ExitCode::FAILURE
        }
    }
}
