use std::env;
use std::path::PathBuf;

use psionic_train::{
    write_parameter_golf_distributed_visualization_from_finalizer_report,
    RemoteTrainingResultClassification, RemoteTrainingSeriesStatus,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut report_path = None;
    let mut repo_revision = None;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--report" => report_path = args.next().map(PathBuf::from),
            "--repo-revision" => repo_revision = args.next(),
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => return Err(format!("unsupported argument `{other}`").into()),
        }
    }

    let report_path = report_path.ok_or("--report is required")?;
    let repo_revision = repo_revision.unwrap_or_else(|| String::from("workspace@unknown"));
    let outcome = write_parameter_golf_distributed_visualization_from_finalizer_report(
        report_path.as_path(),
        repo_revision,
    )?;
    println!(
        "wrote bundle={} run_index={} result={} series_status={}",
        outcome.paths.bundle_path.display(),
        outcome.paths.run_index_path.display(),
        format_result(outcome.bundle.result_classification),
        format_series_status(outcome.bundle.series_status),
    );
    Ok(())
}

fn print_usage() {
    eprintln!(
        "Usage: parameter_golf_distributed_visualization --report <path> [--repo-revision <revision>]"
    );
}

fn format_result(result: RemoteTrainingResultClassification) -> &'static str {
    match result {
        RemoteTrainingResultClassification::Planned => "planned",
        RemoteTrainingResultClassification::Active => "active",
        RemoteTrainingResultClassification::CompletedSuccess => "completed_success",
        RemoteTrainingResultClassification::CompletedFailure => "completed_failure",
        RemoteTrainingResultClassification::Refused => "refused",
        RemoteTrainingResultClassification::RehearsalOnly => "rehearsal_only",
    }
}

fn format_series_status(status: RemoteTrainingSeriesStatus) -> &'static str {
    match status {
        RemoteTrainingSeriesStatus::Available => "available",
        RemoteTrainingSeriesStatus::Partial => "partial",
        RemoteTrainingSeriesStatus::Unavailable => "unavailable",
    }
}
