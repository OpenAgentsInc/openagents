use std::error::Error;

use psionic_train::{
    psion_plugin_route_refusal_hardening_bundle_path,
    write_psion_plugin_route_refusal_hardening_bundle,
};

fn main() -> Result<(), Box<dyn Error>> {
    write_psion_plugin_route_refusal_hardening_bundle(
        psion_plugin_route_refusal_hardening_bundle_path(),
    )?;
    Ok(())
}
