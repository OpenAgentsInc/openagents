use std::error::Error;

use psionic_train::{
    psion_plugin_host_native_capability_matrix_path,
    psion_plugin_host_native_reference_run_bundle_path,
    psion_plugin_host_native_served_posture_path,
    record_psion_plugin_host_native_capability_matrix,
    record_psion_plugin_host_native_served_posture, PsionPluginHostNativeReferenceRunBundle,
};

fn main() -> Result<(), Box<dyn Error>> {
    let run_bundle: PsionPluginHostNativeReferenceRunBundle = serde_json::from_slice(
        &std::fs::read(psion_plugin_host_native_reference_run_bundle_path())?,
    )?;
    let matrix = record_psion_plugin_host_native_capability_matrix(&run_bundle)?;
    let posture = record_psion_plugin_host_native_served_posture(&matrix, &run_bundle)?;
    matrix.write_to_path(psion_plugin_host_native_capability_matrix_path())?;
    posture.write_to_path(psion_plugin_host_native_served_posture_path())?;
    Ok(())
}
