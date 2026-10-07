use std::{env, path::PathBuf};

use psionic_train::write_parameter_golf_homegolf_strict_challenge_lane_report;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().collect::<Vec<_>>();
    let output_path = args.get(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from("/tmp/parameter_golf_homegolf_strict_challenge_lane.json")
    });
    let dataset_root = args.get(2).map(PathBuf::from);
    let tokenizer_path = args.get(3).map(PathBuf::from);
    let report = write_parameter_golf_homegolf_strict_challenge_lane_report(
        output_path.as_path(),
        dataset_root.as_deref(),
        tokenizer_path.as_deref(),
    )?;
    println!(
        "wrote {} disposition={:?} profile_id={} contest_bpb_required={} artifact_cap_required={}",
        output_path.display(),
        report.disposition,
        report.strict_profile.profile_id,
        report
            .strict_profile
            .evaluation_policy
            .contest_bits_per_byte_accounting_required,
        report
            .strict_profile
            .artifact_policy
            .exact_compressed_artifact_cap_required,
    );
    if let Some(refusal) = report.refusal.as_ref() {
        println!(
            "refusal subject={:?} detail={}",
            refusal.subject, refusal.detail
        );
    }
    Ok(())
}
