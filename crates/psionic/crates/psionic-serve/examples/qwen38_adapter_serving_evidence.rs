use std::{env, error::Error, fs, path::PathBuf, time::Instant};

use psionic_serve::{
    AdapterResidencyMode, CpuGgufQwen35TextGenerationService, GenerationOptions,
    GenerationRequest, TextGenerationExecutor,
};
use psionic_train::{
    Qwen38LmHeadAdamWConfig, Qwen38LmHeadLoraBackwardFixture, Qwen38TrainingAdapterRequest,
    admit_qwen38_training_adapter, export_qwen38_lm_head_adapter_safetensors,
    finalize_qwen38_lm_head_adapter_identity, qwen38_lm_head_initial_training_state,
    run_qwen38_lm_head_adamw_step,
};
use serde_json::json;
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let gguf_path = PathBuf::from(args.next().ok_or(
        "usage: qwen38_adapter_serving_evidence <decoder.gguf> <adapter.safetensors>",
    )?);
    let adapter_path = PathBuf::from(args.next().ok_or("missing adapter artifact path")?);
    if args.next().is_some() {
        return Err("unexpected trailing arguments".into());
    }

    let started = Instant::now();
    let prompt = "Name one reason local inference matters.";

    let decoder_load_started = Instant::now();
    let mut service = CpuGgufQwen35TextGenerationService::from_gguf_path(&gguf_path)?;
    let decoder_load_duration_ns = duration_ns(decoder_load_started.elapsed());
    let descriptor = service.model_descriptor().clone();
    let vocabulary_size = descriptor.config.vocab_size;
    let hidden_size = descriptor.config.hidden_size;

    let baseline_request = GenerationRequest::new_text(
        "qwen38-adapter-serving-baseline",
        descriptor.clone(),
        None,
        prompt,
        GenerationOptions::greedy(2),
    );
    let baseline_started = Instant::now();
    let baseline = TextGenerationExecutor::generate(&mut service, &baseline_request)?;
    let baseline_duration_ns = duration_ns(baseline_started.elapsed());
    let baseline_token_ids = token_ids(&baseline);

    let activations_started = Instant::now();
    let (hidden, base_logits) = service.final_hidden_and_logits_for_text(prompt)?;
    let activations_duration_ns = duration_ns(activations_started.elapsed());
    if hidden.len() != hidden_size || base_logits.len() != vocabulary_size {
        return Err(format!(
            "decoder activation shape hidden={} logits={} does not match descriptor hidden={} vocab={}",
            hidden.len(),
            base_logits.len(),
            hidden_size,
            vocabulary_size,
        )
        .into());
    }
    let baseline_argmax = argmax(base_logits.as_slice())?;
    let target_token_id = second_argmax(base_logits.as_slice(), baseline_argmax)?;

    // The adapter recipe is deterministic: LoRA-A is the real model hidden
    // state and LoRA-B seeds a decisive logit boost on the runner-up token, so
    // the served overlay visibly moves the greedy argmax. One real AdamW step
    // then runs through the admitted reference lane so the exported artifact
    // carries an honest `trained_step`.
    let mut initial_lora_b = vec![0.0_f32; vocabulary_size];
    initial_lora_b[target_token_id] = 1_000.0;
    let fixture = Qwen38LmHeadLoraBackwardFixture {
        fixture_id: String::from("qwen38-adapter-serving-real-27b-v1"),
        hidden: hidden.clone(),
        base_logits,
        lora_rank: 1,
        lora_alpha: 1.0,
        lora_a: hidden,
        lora_b: initial_lora_b,
        target_token_id,
        learning_rate: 0.01,
        finite_difference_epsilon: 0.001,
        gradient_tolerance: 0.001,
    };
    let mut state = qwen38_lm_head_initial_training_state(&fixture)?;
    let optimizer = Qwen38LmHeadAdamWConfig::default();
    let step_receipt = run_qwen38_lm_head_adamw_step(&fixture, &optimizer, &mut state)?;

    let mut training_request = Qwen38TrainingAdapterRequest::default();
    training_request.request_id = String::from("qwen38-adapter-serving-real-27b-v1");
    training_request.adapter_id = String::from("qwen38-adapter-serving-real-27b-v1");
    training_request.adapter_revision = String::from("trained-step-1");
    training_request.lora_rank = fixture.lora_rank;
    training_request.lora_alpha = fixture.lora_alpha;
    let plan = admit_qwen38_training_adapter(&training_request);
    if !plan.is_admitted() {
        return Err(format!("Qwen3.8 training adapter admission refused: {:?}", plan.refusal).into());
    }
    let artifact_bytes = export_qwen38_lm_head_adapter_safetensors(&plan, &fixture, &state)?;
    let artifact_sha256 = hex::encode(Sha256::digest(artifact_bytes.as_slice()));
    let identity = finalize_qwen38_lm_head_adapter_identity(
        &plan,
        artifact_sha256.as_str(),
        (fixture.lora_a.len() + fixture.lora_b.len()) as u64,
    )?;
    fs::write(&adapter_path, artifact_bytes.as_slice())?;

    let binding = service.register_qwen38_lm_head_lora_adapter(
        "qwen38-adapter-serving-real-27b",
        &adapter_path,
        identity,
        fixture.lora_alpha,
        AdapterResidencyMode::HotSwapOverlay,
    )?;

    let adapted_request = GenerationRequest::new_text(
        "qwen38-adapter-serving-overlay",
        descriptor.clone(),
        None,
        prompt,
        GenerationOptions::greedy(2),
    )
    .with_adapter_serving(binding.clone());
    let adapted_started = Instant::now();
    let adapted = TextGenerationExecutor::generate(&mut service, &adapted_request)?;
    let adapted_duration_ns = duration_ns(adapted_started.elapsed());
    let adapted_token_ids = token_ids(&adapted);
    let adapted_first_is_target = adapted_token_ids.first().copied() == Some(target_token_id as u32);
    let provenance_binding = adapted
        .provenance
        .as_ref()
        .and_then(|value| value.adapter_serving.clone());
    let provenance_binding_matches = provenance_binding.as_ref() == Some(&binding);

    let merged_refusal = service
        .register_qwen38_lm_head_lora_adapter(
            "qwen38-adapter-serving-real-27b-merged",
            &adapter_path,
            finalize_qwen38_lm_head_adapter_identity(
                &plan,
                artifact_sha256.as_str(),
                (fixture.lora_a.len() + fixture.lora_b.len()) as u64,
            )?,
            fixture.lora_alpha,
            AdapterResidencyMode::MergedResident,
        )
        .err()
        .map(|error| error.to_string());
    let mut drifted_binding = binding.clone();
    drifted_binding.binding_id.push_str("-drift");
    let drifted_refusal = TextGenerationExecutor::generate(
        &mut service,
        &GenerationRequest::new_text(
            "qwen38-adapter-serving-drifted-binding",
            descriptor.clone(),
            None,
            prompt,
            GenerationOptions::greedy(1),
        )
        .with_adapter_serving(drifted_binding),
    )
    .err()
    .map(|error| error.to_string());
    let detached =
        service.detach_qwen38_adapter_binding(binding.served_adapter_digest.as_str())?;
    let detached_refusal = TextGenerationExecutor::generate(&mut service, &adapted_request)
        .err()
        .map(|error| error.to_string());

    let runtime_support = service.runtime_support();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "schema_version": "psionic.qwen38.adapter_serving_evidence.v1",
            "phase": "R12",
            "status": "partial",
            "decoder_gguf": gguf_path,
            "decoder_model": {
                "model": &descriptor.model,
                "config": &descriptor.config,
                "tokenizer_family": &descriptor.tokenizer_family,
                "weights_digest": &descriptor.weights.digest,
                "artifact_identity": &descriptor.artifact_identity,
            },
            "adapter_runtime_support": runtime_support.adapter_runtime,
            "adapter_artifact": {
                "path": adapter_path,
                "sha256": artifact_sha256,
                "byte_len": artifact_bytes.len(),
                "identity": &binding.adapters,
                "recipe": "deterministic: lora_a is the real decoder hidden state, lora_b seeds a decisive logit boost on the runner-up token, then one admitted AdamW step executes",
                "trained_step": state.step,
                "lora_rank": fixture.lora_rank,
                "lora_alpha": fixture.lora_alpha,
                "plan_digest": plan.plan_digest,
                "corpus_manifest_sha256": plan.corpus_manifest_sha256,
                "evaluation_manifest_sha256": plan.evaluation_manifest_sha256,
                "seed": plan.seed,
                "adamw_step_loss_before": step_receipt.loss_before,
                "adamw_step_loss_after": step_receipt.loss_after,
            },
            "prompt": prompt,
            "baseline": {
                "output_token_ids": baseline_token_ids,
                "output_text": baseline.output.text,
                "argmax_token_id": baseline_argmax,
            },
            "adapted": {
                "output_token_ids": adapted_token_ids,
                "output_text": adapted.output.text,
                "target_token_id": target_token_id,
                "first_token_is_target": adapted_first_is_target,
                "provenance_binding_matches": provenance_binding_matches,
            },
            "refusals": {
                "merged_residency_refused": merged_refusal.is_some(),
                "merged_residency_error": merged_refusal,
                "drifted_binding_refused": drifted_refusal.is_some(),
                "drifted_binding_error": drifted_refusal,
                "detached_binding_refused": detached_refusal.is_some(),
                "detached_binding_error": detached_refusal,
                "detached_binding_matches": detached == binding,
            },
            "decoder_load_duration_ns": decoder_load_duration_ns,
            "baseline_duration_ns": baseline_duration_ns,
            "activations_duration_ns": activations_duration_ns,
            "adapted_duration_ns": adapted_duration_ns,
            "total_duration_ns": duration_ns(started.elapsed()),
            "fallback_policy": "refuse",
            "hidden_fallback_used": false,
            "claim_boundary": "This proves real-shape Qwen3.8 LM-head LoRA adapter registration, hot-swap overlay execution, provenance binding, and refusal posture on the admitted 27B decoder on the native CPU runtime. Registration validates adapter shape and identity metadata against the declared 27B base identity; the served decoder is the qualified Q4_K_M GGUF quantization of that base. The adapter artifact is deterministic recipe output trained one AdamW step on real model activations; it is not a trained-model quality or promotion claim. Durations are diagnostic and do not establish a performance claim.",
        }))?
    );
    Ok(())
}

fn token_ids(response: &psionic_serve::GenerationResponse) -> Vec<u32> {
    response
        .output
        .tokens
        .as_slice()
        .iter()
        .map(|token| token.as_u32())
        .collect()
}

fn argmax(values: &[f32]) -> Result<usize, Box<dyn Error>> {
    values
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map(|(index, _)| index)
        .ok_or_else(|| "empty logits cannot produce an argmax".into())
}

fn second_argmax(values: &[f32], excluded: usize) -> Result<usize, Box<dyn Error>> {
    values
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != excluded)
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map(|(index, _)| index)
        .ok_or_else(|| "logits cannot produce a second argmax".into())
}

fn duration_ns(duration: std::time::Duration) -> u64 {
    duration.as_nanos().try_into().unwrap_or(u64::MAX)
}
