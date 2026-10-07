use std::{env, path::PathBuf, process::ExitCode};

use psionic_research::{
    TASSADAR_LEARNED_HORIZON_POLICY_REPORT_FILE, tassadar_learned_horizon_policy_report_path,
    write_tassadar_learned_horizon_policy_report,
};

fn main() -> ExitCode {
    let output_path = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(tassadar_learned_horizon_policy_report_path);
    let output_path = if output_path.is_dir() {
        output_path.join(TASSADAR_LEARNED_HORIZON_POLICY_REPORT_FILE)
    } else {
        output_path
    };

    match write_tassadar_learned_horizon_policy_report(&output_path) {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("tassadar learned horizon policy report should serialize"),
            );
            if report.learned_article_class_bypass_allowed {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!(
                "failed to write Tassadar learned horizon policy report `{}`: {error}",
                output_path.display()
            );
            ExitCode::FAILURE
        }
    }
}
