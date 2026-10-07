use std::{env, path::PathBuf};

use psionic_train::{
    ParameterGolfLocalReferenceFixture, ParameterGolfNonRecordSubmissionConfig,
    ParameterGolfReferenceTrainingConfig, benchmark_parameter_golf_local_reference,
    canonicalize_parameter_golf_local_reference_benchmark_bundle,
    build_parameter_golf_non_record_submission_bundle,
    write_parameter_golf_non_record_submission_bundle,
};

fn main() {
    let output_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/psionic_parameter_golf_non_record_submission"));
    let fixture = ParameterGolfLocalReferenceFixture::reference().expect("load fixture");
    let config = ParameterGolfReferenceTrainingConfig::local_reference();
    let benchmark = canonicalize_parameter_golf_local_reference_benchmark_bundle(
        &benchmark_parameter_golf_local_reference(&fixture, &config)
            .expect("build local-reference benchmark bundle"),
    )
    .expect("canonicalize local-reference benchmark bundle");
    let submission = build_parameter_golf_non_record_submission_bundle(
        &benchmark,
        &ParameterGolfNonRecordSubmissionConfig::local_reference_defaults(),
    )
    .expect("build non-record submission bundle");
    write_parameter_golf_non_record_submission_bundle(&submission, &output_dir)
        .expect("write non-record submission bundle");
    println!(
        "wrote {} with submission_id {}",
        output_dir.display(),
        submission.package.submission_id
    );
}
