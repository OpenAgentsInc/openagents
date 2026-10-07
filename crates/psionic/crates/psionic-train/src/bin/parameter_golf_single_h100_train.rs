use std::{env, path::PathBuf};

use psionic_train::{
    parameter_golf_default_validation_batch_sequences,
    write_parameter_golf_single_h100_training_report, ParameterGolfMatrixExecutionMode,
    ParameterGolfScoreFirstTttConfig, ParameterGolfSingleH100ModelVariant,
    ParameterGolfSingleH100TrainingConfig, ParameterGolfSingleH100ValidationMode,
    ParameterGolfValidationEvalMode,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().collect::<Vec<_>>();
    let dataset_root = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_dataset_root);
    let tokenizer_path = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(default_tokenizer_path);
    let output_path = args
        .get(3)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/parameter_golf_single_h100_training.json"));
    let selected_model_variant = env::var_os("PSIONIC_PARAMETER_GOLF_MODEL_VARIANT")
        .map(|value| {
            let raw = value.to_string_lossy();
            ParameterGolfSingleH100ModelVariant::parse(raw.as_ref())
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))
        })
        .transpose()?;
    let (max_steps, final_validation_mode, validation_eval_mode, score_first_ttt) =
        parse_optional_max_steps_and_validation_mode(&args[4..])?;
    let mut config = match (max_steps, selected_model_variant) {
        (Some(max_steps), Some(model_variant)) => {
            let mut config = ParameterGolfSingleH100TrainingConfig::bounded_proof_defaults(
                dataset_root,
                tokenizer_path,
                max_steps,
            );
            config.apply_model_variant(model_variant);
            config
        }
        (Some(max_steps), None) => ParameterGolfSingleH100TrainingConfig::bounded_proof_defaults(
            dataset_root,
            tokenizer_path,
            max_steps,
        ),
        (None, Some(ParameterGolfSingleH100ModelVariant::CompetitiveHomegolfV1)) => {
            ParameterGolfSingleH100TrainingConfig::challenge_competitive_homegolf_v1_defaults(
                dataset_root,
                tokenizer_path,
            )
        }
        (None, _) => {
            ParameterGolfSingleH100TrainingConfig::challenge_defaults(dataset_root, tokenizer_path)
        }
    };
    if let Some(final_validation_mode) = final_validation_mode {
        config.final_validation_mode = final_validation_mode;
    }
    if let Some(validation_eval_mode) = validation_eval_mode {
        config.validation_eval_mode = validation_eval_mode;
        config.validation_batch_sequences = parameter_golf_default_validation_batch_sequences(
            &config.geometry,
            &config.validation_eval_mode,
        );
    }
    if let Some(score_first_ttt) = score_first_ttt {
        config.score_first_ttt = Some(score_first_ttt);
    }
    if let Some(matrix_execution_mode) = env::var_os("PSIONIC_PARAMETER_GOLF_MATRIX_EXECUTION_MODE")
    {
        let raw = matrix_execution_mode.to_string_lossy();
        config.matrix_execution_mode = ParameterGolfMatrixExecutionMode::parse(raw.as_ref())
            .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidInput, message))?;
    }
    let report = write_parameter_golf_single_h100_training_report(&output_path, &config)?;
    println!(
        "wrote {} with disposition {:?} executed_steps={} stop_reason={:?}",
        output_path.display(),
        report.disposition,
        report.executed_steps,
        report.stop_reason,
    );
    println!(
        "warmup_steps={} completed_warmup_steps={} measured_training_time_ms={} validation_checkpoints={} final_validation_mode={} validation_eval_mode={} validation_batch_sequences={} matrix_execution_mode={}",
        report.warmup_steps,
        report.completed_warmup_steps,
        report.observed_training_time_ms,
        report.validation_checkpoints.len(),
        report.final_validation_mode.as_str(),
        report.validation_eval_mode.as_str(),
        report.validation_batch_sequences,
        report.matrix_execution_mode.as_str(),
    );
    if let Some(score_first_ttt) = report.score_first_ttt.as_ref() {
        println!(
            "score_first_ttt={} stride={} chunk_tokens={} epochs={} freeze_blocks={} batch_sequences={}",
            score_first_ttt.label(),
            score_first_ttt.stride,
            score_first_ttt.chunk_tokens,
            score_first_ttt.epochs,
            score_first_ttt.freeze_blocks,
            score_first_ttt.batch_sequences,
        );
    }
    if let Some(ref initial_validation) = report.initial_validation {
        println!(
            "initial_validation val_loss:{:.8} val_bpb:{:.8}",
            initial_validation.mean_loss, initial_validation.bits_per_byte
        );
    }
    if let Some(ref pre_export_final_validation) = report.pre_export_final_validation {
        println!(
            "pre_export_final_validation val_loss:{:.8} val_bpb:{:.8}",
            pre_export_final_validation.mean_loss, pre_export_final_validation.bits_per_byte
        );
    }
    for step in &report.step_metrics {
        println!(
            "train_step step:{} train_loss:{:.8} lr_mult:{:.8} muon_momentum:{:.8} windows:{}",
            step.global_step,
            step.mean_microbatch_loss,
            step.learning_rate_multiplier,
            step.muon_momentum,
            step.train_window_ids.join(",")
        );
    }
    if let Some(ref roundtrip_receipt) = report.final_roundtrip_receipt {
        println!(
            "{} val_loss:{:.4} val_bpb:{:.4} eval_time:{}ms",
            roundtrip_receipt.metric_source,
            roundtrip_receipt.validation.mean_loss,
            roundtrip_receipt.validation.bits_per_byte,
            roundtrip_receipt.observed_eval_ms,
        );
        println!(
            "{}_exact val_loss:{:.8} val_bpb:{:.8}",
            roundtrip_receipt.metric_source,
            roundtrip_receipt.validation.mean_loss,
            roundtrip_receipt.validation.bits_per_byte
        );
    } else if let Some(final_validation) = report.final_validation {
        println!(
            "final_validation val_loss:{:.8} val_bpb:{:.8}",
            final_validation.mean_loss, final_validation.bits_per_byte
        );
    }
    if let Some(bytes) = report.compressed_model_bytes {
        println!("compressed_model_bytes={bytes}");
    }
    if let Some(ref artifact_ref) = report.compressed_model_artifact_ref {
        println!("compressed_model_artifact_ref={artifact_ref}");
    }
    if let Some(ref artifact_digest) = report.compressed_model_artifact_digest {
        println!("compressed_model_artifact_digest={artifact_digest}");
    }
    if let Some(refusal) = report.refusal {
        println!(
            "refusal subject={:?} detail={}",
            refusal.subject, refusal.detail
        );
    }
    Ok(())
}

