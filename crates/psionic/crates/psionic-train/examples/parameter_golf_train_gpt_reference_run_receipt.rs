use std::{env, path::PathBuf};

use psionic_train::{
    write_parameter_golf_train_gpt_reference_run_receipt, ParameterGolfBatchGeometry,
    ParameterGolfTrainGptReferenceRunConfig,
};

fn main() {
    let mut run_id = None;
    let mut log_path = None;
    let mut output_path = None;
    let mut device_name = None;
    let mut dataset_manifest_digest = None;
    let mut tokenizer_digest = None;
    let mut world_size = None;
    let mut train_batch_tokens = None;
    let mut validation_batch_tokens = None;
    let mut train_sequence_length = None;
    let mut grad_accum_steps = None;
    let mut final_validation_observed_ms = None;
    let mut final_roundtrip_eval_ms = None;

    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--run-id" => run_id = Some(args.next().expect("missing value for --run-id")),
            "--log" => {
                log_path = Some(PathBuf::from(args.next().expect("missing value for --log")))
            }
            "--output" => {
                output_path = Some(PathBuf::from(
                    args.next().expect("missing value for --output"),
                ))
            }
            "--device-name" => {
                device_name = Some(args.next().expect("missing value for --device-name"))
            }
            "--dataset-manifest-digest" => {
                dataset_manifest_digest = Some(
                    args.next()
                        .expect("missing value for --dataset-manifest-digest"),
                )
            }
            "--tokenizer-digest" => {
                tokenizer_digest = Some(args.next().expect("missing value for --tokenizer-digest"))
            }
            "--world-size" => {
                world_size = Some(
                    args.next()
                        .expect("missing value for --world-size")
                        .parse()
                        .expect("invalid --world-size"),
                )
            }
            "--train-batch-tokens" => {
                train_batch_tokens = Some(
                    args.next()
                        .expect("missing value for --train-batch-tokens")
                        .parse()
                        .expect("invalid --train-batch-tokens"),
                )
            }
            "--validation-batch-tokens" => {
                validation_batch_tokens = Some(
                    args.next()
                        .expect("missing value for --validation-batch-tokens")
                        .parse()
                        .expect("invalid --validation-batch-tokens"),
                )
            }
            "--train-sequence-length" => {
                train_sequence_length = Some(
                    args.next()
                        .expect("missing value for --train-sequence-length")
                        .parse()
                        .expect("invalid --train-sequence-length"),
                )
            }
            "--grad-accum-steps" => {
                grad_accum_steps = Some(
                    args.next()
                        .expect("missing value for --grad-accum-steps")
                        .parse()
                        .expect("invalid --grad-accum-steps"),
                )
            }
            "--final-validation-ms" => {
                final_validation_observed_ms = Some(
                    args.next()
                        .expect("missing value for --final-validation-ms")
                        .parse()
                        .expect("invalid --final-validation-ms"),
                )
            }
            "--final-roundtrip-eval-ms" => {
                final_roundtrip_eval_ms = Some(
                    args.next()
                        .expect("missing value for --final-roundtrip-eval-ms")
                        .parse()
                        .expect("invalid --final-roundtrip-eval-ms"),
                )
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: parameter_golf_train_gpt_reference_run_receipt \\
  --run-id <id> --log <path> --output <path> --device-name <name> \\
  --dataset-manifest-digest <digest> --tokenizer-digest <digest> \\
  --world-size <n> --train-batch-tokens <n> --validation-batch-tokens <n> \\
  --train-sequence-length <n> --grad-accum-steps <n> \\
  [--final-validation-ms <ms>] [--final-roundtrip-eval-ms <ms>]"
                );
                std::process::exit(0);
            }
            value => panic!("unexpected argument `{value}`"),
        }
    }

    let config = ParameterGolfTrainGptReferenceRunConfig {
        run_id: run_id.expect("missing --run-id"),
        source_log_path: log_path.expect("missing --log"),
        device_name: device_name.expect("missing --device-name"),
        dataset_manifest_digest: dataset_manifest_digest
            .expect("missing --dataset-manifest-digest"),
        tokenizer_digest: tokenizer_digest.expect("missing --tokenizer-digest"),
        geometry: ParameterGolfBatchGeometry {
            world_size: world_size.expect("missing --world-size"),
            train_batch_tokens: train_batch_tokens.expect("missing --train-batch-tokens"),
            validation_batch_tokens: validation_batch_tokens
                .expect("missing --validation-batch-tokens"),
            train_sequence_length: train_sequence_length.expect("missing --train-sequence-length"),
            grad_accum_steps: grad_accum_steps.expect("missing --grad-accum-steps"),
        },
        final_validation_observed_ms,
        final_roundtrip_eval_ms,
        peak_memory_allocated_mib: None,
        peak_memory_reserved_mib: None,
    };

    let output_path = output_path.expect("missing --output");
    let receipt = write_parameter_golf_train_gpt_reference_run_receipt(&output_path, &config)
        .expect("write upstream train_gpt.py reference receipt");
    println!(
        "wrote {} ({})",
        output_path.display(),
        receipt.report_digest
    );
}
