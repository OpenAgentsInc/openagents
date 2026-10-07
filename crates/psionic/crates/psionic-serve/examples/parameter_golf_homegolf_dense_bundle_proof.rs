use std::{
    env, fs,
    path::{Path, PathBuf},
};

use psionic_models::{
    ParameterGolfConfig, ParameterGolfPromotedRuntimeBundle, TokenId, TokenSequence,
    TokenizerBoundary,
};
use psionic_serve::{
    CpuPromotedParameterGolfTextGenerationService, GenerationOptions, GenerationRequest,
    TextGenerationExecutor,
};
use psionic_train::{
    build_parameter_golf_promoted_inference_promotion_receipt,
    run_parameter_golf_promoted_reference_run, write_parameter_golf_promoted_reference_run,
    ParameterGolfLocalReferenceFixture, ParameterGolfPromotedProfileAssumption,
    ParameterGolfReferenceTrainingConfig,
};
use serde::Serialize;

const REPORT_SCHEMA_VERSION: &str = "psionic.parameter_golf_homegolf_dense_bundle_proof.v1";
const REPORT_FILENAME: &str = "parameter_golf_homegolf_dense_bundle_proof.json";
const SOURCE_DENSE_BASELINE_SURFACE_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_dense_baseline_surface.json";

#[derive(Clone, Debug, Serialize)]
struct HomegolfDenseBundleProofReport {
    schema_version: String,
    report_id: String,
    track_id: String,
    source_dense_baseline_surface_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    evidence_seed_slot: Option<u32>,
    run_id: String,
    profile_id: String,
    baseline_model_id: String,
    baseline_revision: String,
    descriptor_digest: String,
    tokenizer_digest: String,
    model_artifact_bytes: u64,
    final_validation_mean_loss: f64,
    final_validation_bits_per_byte: f64,
    prompt_tokens: Vec<u32>,
    prompt_text: String,
    direct_generated_tokens: Vec<u32>,
    direct_generated_text: String,
    served_generated_tokens: Vec<u32>,
    served_generated_text: String,
    direct_and_served_match: bool,
    detail: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let output_root = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/psionic_parameter_golf_homegolf_dense_bundle"));
    let evidence_seed_slot = args.next().map(|value| value.parse::<u32>()).transpose()?;
    let bundle_dir = output_root.join("bundle");
    let report_path = output_root.join(REPORT_FILENAME);
    fs::create_dir_all(&output_root)?;
    remove_dir_if_exists(&bundle_dir)?;

    let fixture = ParameterGolfLocalReferenceFixture::reference()?;
    let mut config = ParameterGolfReferenceTrainingConfig::promoted_general_small_decoder();
    if let Some(seed_slot) = evidence_seed_slot {
        config.run_id = format!("parameter-golf-homegolf-dense-bundle-proof-seed-{seed_slot:03}");
        config.checkpoint_family =
            format!("train.parameter_golf.homegolf_dense_bundle_proof.seed_{seed_slot:03}");
    } else {
        config.run_id = String::from("parameter-golf-homegolf-dense-bundle-proof");
        config.checkpoint_family = String::from("train.parameter_golf.homegolf_dense_bundle_proof");
    }

    let run = run_parameter_golf_promoted_reference_run(&fixture, &config)?;
    write_parameter_golf_promoted_reference_run(&run, &bundle_dir)?;

    let receipt = build_parameter_golf_promoted_inference_promotion_receipt(
        &bundle_dir,
        ParameterGolfPromotedProfileAssumption::GeneralPsionSmallDecoder,
    )?;
    let bundle = ParameterGolfPromotedRuntimeBundle::load_dir(&bundle_dir)?;
    if bundle.descriptor().config != ParameterGolfConfig::baseline_sp1024_9x512() {
        return Err(format!(
            "HOMEGOLF dense bundle proof expected exact baseline config, observed {:?}",
            bundle.descriptor().config
        )
        .into());
    }

