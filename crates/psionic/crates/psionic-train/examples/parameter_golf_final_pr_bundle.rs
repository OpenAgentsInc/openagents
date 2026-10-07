use std::{env, path::PathBuf};

use psionic_train::{
    ParameterGolfSubmissionChallengeExecutionPosture, write_parameter_golf_final_pr_bundle,
};

fn main() {
    let output_root = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/psionic_parameter_golf_final_pr_bundle"));
    let report = write_parameter_golf_final_pr_bundle(
        &output_root,
        &ParameterGolfSubmissionChallengeExecutionPosture::local_review_host_defaults(),
    )
    .expect("write Parameter Golf final PR bundle");
    println!(
        "wrote {} with record folder {}",
        output_root.display(),
        report.record_folder_relpath
    );
}
