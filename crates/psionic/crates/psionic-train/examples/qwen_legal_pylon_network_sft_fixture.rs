use std::{env, process::ExitCode};

use psionic_train::write_canonical_qwen_legal_pylon_network_sft_fixture;

fn main() -> ExitCode {
    let output_dir = env::args()
        .nth(1)
        .unwrap_or_else(|| String::from("fixtures/qwen_legal/pylon_network_sft"));
    match write_canonical_qwen_legal_pylon_network_sft_fixture(&output_dir) {
        Ok(report) => {
            println!(
                "wrote Qwen legal Pylon network SFT report {} with aggregate {}",
                report.report_id, report.aggregate.aggregate_adapter_artifact_digest
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to write Qwen legal Pylon network SFT fixture: {error}");
            ExitCode::FAILURE
        }
    }
}