fn default_dataset_root() -> PathBuf {
    PathBuf::from(
        env::var("HOME").expect("HOME should exist for the default parameter-golf dataset root"),
    )
    .join("code/parameter-golf/data/datasets/fineweb10B_sp1024")
}

fn default_tokenizer_path() -> PathBuf {
    PathBuf::from(
        env::var("HOME").expect("HOME should exist for the default parameter-golf tokenizer path"),
    )
    .join("code/parameter-golf/data/tokenizers/fineweb_1024_bpe.model")
}

fn parse_optional_max_steps_and_validation_mode(
    args: &[String],
) -> Result<
    (
        Option<u64>,
        Option<ParameterGolfSingleH100ValidationMode>,
        Option<ParameterGolfValidationEvalMode>,
        Option<ParameterGolfScoreFirstTttConfig>,
    ),
    Box<dyn std::error::Error>,
> {
    let Some(first) = args.first().map(String::as_str) else {
        return Ok((None, None, None, None));
    };
    if let Ok(max_steps) = first.parse::<u64>() {
        let validation_mode = args
            .get(1)
            .map(String::as_str)
            .map(ParameterGolfSingleH100ValidationMode::parse)
            .transpose();
        if let Ok(validation_mode) = validation_mode {
            let validation_eval_mode = args
                .get(2)
                .map(String::as_str)
                .map(ParameterGolfValidationEvalMode::parse)
                .transpose()?;
            let score_first_ttt = args
                .get(3)
                .map(String::as_str)
                .map(ParameterGolfScoreFirstTttConfig::parse)
                .transpose()?;
            return Ok((
                Some(max_steps),
                validation_mode,
                validation_eval_mode,
                score_first_ttt,
            ));
        }
        let validation_eval_mode = ParameterGolfValidationEvalMode::parse(
            args.get(1).map(String::as_str).unwrap_or_default(),
        )?;
        let score_first_ttt = args
            .get(2)
            .map(String::as_str)
            .map(ParameterGolfScoreFirstTttConfig::parse)
            .transpose()?;
        return Ok((
            Some(max_steps),
            None,
            Some(validation_eval_mode),
            score_first_ttt,
        ));
    }
    if let Ok(validation_mode) = ParameterGolfSingleH100ValidationMode::parse(first) {
        let validation_eval_mode = args
            .get(1)
            .map(String::as_str)
            .map(ParameterGolfValidationEvalMode::parse)
            .transpose()?;
        let score_first_ttt = args
            .get(2)
            .map(String::as_str)
            .map(ParameterGolfScoreFirstTttConfig::parse)
            .transpose()?;
        return Ok((
            None,
            Some(validation_mode),
            validation_eval_mode,
            score_first_ttt,
        ));
    }
    let validation_eval_mode = ParameterGolfValidationEvalMode::parse(first)?;
    let score_first_ttt = args
        .get(1)
        .map(String::as_str)
        .map(ParameterGolfScoreFirstTttConfig::parse)
        .transpose()?;
    Ok((None, None, Some(validation_eval_mode), score_first_ttt))
}
