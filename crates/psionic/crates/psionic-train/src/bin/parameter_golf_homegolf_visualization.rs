use std::{env, path::PathBuf};

use psionic_train::{
    write_parameter_golf_homegolf_visualization_artifacts_v2,
    PARAMETER_GOLF_HOMEGOLF_VISUALIZATION_BUNDLE_V2_FIXTURE_PATH,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut bundle_path = None;
    let mut run_index_path = None;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--bundle-output" => bundle_path = args.next().map(PathBuf::from),
            "--run-index-output" => run_index_path = args.next().map(PathBuf::from),
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            other => return Err(format!("unsupported argument `{other}`").into()),
        }
    }

    let bundle_path = bundle_path.unwrap_or_else(|| {
        PathBuf::from(PARAMETER_GOLF_HOMEGOLF_VISUALIZATION_BUNDLE_V2_FIXTURE_PATH)
    });
    let run_index_path = run_index_path.unwrap_or_else(|| {
        PathBuf::from("fixtures/training_visualization/remote_training_run_index_v2.json")
    });
    let (bundle, run_index) = write_parameter_golf_homegolf_visualization_artifacts_v2(
        bundle_path.as_path(),
        run_index_path.as_path(),
    )?;
    println!(
        "wrote bundle={} run_index={} score_metric={} promotion_gate={}",
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
    println!("run_index_entries={}", run_index.entries.len());
    Ok(())
}

fn print_usage() {
    eprintln!(
        "usage: cargo run -q -p psionic-train --bin parameter_golf_homegolf_visualization -- [--bundle-output <path>] [--run-index-output <path>]"
    );
}
