use std::{
    env,
    path::{Path, PathBuf},
    process::ExitCode,
};

use psionic_train::{
    TassadarTrainLauncherPhase, builtin_tassadar_train_launcher_lane_specs,
    read_tassadar_train_retained_summary, write_tassadar_train_launcher_outputs,
};

fn main() -> ExitCode {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut args = env::args().skip(1);
    let mut command = String::from("start");
    let mut lane_id = None;
    let mut run_root = None::<PathBuf>;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => return print_help(workspace_root.as_path()),
            "start" | "dry-run" | "status" => command = arg,
            "--lane" => lane_id = args.next(),
            "--run-root" => run_root = args.next().map(PathBuf::from),
            other => {
                eprintln!("unsupported argument `{other}`");
                return ExitCode::FAILURE;
            }
        }
    }

    match command.as_str() {
        "start" => materialize(
            workspace_root.as_path(),
            TassadarTrainLauncherPhase::LaunchStaged,
            lane_id.as_deref(),
            run_root.unwrap_or_else(|| {
                workspace_root
                    .join("tassadar_operator_runs")
                    .join("run-tassadar-20260402t200000z")
            }),
        ),
        "dry-run" => materialize(
            workspace_root.as_path(),
            TassadarTrainLauncherPhase::DryRunPlanned,
            lane_id.as_deref(),
            run_root.unwrap_or_else(|| {
                workspace_root
                    .join("tassadar_operator_runs")
                    .join("run-tassadar-20260402t200000z")
            }),
        ),
        "status" => match run_root {
            Some(run_root) => match read_tassadar_train_retained_summary(run_root.as_path()) {
                Ok(summary) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&summary)
                            .expect("retained summary should serialize"),
                    );
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("failed to read Tassadar retained summary: {error}");
                    ExitCode::FAILURE
                }
            },
            None => {
                eprintln!("status requires --run-root");
                ExitCode::FAILURE
            }
        },
        other => {
            eprintln!("unsupported command `{other}`");
            ExitCode::FAILURE
        }
    }
}

fn materialize(
    workspace_root: &Path,
    phase: TassadarTrainLauncherPhase,
    lane_id: Option<&str>,
    run_root: PathBuf,
) -> ExitCode {
    match write_tassadar_train_launcher_outputs(workspace_root, phase, lane_id, run_root.as_path())
    {
        Ok(output) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&output.retained_summary)
                    .expect("retained summary should serialize"),
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to materialize Tassadar launcher output: {error}");
            ExitCode::FAILURE
        }
    }
}

fn print_help(workspace_root: &Path) -> ExitCode {
    let specs = match builtin_tassadar_train_launcher_lane_specs(workspace_root) {
        Ok(specs) => specs,
        Err(error) => {
            eprintln!("failed to load Tassadar launcher lane specs: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "Usage: ./TRAIN_TASSADAR [start|dry-run|status] [--lane <lane_id>] [--run-root <path>]"
    );
    println!();
    println!("Supported lanes:");
    for spec in specs {
        println!(
            "- {}{}: {}",
            spec.lane_id,
            if spec.is_default { " (default)" } else { "" },
            spec.checker_command
        );
    }
    ExitCode::SUCCESS
}
