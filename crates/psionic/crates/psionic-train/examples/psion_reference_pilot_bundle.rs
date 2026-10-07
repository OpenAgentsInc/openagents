use std::{env, error::Error, fs, path::PathBuf};

use psionic_train::{run_psion_reference_pilot_evidence_bundle, PsionReferencePilotConfig};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let output_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/psion_reference_pilot_bundle"));
    fs::create_dir_all(&output_dir)?;

    let config = PsionReferencePilotConfig::reference()?;
    let bundle = run_psion_reference_pilot_evidence_bundle(root.as_path(), &config)?;
    bundle.write_to_dir(&output_dir)?;

    println!(
        "psion reference pilot bundle completed: bundle={} output={}",
        bundle.pilot_bundle.bundle_id,
        output_dir.display()
    );
    println!(
        "benchmark pass rates bps: architecture={} normative_specs={} held_out={} refusal={}",
        bundle.architecture_benchmark.aggregate_pass_rate_bps,
        bundle.normative_spec_benchmark.aggregate_pass_rate_bps,
        bundle.held_out_benchmark.aggregate_pass_rate_bps,
        bundle
            .refusal_calibration_receipt
            .aggregate_unsupported_request_refusal_bps
    );

    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve workspace root".into())
}
