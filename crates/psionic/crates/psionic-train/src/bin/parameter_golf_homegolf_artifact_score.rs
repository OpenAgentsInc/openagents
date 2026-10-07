use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Instant,
};

use psionic_data::{
    load_parameter_golf_validation_tokens_from_paths, parameter_golf_dataset_bundle_from_local_dir,
    parameter_golf_sentencepiece_byte_luts_from_tokenizer_path,
};
use psionic_eval::evaluate_parameter_golf_validation;
use psionic_models::{
    ModelDescriptor, ParameterGolfConfig, ParameterGolfReferenceModel, ParameterGolfWeights,
};
use psionic_train::{
    restore_parameter_golf_model_from_quantized_artifact, ParameterGolfSingleH100ModelVariant,
    ParameterGolfSingleH100TrainingReport, ParameterGolfValidationEvalMode,
};
use serde::Serialize;

#[derive(Serialize)]
struct DetachedArtifactScoreReport {
    schema_version: String,
    training_report_path: String,
    artifact_path: String,
    run_id: String,
    machine_profile: String,
    model_variant: String,
    final_validation_mode: String,
    validation_eval_mode: String,
    sequence_length: usize,
    validation_batch_sequences: usize,
    batch_token_budget: usize,
    observed_eval_ms: u64,
    validation: psionic_eval::ParameterGolfValidationEvalReport,
    claim_boundary: String,
    summary: String,
}

struct ScoreCli {
    report_path: PathBuf,
    output_path: Option<PathBuf>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = parse_args(env::args().skip(1).collect::<Vec<_>>())?;
    let report: ParameterGolfSingleH100TrainingReport =
        serde_json::from_slice(&fs::read(&cli.report_path)?)?;
    if !report.training_executed() {
        return Err(format!(
            "training report {} did not execute training and cannot produce a detached score",
            cli.report_path.display()
        )
        .into());
    }
    if report.validation_eval_mode != ParameterGolfValidationEvalMode::NonOverlapping {
        return Err(format!(
            "detached HOMEGOLF artifact scoring currently supports validation_eval_mode=non_overlapping only, found {}",
            report.validation_eval_mode.as_str()
        )
        .into());
    }

