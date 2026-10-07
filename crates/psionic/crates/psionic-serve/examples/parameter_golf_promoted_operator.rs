use std::{env, fs, path::PathBuf};

use psionic_models::ParameterGolfPromotedRuntimeBundle;
use psionic_serve::{
    CpuPromotedParameterGolfTextGenerationService, GenerationOptions, GenerationRequest,
    TextGenerationExecutor,
};
use psionic_train::{
    ParameterGolfPromotedProfileAssumption,
    build_parameter_golf_promoted_inference_promotion_receipt,
    inspect_parameter_golf_promoted_bundle,
};
use serde::Serialize;

#[derive(Serialize)]
struct PromptReport {
    prompt: String,
    mode: String,
    profile_id: String,
    profile_kind: String,
    receipt_digest: String,
    prompt_tokens: usize,
    output_tokens: usize,
    termination: String,
    output_text: String,
}

#[derive(Serialize)]
struct WarmReport {
    profile_id: String,
    profile_kind: String,
    receipt_digest: String,
    model_id: String,
    session_id: String,
    prompt: String,
    output_text: String,
    output_tokens: usize,
    cache_tokens: usize,
    termination: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or_else(|| usage_error())?;
    let bundle_dir = args.next().ok_or_else(|| usage_error())?;
    let bundle_dir = PathBuf::from(bundle_dir);

    match command.as_str() {
        "inspect" => {
            let inspection = inspect_parameter_golf_promoted_bundle(bundle_dir.as_path())?;
            println!("{}", serde_json::to_string_pretty(&inspection)?);
            Ok(())
        }
        "validate" => {
            let (assumption, output_path) = parse_validate_args(args.collect::<Vec<_>>())?;
            let receipt = build_parameter_golf_promoted_inference_promotion_receipt(
                bundle_dir.as_path(),
                assumption,
            )?;
            write_or_print_json(&receipt, output_path)?;
            exit_on_refused(&receipt)
        }
        "prompt" => {
            let prompt_args = parse_prompt_args(args.collect::<Vec<_>>())?;
            let receipt = build_parameter_golf_promoted_inference_promotion_receipt(
                bundle_dir.as_path(),
                prompt_args.assumption,
            )?;
            exit_on_refused(&receipt)?;
            let bundle = ParameterGolfPromotedRuntimeBundle::load_dir(bundle_dir.as_path())?;
            let mut options = if prompt_args.mode == "sample" {
                bundle.default_seeded_sampling_options(prompt_args.seed)
            } else {
                bundle.default_greedy_generation_options()
            };
            options.max_new_tokens = prompt_args.max_new_tokens;
            let output = bundle.generate_text(prompt_args.prompt.as_str(), &options)?;
            let report = PromptReport {
                prompt: prompt_args.prompt,
                mode: prompt_args.mode,
                profile_id: bundle.manifest().profile_id.clone(),
                profile_kind: bundle.manifest().profile_kind.clone(),
                receipt_digest: receipt.receipt_digest,
                prompt_tokens: output.prompt_tokens.len(),
                output_tokens: output.generated_tokens.len(),
                termination: format!("{:?}", output.termination),
                output_text: output.text,
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
            Ok(())
        }
        "warm" => {
            let warm_args = parse_warm_args(args.collect::<Vec<_>>())?;
            let receipt = build_parameter_golf_promoted_inference_promotion_receipt(
                bundle_dir.as_path(),
                warm_args.assumption,
            )?;
            exit_on_refused(&receipt)?;
            let mut service = CpuPromotedParameterGolfTextGenerationService::from_bundle_dir(
                bundle_dir.as_path(),
            )?;
            let model_id = service.model_descriptor().model.model_id.clone();
            let session = service.create_session(model_id.as_str())?;
            let request = GenerationRequest::new_text(
                "promoted-pgolf-warm",
                service.model_descriptor().clone(),
                Some(session.session_id.clone()),
                warm_args.prompt.as_str(),
                GenerationOptions::greedy(warm_args.max_new_tokens),
            );
            let response = service.generate(&request)?;
            let report = WarmReport {
                profile_id: receipt.profile_id,
                profile_kind: receipt.profile_kind,
                receipt_digest: receipt.receipt_digest,
                model_id,
                session_id: session.session_id.0.clone(),
                prompt: warm_args.prompt,
                output_text: response.output.text,
                output_tokens: response.usage.output_tokens,
                cache_tokens: response.usage.cache_tokens,
                termination: format!("{:?}", response.termination),
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
            Ok(())
        }
        _ => Err(usage_error()),
    }
}

fn parse_validate_args(
    args: Vec<String>,
) -> Result<(ParameterGolfPromotedProfileAssumption, Option<PathBuf>), Box<dyn std::error::Error>> {
    let mut assumption = ParameterGolfPromotedProfileAssumption::BundleDeclaredProfile;
    let mut output_path = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--assume" => {
                index += 1;
                assumption = parse_assumption(args.get(index).ok_or_else(|| usage_error())?)?;
            }
            "--output" => {
                index += 1;
                output_path = Some(PathBuf::from(args.get(index).ok_or_else(|| usage_error())?));
            }
            _ => return Err(usage_error()),
        }
        index += 1;
    }
    Ok((assumption, output_path))
}

