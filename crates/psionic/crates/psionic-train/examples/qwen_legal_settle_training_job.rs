use std::{env, process::ExitCode};

use psionic_train::{settle_qwen_legal_pylon_training_job, PylonTrainingPaymentStatus};

fn main() -> ExitCode {
    let Some(job_id) = env::args().nth(1) else {
        eprintln!("usage: qwen_legal_settle_training_job <job-id>");
        return ExitCode::from(2);
    };
    match settle_qwen_legal_pylon_training_job(job_id.as_str()) {
        Ok(decision) => {
            match serde_json::to_string_pretty(&decision) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("failed to serialize payment decision: {error}");
                    return ExitCode::from(1);
                }
            }
            match decision.payment_status {
                PylonTrainingPaymentStatus::Payable | PylonTrainingPaymentStatus::Paid => {
                    ExitCode::SUCCESS
                }
                PylonTrainingPaymentStatus::PendingValidation
                | PylonTrainingPaymentStatus::Withheld => ExitCode::SUCCESS,
            }
        }
        Err(error) => {
            eprintln!("failed to settle Pylon training job: {error}");
            ExitCode::from(1)
        }
    }
}
