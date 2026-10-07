use std::{env, fs, path::PathBuf};

use psionic_train::{
    build_parameter_golf_submission_run_evidence_report_with_distributed_receipt,
    ParameterGolfSubmissionChallengeExecutionPosture,
};
use psionic_eval::ParameterGolfDistributedThroughputReceipt;

fn main() {
    let mut submission_dir = None;
    let mut output_path = None;
    let mut distributed_receipt_path = None;
    let mut posture =
        ParameterGolfSubmissionChallengeExecutionPosture::local_review_host_defaults();
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--posture" => {
                let posture_id = args.next().expect("missing value for --posture");
                posture = match posture_id.as_str() {
                    "local_review_host" => {
                        ParameterGolfSubmissionChallengeExecutionPosture::local_review_host_defaults(
                        )
                    }
                    "runpod_8xh100" => {
                        ParameterGolfSubmissionChallengeExecutionPosture::runpod_8xh100_defaults()
                    }
                    _ => panic!("unsupported posture `{posture_id}`"),
                };
            }
            "--distributed-receipt" => {
                distributed_receipt_path =
                    Some(PathBuf::from(args.next().expect("missing value for --distributed-receipt")));
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: parameter_golf_submission_run_evidence [submission_dir] [output_path] [--posture local_review_host|runpod_8xh100] [--distributed-receipt path]"
                );
                std::process::exit(0);
            }
            value => {
                if submission_dir.is_none() {
                    submission_dir = Some(PathBuf::from(value));
                } else if output_path.is_none() {
                    output_path = Some(PathBuf::from(value));
                } else {
                    panic!("unexpected extra argument `{value}`");
                }
            }
        }
    }
    let submission_dir = submission_dir
        .unwrap_or_else(|| std::env::current_dir().expect("resolve current directory"));
    let distributed_receipt = distributed_receipt_path.as_ref().map(|path| {
        serde_json::from_slice::<ParameterGolfDistributedThroughputReceipt>(
            &fs::read(path).expect("read distributed receipt"),
        )
        .expect("decode distributed receipt")
    });
    let report = build_parameter_golf_submission_run_evidence_report_with_distributed_receipt(
        &submission_dir,
        &posture,
        distributed_receipt.as_ref(),
    )
    .expect("build Parameter Golf submission run evidence report");
    let json = serde_json::to_string_pretty(&report).expect("serialize run evidence report");
    if let Some(path) = output_path {
        fs::write(&path, format!("{json}\n")).expect("write run evidence report");
        println!("wrote {}", path.display());
    } else {
        println!("{json}");
    }
}
