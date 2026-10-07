use std::{env, path::PathBuf};

use psionic_train::{
    write_parameter_golf_xtrain_visualization_artifacts_v2,
    PARAMETER_GOLF_XTRAIN_QUICK_EVAL_REPORT_FIXTURE_PATH,
    PARAMETER_GOLF_XTRAIN_VISUALIZATION_BUNDLE_V2_FIXTURE_PATH,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut report_path = None;
    let mut bundle_path = None;
    let mut run_index_path = None;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--report-output" => report_path = args.next().map(PathBuf::from),
            "--bundle-output" => bundle_path = args.next().map(PathBuf::from),
            "--run-index-output" => run_index_path = args.next().map(PathBuf::from),
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => return Err(format!("unsupported argument `{other}`").into()),
        }
    }

    let report_path = report_path
        .unwrap_or_else(|| PathBuf::from(PARAMETER_GOLF_XTRAIN_QUICK_EVAL_REPORT_FIXTURE_PATH));
    let bundle_path = bundle_path.unwrap_or_else(|| {
        PathBuf::from(PARAMETER_GOLF_XTRAIN_VISUALIZATION_BUNDLE_V2_FIXTURE_PATH)
    });
    let run_index_path = run_index_path.unwrap_or_else(|| {
        PathBuf::from("fixtures/training_visualization/remote_training_run_index_v2.json")
    });
    let (report, bundle, run_index) = write_parameter_golf_xtrain_visualization_artifacts_v2(
        report_path.as_path(),
        bundle_path.as_path(),
        run_index_path.as_path(),
    )?;
    println!(
        "wrote report={} bundle={} run_index={} score_metric={} promotion_gate={}",
        report_path.display(),
        bundle_path.display(),
        run_index_path.display(),
        bundle
            .primary_score
            .as_ref()
            .map(|score| score.score_metric_id.as_str())
            .unwrap_or("none"),
        bundle
            .score_surface
            .as_ref()
            .map(|surface| format!("{:?}", surface.promotion_gate_posture))
            .unwrap_or_else(|| String::from("none")),
    );
    println!(
        "report_digest={} run_index_entries={}",
        report.report_digest,
        run_index.entries.len()
    );
    Ok(())
}

fn print_usage() {
    eprintln!(
        "usage: cargo run -q -p psionic-train --bin parameter_golf_xtrain_visualization -- [--report-output <path>] [--bundle-output <path>] [--run-index-output <path>]"
    );
}
