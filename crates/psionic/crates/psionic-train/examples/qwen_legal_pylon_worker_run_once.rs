use std::{env, process::ExitCode};

use psionic_train::{PylonTrainingWorkerJobStatus, run_qwen_legal_pylon_worker_job_path};

fn main() -> ExitCode {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let Some(job_path) = parse_job_path(args.as_slice()) else {
        eprintln!("usage: qwen_legal_pylon_worker_run_once --job <job.json>");
        return ExitCode::from(2);
    };
    match run_qwen_legal_pylon_worker_job_path(job_path) {
        Ok(receipt) => {
            match serde_json::to_string_pretty(&receipt) {
                Ok(json) => println!("{json}"),
                Err(error) => {
                    eprintln!("failed to serialize worker receipt: {error}");
                    return ExitCode::from(1);
                }
            }
            if receipt.status == PylonTrainingWorkerJobStatus::Succeeded {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(3)
            }
        }
        Err(error) => {
            eprintln!("failed to run Pylon worker job: {error}");
            ExitCode::from(1)
        }
    }
}

fn parse_job_path(args: &[String]) -> Option<&str> {
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--job" => return args.get(index + 1).map(String::as_str),
            "--help" | "-h" => return None,
            _ => index += 1,
        }
    }
    None
}
