use std::error::Error;

use psionic_train::{
    canonical_probe_gepa_stage_0_1_candidate_manifest,
    canonical_probe_gepa_terminal_bench_pylon_canary_import, import_probe_gepa_live_closeout,
    ProbeGepaCoordinatorState,
};

fn main() -> Result<(), Box<dyn Error>> {
    let candidate = canonical_probe_gepa_stage_0_1_candidate_manifest()?;
    let mut state = ProbeGepaCoordinatorState::default();
    let import = canonical_probe_gepa_terminal_bench_pylon_canary_import(&candidate);

    let receipt = import_probe_gepa_live_closeout(&mut state, &candidate, import)?;
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}
