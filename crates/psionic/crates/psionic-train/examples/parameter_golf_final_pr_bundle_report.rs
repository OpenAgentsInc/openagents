use std::{env, path::PathBuf};

use psionic_train::{
    write_parameter_golf_final_pr_bundle_report,
    ParameterGolfSubmissionChallengeExecutionPosture,
};

fn main() {
    let output_path = env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from("fixtures/parameter_golf/reports/parameter_golf_final_pr_bundle.json")
    });
    let report = write_parameter_golf_final_pr_bundle_report(
        &output_path,
        &ParameterGolfSubmissionChallengeExecutionPosture::local_review_host_defaults(),
    )
    .expect("write Parameter Golf final PR bundle report");
    println!(
        "wrote {} with record folder {}",
        output_path.display(),
        report.record_folder_relpath
    );
}