struct PromptArgs {
    prompt: String,
    max_new_tokens: usize,
    mode: String,
    seed: u64,
    assumption: ParameterGolfPromotedProfileAssumption,
}

fn parse_prompt_args(args: Vec<String>) -> Result<PromptArgs, Box<dyn std::error::Error>> {
    let mut prompt = String::from("abcd");
    let mut max_new_tokens = 4;
    let mut mode = String::from("greedy");
    let mut seed = 42_u64;
    let mut assumption = ParameterGolfPromotedProfileAssumption::BundleDeclaredProfile;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--prompt" => {
                index += 1;
                prompt = args.get(index).ok_or_else(|| usage_error())?.clone();
            }
            "--max-new-tokens" => {
                index += 1;
                max_new_tokens = args.get(index).ok_or_else(|| usage_error())?.parse()?;
            }
            "--mode" => {
                index += 1;
                mode = args.get(index).ok_or_else(|| usage_error())?.clone();
            }
            "--seed" => {
                index += 1;
                seed = args.get(index).ok_or_else(|| usage_error())?.parse()?;
            }
            "--assume" => {
                index += 1;
                assumption = parse_assumption(args.get(index).ok_or_else(|| usage_error())?)?;
            }
            _ => return Err(usage_error()),
        }
        index += 1;
    }
    Ok(PromptArgs {
        prompt,
        max_new_tokens,
        mode,
        seed,
        assumption,
    })
}

struct WarmArgs {
    prompt: String,
    max_new_tokens: usize,
    assumption: ParameterGolfPromotedProfileAssumption,
}

fn parse_warm_args(args: Vec<String>) -> Result<WarmArgs, Box<dyn std::error::Error>> {
    let mut prompt = String::from("abcd");
    let mut max_new_tokens = 2;
    let mut assumption = ParameterGolfPromotedProfileAssumption::BundleDeclaredProfile;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--prompt" => {
                index += 1;
                prompt = args.get(index).ok_or_else(|| usage_error())?.clone();
            }
            "--max-new-tokens" => {
                index += 1;
                max_new_tokens = args.get(index).ok_or_else(|| usage_error())?.parse()?;
            }
            "--assume" => {
                index += 1;
                assumption = parse_assumption(args.get(index).ok_or_else(|| usage_error())?)?;
            }
            _ => return Err(usage_error()),
        }
        index += 1;
    }
    Ok(WarmArgs {
        prompt,
        max_new_tokens,
        assumption,
    })
}

fn parse_assumption(
    value: &str,
) -> Result<ParameterGolfPromotedProfileAssumption, Box<dyn std::error::Error>> {
    match value {
        "bundle" | "bundle_declared_profile" => {
            Ok(ParameterGolfPromotedProfileAssumption::BundleDeclaredProfile)
        }
        "general" | "general_psion_small_decoder" => {
            Ok(ParameterGolfPromotedProfileAssumption::GeneralPsionSmallDecoder)
        }
        "strict" | "strict_pgolf_challenge" => {
            Ok(ParameterGolfPromotedProfileAssumption::StrictPgolfChallenge)
        }
        _ => Err(usage_error()),
    }
}

fn exit_on_refused(
    receipt: &psionic_train::ParameterGolfPromotedInferencePromotionReceipt,
) -> Result<(), Box<dyn std::error::Error>> {
    if receipt.disposition
        == psionic_train::ParameterGolfPromotedInferencePromotionDisposition::Refused
    {
        println!("{}", serde_json::to_string_pretty(receipt)?);
        std::process::exit(2);
    }
    Ok(())
}

fn write_or_print_json<T: Serialize>(
    value: &T,
    output_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let encoded = serde_json::to_vec_pretty(value)?;
    if let Some(output_path) = output_path {
        fs::write(output_path, encoded)?;
    } else {
        println!("{}", String::from_utf8(encoded)?);
    }
    Ok(())
}

fn usage_error() -> Box<dyn std::error::Error> {
    String::from(
        "usage:\n  cargo run -p psionic-serve --example parameter_golf_promoted_operator -- inspect <bundle_dir>\n  cargo run -p psionic-serve --example parameter_golf_promoted_operator -- validate <bundle_dir> [--assume bundle|general|strict] [--output <path>]\n  cargo run -p psionic-serve --example parameter_golf_promoted_operator -- prompt <bundle_dir> [--assume bundle|general|strict] [--prompt <text>] [--max-new-tokens <n>] [--mode greedy|sample] [--seed <u64>]\n  cargo run -p psionic-serve --example parameter_golf_promoted_operator -- warm <bundle_dir> [--assume bundle|general|strict] [--prompt <text>] [--max-new-tokens <n>]",
    )
    .into()
}
