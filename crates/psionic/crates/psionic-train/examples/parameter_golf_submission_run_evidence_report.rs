use psionic_train::{
    parameter_golf_submission_run_evidence_report_path,
    write_parameter_golf_submission_run_evidence_report,
    ParameterGolfSubmissionChallengeExecutionPosture,
};

fn main() {
    let path = parameter_golf_submission_run_evidence_report_path();
    let report = write_parameter_golf_submission_run_evidence_report(
        &path,
        &ParameterGolfSubmissionChallengeExecutionPosture::local_review_host_defaults(),
    )
    .expect("write Parameter Golf submission run evidence report");
    println!(
        "wrote {} with digest {}",
        path.display(),
        report.report_digest
    );
}