    let artifact_path = report
        .compressed_model_artifact_path
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| default_artifact_path(cli.report_path.as_path()));
    if !artifact_path.is_file() {
        return Err(format!("artifact path {} does not exist", artifact_path.display()).into());
    }

    let baseline_model =
        baseline_model_from_variant(report.model_variant, report.model_config.clone())?;
    let artifact_bytes = fs::read(&artifact_path)?;
    let restored_model = restore_parameter_golf_model_from_quantized_artifact(
        &baseline_model,
        artifact_bytes.as_slice(),
    )?;

    let tokenizer_artifact_ref = report.tokenizer_path.display().to_string();
    let bundle = parameter_golf_dataset_bundle_from_local_dir(
        report.dataset_key.clone(),
        &report.dataset_root,
        report.variant.clone(),
        report.tokenizer_digest.clone(),
        tokenizer_artifact_ref,
        None,
    )?;
    let validation_paths = bundle
        .validation_shards
        .iter()
        .map(|receipt| PathBuf::from(receipt.path.clone()))
        .collect::<Vec<_>>();
    let validation_tokens = load_parameter_golf_validation_tokens_from_paths(
        validation_paths.as_slice(),
        report.geometry.train_sequence_length,
    )?;
    let byte_luts =
        parameter_golf_sentencepiece_byte_luts_from_tokenizer_path(&report.tokenizer_path)?;
    let batch_token_budget = report
        .geometry
        .train_sequence_length
        .saturating_mul(report.validation_batch_sequences);

    let started = Instant::now();
    let validation = evaluate_parameter_golf_validation(
        &restored_model,
        validation_tokens.as_slice(),
        report.geometry.train_sequence_length,
        batch_token_budget,
        &byte_luts,
    )?;
    let observed_eval_ms = started.elapsed().as_millis() as u64;

    let detached_report = DetachedArtifactScoreReport {
        schema_version: String::from("psionic.homegolf_detached_artifact_score.v1"),
        training_report_path: cli.report_path.display().to_string(),
        artifact_path: artifact_path.display().to_string(),
        run_id: report.run_id.clone(),
        machine_profile: report.machine_profile.as_str().to_string(),
        model_variant: report.model_variant.as_str().to_string(),
        final_validation_mode: report.final_validation_mode.as_str().to_string(),
        validation_eval_mode: report.validation_eval_mode.as_str().to_string(),
        sequence_length: report.geometry.train_sequence_length,
        validation_batch_sequences: report.validation_batch_sequences,
        batch_token_budget,
        observed_eval_ms,
        validation: validation.clone(),
        claim_boundary: String::from(
            "Detached HOMEGOLF artifact score receipt for exported local-challenge artifacts under non_overlapping validation semantics. This is an honest local score closeout surface, not an 8xH100 public leaderboard-equivalent record.",
        ),
        summary: format!(
            "Detached HOMEGOLF artifact scoring restored the exported quantized artifact from {} and measured non_overlapping validation at {:.8} bits/byte in {}ms.",
            artifact_path.display(),
            validation.bits_per_byte,
            observed_eval_ms
        ),
    };

    let encoded = serde_json::to_vec_pretty(&detached_report)?;
    if let Some(output_path) = cli.output_path {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output_path, &encoded)?;
        println!("{}", output_path.display());
    } else {
        println!("{}", String::from_utf8(encoded)?);
    }
    println!(
        "final_int8_zlib_roundtrip_exact val_loss:{:.8} val_bpb:{:.8}",
        validation.mean_loss, validation.bits_per_byte
    );
    Ok(())
}

fn baseline_model_from_variant(
    model_variant: ParameterGolfSingleH100ModelVariant,
    config: ParameterGolfConfig,
) -> Result<ParameterGolfReferenceModel, Box<dyn std::error::Error>> {
    let model_id = match model_variant {
        ParameterGolfSingleH100ModelVariant::BaselineSp1024_9x512 => {
            "parameter-golf-baseline-sp1024-9x512"
        }
        ParameterGolfSingleH100ModelVariant::CompetitiveHomegolfV1 => {
            "parameter-golf-homegolf-competitive-v1"
        }
    };
    let weights = ParameterGolfWeights::from_initializer(&config, Default::default())?;
    Ok(ParameterGolfReferenceModel::new(
        ModelDescriptor::new(model_id, "parameter_golf_decoder", "v1"),
        config,
        weights,
    )?)
}

fn default_artifact_path(report_path: &Path) -> PathBuf {
    let stem = report_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("parameter_golf_single_device_training");
    report_path.with_file_name(format!("{stem}.final_model.st"))
}

fn parse_args(args: Vec<String>) -> Result<ScoreCli, Box<dyn std::error::Error>> {
    if args.is_empty() {
        return Err(usage_error());
    }
    let mut report_path = None;
    let mut output_path = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--report" => {
                report_path = Some(PathBuf::from(
                    args.get(index + 1).ok_or_else(usage_error)?.clone(),
                ));
                index += 2;
            }
            "--output" => {
                output_path = Some(PathBuf::from(
                    args.get(index + 1).ok_or_else(usage_error)?.clone(),
                ));
                index += 2;
            }
            "--help" | "-h" => return Err(usage_error()),
            other if !other.starts_with("--") && report_path.is_none() => {
                report_path = Some(PathBuf::from(other));
                index += 1;
            }
            _ => return Err(usage_error()),
        }
    }
    Ok(ScoreCli {
        report_path: report_path.ok_or_else(usage_error)?,
        output_path,
    })
}

fn usage_error() -> Box<dyn std::error::Error> {
    "usage: parameter_golf_homegolf_artifact_score --report <training_report.json> [--output <path>]".into()
}
