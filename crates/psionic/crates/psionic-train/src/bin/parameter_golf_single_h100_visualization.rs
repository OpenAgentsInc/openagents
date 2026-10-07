use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use psionic_train::{
    build_remote_training_run_index, read_visualization_bundle,
    record_remote_training_visualization_bundle, write_visualization_artifacts_from_log_fallback,
    write_visualization_artifacts_from_report, ParameterGolfSingleH100TrainingReport,
    ParameterGolfSingleH100VisualizationMetadata, RemoteTrainingProvider,
    RemoteTrainingResultClassification, RemoteTrainingRunIndex, RemoteTrainingRunIndexEntry,
    RemoteTrainingVisualizationBundle,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut run_root = None;
    let mut training_report = None;
    let mut training_log = None;
    let mut provider = None;
    let mut profile_id = None;
    let mut lane_id = None;
    let mut repo_revision = None;
    let mut result_classification = None;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--run-root" => run_root = args.next().map(PathBuf::from),
            "--training-report" => training_report = args.next().map(PathBuf::from),
            "--training-log" => training_log = args.next().map(PathBuf::from),
            "--provider" => provider = args.next(),
            "--profile-id" => profile_id = args.next(),
            "--lane-id" => lane_id = args.next(),
            "--repo-revision" => repo_revision = args.next(),
            "--result-classification" => result_classification = args.next(),
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => {
                return Err(format!("unsupported argument `{other}`").into());
            }
        }
    }

    let run_root = run_root.ok_or("--run-root is required")?;
    let provider = parse_provider(provider.ok_or("--provider is required")?.as_str())?;
    let profile_id = profile_id.ok_or("--profile-id is required")?;
    let lane_id = lane_id.unwrap_or_else(|| String::from("parameter_golf.runpod_single_h100"));
    let repo_revision = repo_revision.unwrap_or_else(|| String::from("workspace@unknown"));
    let result_classification = parse_result(
        result_classification
            .unwrap_or_else(|| String::from("completed_success"))
            .as_str(),
    )?;
    let training_report = training_report
        .unwrap_or_else(|| run_root.join("parameter_golf_single_h100_training.json"));
    let training_log =
        training_log.unwrap_or_else(|| run_root.join("parameter_golf_single_h100_train.log"));
    let metadata = ParameterGolfSingleH100VisualizationMetadata::new(
        provider,
        profile_id,
        lane_id,
        repo_revision,
        run_root,
        training_report.clone(),
    );
    let existing_bundle = read_visualization_bundle(metadata.paths().bundle_path.as_path())?;

    let outcome = if training_report.is_file() {
        let report = read_report(training_report.as_path())?;
        write_visualization_artifacts_from_report(metadata, &report, existing_bundle.as_ref())?
    } else if let Some(existing_bundle) = existing_bundle {
        rewrite_existing_bundle(metadata, existing_bundle, result_classification)?
    } else if training_log.is_file() {
        write_visualization_artifacts_from_log_fallback(
            metadata,
            training_log.as_path(),
            result_classification,
        )?
    } else {
        return Err("no trainer report, existing live bundle, or trainer log was available".into());
    };

    println!(
        "wrote bundle={} run_index={} result={} series_status={}",
        outcome.paths.bundle_path.display(),
        outcome.paths.run_index_path.display(),
        format_result(outcome.bundle.result_classification),
        format_series_status(outcome.bundle.series_status),
    );
    Ok(())
}

fn rewrite_existing_bundle(
    metadata: ParameterGolfSingleH100VisualizationMetadata,
    mut bundle: RemoteTrainingVisualizationBundle,
    result_classification: RemoteTrainingResultClassification,
) -> Result<
    psionic_train::ParameterGolfSingleH100VisualizationWriteOutcome,
    Box<dyn std::error::Error>,
