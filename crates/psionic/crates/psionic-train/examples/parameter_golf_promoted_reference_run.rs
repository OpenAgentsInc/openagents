use std::{env, path::PathBuf};

use psionic_train::{
    run_parameter_golf_promoted_reference_run, write_parameter_golf_promoted_reference_run,
    ParameterGolfLocalReferenceFixture, ParameterGolfReferenceTrainingConfig,
};

fn main() {
    let output_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/psionic_parameter_golf_promoted_reference_run"));
    let fixture = ParameterGolfLocalReferenceFixture::reference().expect("load fixture");
    let config = ParameterGolfReferenceTrainingConfig::promoted_general_small_decoder();
    let run = run_parameter_golf_promoted_reference_run(&fixture, &config)
        .expect("run promoted PGOLF reference proof");
    write_parameter_golf_promoted_reference_run(&run, &output_dir)
        .expect("write promoted PGOLF proof directory");
    println!(
        "parameter golf promoted reference run completed: profile={} checkpoint={} output={}",
        run.profile_contract.profile_id,
        run.summary.final_checkpoint_ref,
        output_dir.display()
    );
}
