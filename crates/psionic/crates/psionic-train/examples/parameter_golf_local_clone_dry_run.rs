use std::{env, path::PathBuf};

use psionic_train::{
    ParameterGolfSubmissionChallengeExecutionPosture,
    write_parameter_golf_local_clone_dry_run_report,
};

fn main() {
    let parameter_golf_root = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(
                env::var("HOME")
                    .expect("HOME should exist for the default parameter-golf clone path"),
            )
            .join("code/parameter-golf")
        });
    let output_path = env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/parameter_golf_local_clone_dry_run.json"));
    let report = write_parameter_golf_local_clone_dry_run_report(
        &output_path,
        &parameter_golf_root,
        &ParameterGolfSubmissionChallengeExecutionPosture::local_review_host_defaults(),
    )
    .expect("write Parameter Golf local-clone dry-run report");
    println!(
        "wrote {} with verdict {:?}",
        output_path.display(),
        report.verdict
    );
}