> {
    bundle.result_classification = result_classification;
    if result_classification != RemoteTrainingResultClassification::Active {
        bundle.timeline.push(psionic_train::RemoteTrainingTimelineEntry {
            observed_at_ms: now_ms(),
            phase: String::from("complete"),
            subphase: Some(String::from("finalizer_rewrite")),
            detail: String::from(
                "The finalizer retained the live bundle and rewrote the terminal result classification after the trainer report remained absent.",
            ),
        });
        bundle.event_series.push(psionic_train::RemoteTrainingEventSample {
            observed_at_ms: now_ms(),
            severity: psionic_train::RemoteTrainingEventSeverity::Warning,
            event_kind: String::from("terminal_rewrite_without_report"),
            detail: String::from(
                "The finalizer rewrote the live bundle terminal posture because the trainer report was still absent.",
            ),
        });
        bundle.summary.detail = String::from(
            "The live bundle stayed authoritative for current-series truth, but the finalizer had to seal terminal posture without the trainer report.",
        );
    }
    bundle = record_remote_training_visualization_bundle(bundle)?;
    let run_index = build_remote_training_run_index(RemoteTrainingRunIndex {
        schema_version: String::new(),
        index_id: format!("{}-remote-training-index-v1", bundle.run_id),
        generated_at_ms: now_ms(),
        entries: vec![RemoteTrainingRunIndexEntry {
            provider: bundle.provider,
            profile_id: bundle.profile_id.clone(),
            lane_id: bundle.lane_id.clone(),
            run_id: bundle.run_id.clone(),
            repo_revision: bundle.repo_revision.clone(),
            result_classification: bundle.result_classification,
            series_status: bundle.series_status,
            series_unavailable_reason: bundle.series_unavailable_reason.clone(),
            last_heartbeat_at_ms: bundle.refresh_contract.last_heartbeat_at_ms,
            bundle_artifact_uri: Some(metadata.bundle_relative_path()),
            bundle_digest: Some(bundle.bundle_digest.clone()),
            summary_label: format!(
                "{} {} single-H100",
                provider_label(bundle.provider),
                bundle.profile_id
            ),
            detail: bundle.summary.detail.clone(),
        }],
        detail: String::from(
            "The finalizer retained the existing single-H100 live bundle and sealed the current run posture.",
        ),
        index_digest: String::new(),
    })?;
    fs::create_dir_all(
        metadata
            .paths()
            .bundle_path
            .parent()
            .expect("bundle path should have a parent"),
    )?;
    fs::write(
        metadata.paths().bundle_path.as_path(),
        format!("{}\n", serde_json::to_string_pretty(&bundle)?),
    )?;
    fs::write(
        metadata.paths().run_index_path.as_path(),
        format!("{}\n", serde_json::to_string_pretty(&run_index)?),
    )?;
    Ok(
        psionic_train::ParameterGolfSingleH100VisualizationWriteOutcome {
            bundle,
            run_index,
            paths: metadata.paths(),
        },
    )
}

fn read_report(
    path: &Path,
) -> Result<ParameterGolfSingleH100TrainingReport, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn parse_provider(value: &str) -> Result<RemoteTrainingProvider, Box<dyn std::error::Error>> {
    match value {
        "google_cloud" => Ok(RemoteTrainingProvider::GoogleCloud),
        "run_pod" | "runpod" => Ok(RemoteTrainingProvider::RunPod),
        "local_hybrid" | "localhybrid" => Ok(RemoteTrainingProvider::LocalHybrid),
        other => Err(format!("unsupported provider `{other}`").into()),
    }
}

fn parse_result(
    value: &str,
) -> Result<RemoteTrainingResultClassification, Box<dyn std::error::Error>> {
    match value {
        "planned" => Ok(RemoteTrainingResultClassification::Planned),
        "active" => Ok(RemoteTrainingResultClassification::Active),
        "completed_success" => Ok(RemoteTrainingResultClassification::CompletedSuccess),
        "completed_failure" => Ok(RemoteTrainingResultClassification::CompletedFailure),
        "refused" => Ok(RemoteTrainingResultClassification::Refused),
        "rehearsal_only" => Ok(RemoteTrainingResultClassification::RehearsalOnly),
        other => Err(format!("unsupported result classification `{other}`").into()),
    }
}

fn provider_label(provider: RemoteTrainingProvider) -> &'static str {
    match provider {
        RemoteTrainingProvider::GoogleCloud => "google",
        RemoteTrainingProvider::RunPod => "runpod",
        RemoteTrainingProvider::LocalHybrid => "local_hybrid",
    }
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

fn format_series_status(status: psionic_train::RemoteTrainingSeriesStatus) -> &'static str {
    match status {
        psionic_train::RemoteTrainingSeriesStatus::Available => "available",
        psionic_train::RemoteTrainingSeriesStatus::Partial => "partial",
        psionic_train::RemoteTrainingSeriesStatus::Unavailable => "unavailable",
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis() as u64
}

fn print_usage() {
    eprintln!(
        "Usage: parameter_golf_single_h100_visualization --run-root <path> --provider <google_cloud|run_pod> --profile-id <id> [--lane-id <id>] [--repo-revision <rev>] [--training-report <path>] [--training-log <path>] [--result-classification <planned|active|completed_success|completed_failure|refused|rehearsal_only>]"
    );
}
