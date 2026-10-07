use std::error::Error;

use psionic_train::{
    psion_plugin_host_native_reference_run_bundle_path, run_psion_plugin_host_native_reference_lane,
};

fn main() -> Result<(), Box<dyn Error>> {
    let bundle = run_psion_plugin_host_native_reference_lane()?;
    bundle.write_to_path(psion_plugin_host_native_reference_run_bundle_path())?;
    Ok(())
}