    let prompt_tokens = token_sequence(&[1, 2, 3, 4]);
    let mut options = bundle.default_greedy_generation_options();
    options.max_new_tokens = 4;
    let direct_output = bundle.generate_tokens(prompt_tokens.clone(), &options)?;
    let direct_generated_tokens = direct_output
        .generated_tokens
        .as_slice()
        .iter()
        .map(|token| token.as_u32())
        .collect::<Vec<_>>();

    let mut service = CpuPromotedParameterGolfTextGenerationService::from_bundle_dir(&bundle_dir)?;
    let request = GenerationRequest::new_tokens(
        String::from("parameter-golf-homegolf-dense-bundle-proof"),
        service.model_descriptor().clone(),
        None,
        prompt_tokens.clone(),
        GenerationOptions::greedy(options.max_new_tokens),
    );
    let served_response = service.generate(&request)?;
    let served_generated_tokens = served_response
        .output
        .tokens
        .as_slice()
        .iter()
        .map(|token| token.as_u32())
        .collect::<Vec<_>>();

    let model_artifact_bytes = fs::metadata(bundle_dir.join("model.safetensors"))?.len();
    let report = HomegolfDenseBundleProofReport {
        schema_version: String::from(REPORT_SCHEMA_VERSION),
        report_id: if let Some(seed_slot) = evidence_seed_slot {
            format!("parameter_golf.homegolf_dense_bundle_proof.seed_{seed_slot:03}.v1")
        } else {
            String::from("parameter_golf.homegolf_dense_bundle_proof.v1")
        },
        track_id: String::from("parameter_golf.home_cluster_compatible_10min.v1"),
        source_dense_baseline_surface_ref: String::from(SOURCE_DENSE_BASELINE_SURFACE_REF),
        evidence_seed_slot,
        run_id: config.run_id.clone(),
        profile_id: bundle.profile_contract().profile_id.clone(),
        baseline_model_id: bundle.profile_contract().baseline_model_id.clone(),
        baseline_revision: bundle.profile_contract().baseline_revision.clone(),
        descriptor_digest: bundle.descriptor().stable_digest(),
        tokenizer_digest: bundle.tokenizer().asset().tokenizer_digest.clone(),
        model_artifact_bytes,
        final_validation_mean_loss: run.training_outcome.summary.final_validation_mean_loss,
        final_validation_bits_per_byte: run.training_outcome.summary.final_validation_bits_per_byte,
        prompt_tokens: prompt_tokens
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect(),
        prompt_text: bundle.tokenizer().decode(prompt_tokens.as_slice()),
        direct_generated_text: direct_output.text,
        direct_generated_tokens,
        served_generated_text: bundle
            .tokenizer()
            .decode(served_response.output.tokens.as_slice()),
        served_generated_tokens: served_generated_tokens.clone(),
        direct_and_served_match: direct_output
            .generated_tokens
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect::<Vec<_>>()
            == served_generated_tokens,
        detail: if let Some(seed_slot) = evidence_seed_slot {
            format!(
                "This is one retained HOMEGOLF repeated proof run for evidence slot {seed_slot}. The current bounded exact-family HOMEGOLF lane is deterministic, so repeated slots are expected to reproduce the same bundle/runtime result while preserving separate run receipts for later multi-seed packaging."
            )
        } else {
            String::from(
                "This is the first retained HOMEGOLF train-to-infer closure proof. It uses the exact 9x512 PGOLF family contract, emits a real promoted runtime bundle from the repo-owned bounded local exact-family lane, loads that bundle directly, serves it through psionic-serve, and records direct-versus-served parity. It does not claim that the retained single-H100 dense source report already carried committed model bytes or that the public challenge scorepath has been reproduced locally.",
            )
        },
    };

    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "wrote {} descriptor_digest={} promotion_receipt_digest={} direct_and_served_match={}",
        report_path.display(),
        report.descriptor_digest,
        receipt.receipt_digest,
        report.direct_and_served_match,
    );
    Ok(())
}

fn token_sequence(ids: &[u32]) -> TokenSequence {
    TokenSequence::new(ids.iter().copied().map(TokenId).collect::<Vec<_>>())
}

fn remove_dir_if_exists(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}
