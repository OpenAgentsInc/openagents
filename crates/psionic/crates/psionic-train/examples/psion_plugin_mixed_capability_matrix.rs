use std::error::Error;

use psionic_train::{
    build_psion_plugin_guest_plugin_benchmark_bundle,
    psion_plugin_mixed_capability_matrix_path, psion_plugin_mixed_reference_run_bundle_path,
    psion_plugin_mixed_served_posture_path, record_psion_plugin_mixed_capability_matrix,
    record_psion_plugin_mixed_served_posture, PsionPluginMixedReferenceRunBundle,
};

fn main() -> Result<(), Box<dyn Error>> {
    let run_bundle: PsionPluginMixedReferenceRunBundle = serde_json::from_slice(&std::fs::read(
        psion_plugin_mixed_reference_run_bundle_path(),
    )?)?;
    let guest_benchmark_bundle = build_psion_plugin_guest_plugin_benchmark_bundle()?;
    let matrix = record_psion_plugin_mixed_capability_matrix(&run_bundle, &guest_benchmark_bundle)?;
    let posture =
        record_psion_plugin_mixed_served_posture(&matrix, &run_bundle, &guest_benchmark_bundle)?;
    matrix.write_to_path(psion_plugin_mixed_capability_matrix_path())?;
    posture.write_to_path(psion_plugin_mixed_served_posture_path())?;
    Ok(())
}
