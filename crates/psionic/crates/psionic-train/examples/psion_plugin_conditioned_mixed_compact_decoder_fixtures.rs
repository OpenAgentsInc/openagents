use std::error::Error;

use psionic_train::{
    psion_plugin_conditioned_mixed_compact_decoder_reference_config_path,
    run_psion_plugin_mixed_reference_lane,
};

fn main() -> Result<(), Box<dyn Error>> {
    let bundle = run_psion_plugin_mixed_reference_lane()?;
    bundle.model_config.write_to_path(
        psion_plugin_conditioned_mixed_compact_decoder_reference_config_path(),
    )?;
    Ok(())
}
