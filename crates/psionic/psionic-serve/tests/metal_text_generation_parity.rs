#![allow(
    unused_imports,
    clippy::expect_used,
    clippy::panic,
    clippy::panic_in_result_fn,
    clippy::too_many_arguments
)]

use psionic_backend_cpu::CpuBackend;
use psionic_backend_metal::{MetalBackend, TEXT_GENERATION_SUPPORTED_OPS};
use psionic_runtime::{DeviceDiscovery, HealthStatus, validation_reference_for_served_product};

#[cfg(target_os = "macos")]
use psionic_core::{DType, Device, QuantizationMode, Shape, TensorId};
#[cfg(target_os = "macos")]
use psionic_ir::{Graph, GraphBuilder};
#[cfg(target_os = "macos")]
use psionic_runtime::{
    BackendParityPolicy, ParityExpectation, SamplingPolicy, SamplingStrategy, TokenSampler,
    compare_embedding_vectors, compare_logits,
};
#[cfg(target_os = "macos")]
use psionic_serve::{
    ArtifactWordDecoder, CpuModelTextGenerationService, DecodeStrategy, GenerationOptions,
    GenerationRequest, TerminationReason, TextGenerationExecutor, TokenId, TokenizerBoundary,
};
#[cfg(target_os = "macos")]
use std::collections::BTreeMap;
#[cfg(target_os = "macos")]
use tempfile::tempdir;

#[cfg(target_os = "macos")]
#[derive(Clone, Debug)]
struct GenerationGraph {
    graph: Graph,
    token_input_id: TensorId,
    position_input_id: TensorId,
    context_input_id: TensorId,
    hidden_output_id: TensorId,
    logits_output_id: TensorId,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Debug)]
struct GenerationStep {
    hidden: Vec<f32>,
    logits: Vec<f32>,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct GenerationTrace {
    generated_tokens: Vec<TokenId>,
    termination: TerminationReason,
}

#[cfg(not(target_os = "macos"))]
#[test]
fn metal_text_generation_parity_reports_explicit_offline_state()
-> Result<(), Box<dyn std::error::Error>> {
    let metal = MetalBackend::new();
    assert_eq!(metal.health().status, HealthStatus::Offline);

    let fallback = metal.fallback_selection(&CpuBackend::new(), TEXT_GENERATION_SUPPORTED_OPS)?;
    assert_eq!(fallback.requested_backend, "metal");
    assert_eq!(fallback.effective_backend, "cpu");
    assert!(fallback.fallback_reason.is_some());
    assert_eq!(
        validation_reference_for_served_product(&fallback, "psionic.text_generation").claim_id,
        "metal.refusal.off_platform"
    );
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn metal_text_generation_matches_cpu_baseline_within_budget_and_seeded_sampling()
-> Result<(), Box<dyn std::error::Error>> {
    let parity_policy = BackendParityPolicy::default();
    let generation_budget = parity_policy.generation_budget(true);
    assert_eq!(generation_budget.token_choices, ParityExpectation::Exact);
    assert_eq!(generation_budget.output_text, ParityExpectation::Exact);
    assert_eq!(generation_budget.termination, ParityExpectation::Exact);

    let temp = tempdir()?;
    let path = temp.path().join("wordpiece_decoder.safetensors");
    ArtifactWordDecoder::write_default_safetensors_artifact(&path)?;

    let mut cpu_service = CpuModelTextGenerationService::from_safetensors_artifact(&path)?;
    let model = ArtifactWordDecoder::from_safetensors_artifact(&path)?;
    let mut options = GenerationOptions::sample(4);
    options.seed = Some(42);
    options.temperature = Some(0.8);
    options.top_k = Some(5);
    options.top_p = Some(0.95);

    let request = GenerationRequest::new_text(
        "gen-metal-parity-1",
        cpu_service.model_descriptor().clone(),
        None,
        "hello",
        options.clone(),
    );
    let cpu_response = cpu_service.generate(&request)?;

    let mut metal = MetalBackend::new();
    let Some(selected_device) = metal.selected_device().cloned() else {
        assert_ne!(metal.health().status, HealthStatus::Ready);
        let fallback =
            metal.fallback_selection(&CpuBackend::new(), TEXT_GENERATION_SUPPORTED_OPS)?;
        assert_eq!(fallback.requested_backend, "metal");
        assert_eq!(fallback.effective_backend, "cpu");
        assert!(fallback.fallback_reason.is_some());
        return Ok(());
    };

    let cpu_graph = build_generation_graph(Device::cpu(), &model)?;
    let metal_graph = build_generation_graph(selected_device.device, &model)?;
    let trace = run_cpu_metal_generation_parity(
        &model,
        &options,
        "hello",
        &cpu_graph,
        &metal_graph,
        &mut CpuBackend::new(),
        &mut metal,
        parity_policy,
    )?;

    assert_eq!(
        trace.generated_tokens,
        cpu_response.output.tokens.as_slice()
    );
    assert_eq!(
        model.tokenizer().decode(&trace.generated_tokens),
        cpu_response.output.text
    );
    assert_eq!(trace.termination, cpu_response.termination);
    Ok(())
}

#[cfg(target_os = "macos")]
fn build_generation_graph(
    device: Device,
    model: &ArtifactWordDecoder,
) -> Result<GenerationGraph, Box<dyn std::error::Error>> {
    let descriptor = model.descriptor();
    let config = &descriptor.config;
    let weights = model.weights();

    let mut builder = GraphBuilder::new(device);
    let token_input = builder.input(
        "token_one_hot",
        Shape::new(vec![1, config.vocab_size]),
        DType::F32,
    );
    let position_input = builder.input(
        "position_one_hot",
        Shape::new(vec![1, config.max_context]),
        DType::F32,
    );
    let context_input = builder.input(
        "context",
        Shape::new(vec![1, config.hidden_size]),
        DType::F32,
    );
    let token_embedding = builder.constant_f32(
        Shape::new(vec![config.vocab_size, config.hidden_size]),
        weights.token_embedding().to_vec(),
    )?;
    let position_embedding = builder.constant_f32(
        Shape::new(vec![config.max_context, config.hidden_size]),
        weights.position_embedding().to_vec(),
    )?;
    let context_projection = builder.constant_f32(
        Shape::new(vec![config.hidden_size, config.hidden_size]),
        weights.context_projection().to_vec(),
    )?;
    let lm_head = builder.constant_f32(
        Shape::new(vec![config.hidden_size, config.vocab_size]),
        weights.lm_head().to_vec(),
    )?;
    let lm_bias = builder.constant_f32(
        Shape::new(vec![1, config.vocab_size]),
        weights.lm_bias().to_vec(),
    )?;

    let token_hidden = builder.matmul(&token_input, &token_embedding)?;
    let position_hidden = builder.matmul(&position_input, &position_embedding)?;
    let context_hidden = builder.matmul(&context_input, &context_projection)?;
    let hidden = builder.add(&token_hidden, &position_hidden)?;
    let hidden = builder.add(&hidden, &context_hidden)?;
    let logits = builder.matmul(&hidden, &lm_head)?;
    let logits = builder.add(&logits, &lm_bias)?;

    Ok(GenerationGraph {
        graph: builder.finish(vec![hidden.clone(), logits.clone()]),
        token_input_id: token_input.id(),
        position_input_id: position_input.id(),
        context_input_id: context_input.id(),
        hidden_output_id: hidden.id(),
        logits_output_id: logits.id(),
    })
}

#[cfg(target_os = "macos")]
fn run_cpu_metal_generation_parity(
    model: &ArtifactWordDecoder,
    options: &GenerationOptions,
    prompt: &str,
    cpu_graph: &GenerationGraph,
    metal_graph: &GenerationGraph,
    cpu_backend: &mut CpuBackend,
    metal_backend: &mut MetalBackend,
    parity_policy: BackendParityPolicy,
) -> Result<GenerationTrace, Box<dyn std::error::Error>> {
    let hidden_budget = parity_policy.embedding_budget(QuantizationMode::None);
    let logit_budget = parity_policy.logit_budget(QuantizationMode::None);
    let prompt_tokens = model
        .tokenizer()
        .encode_with_special_tokens(prompt, true, false);
    let hidden_size = model.descriptor().config.hidden_size;
    let eos_id = model.tokenizer().vocabulary().eos_id();
    let sampling_policy = runtime_sampling_policy(options);
    let mut cpu_sampler = TokenSampler::new(&sampling_policy);
    let mut metal_sampler = TokenSampler::new(&sampling_policy);
    let mut history = prompt_tokens
        .as_slice()
        .iter()
        .copied()
        .map(TokenId::as_u32)
        .collect::<Vec<_>>();

    let mut cpu_hidden_cache = Vec::new();
    let mut metal_hidden_cache = Vec::new();
    let mut last_cpu_logits = Vec::new();
    let mut last_metal_logits = Vec::new();

    for (position, token) in prompt_tokens.as_slice().iter().copied().enumerate() {
        let cpu_step = execute_cpu_step(
            cpu_backend,
            cpu_graph,
            model,
            token,
            position,
            &mean_hidden_state(&cpu_hidden_cache, hidden_size),
        )?;
        let metal_step = execute_metal_step(
            metal_backend,
            metal_graph,
            model,
            token,
            position,
            &mean_hidden_state(&metal_hidden_cache, hidden_size),
        )?;
        assert_hidden_within_budget(
            position,
            &cpu_step.hidden,
            &metal_step.hidden,
            hidden_budget,
        )?;
        assert_logits_within_budget(position, &cpu_step.logits, &metal_step.logits, logit_budget)?;
        last_cpu_logits = cpu_step.logits;
        last_metal_logits = metal_step.logits;
        cpu_hidden_cache.push(cpu_step.hidden);
        metal_hidden_cache.push(metal_step.hidden);
    }

    let mut generated_tokens = Vec::new();
    let termination = loop {
        if generated_tokens.len() >= options.max_output_tokens {
            break TerminationReason::MaxOutputTokens;
        }

        let cpu_next = cpu_sampler
            .select_next_token(&last_cpu_logits, &history)
            .ok_or("missing cpu next token")?;
        let metal_next = metal_sampler
            .select_next_token(&last_metal_logits, &history)
            .ok_or("missing metal next token")?;
        assert_eq!(
            cpu_next, metal_next,
            "seeded cpu-vs-metal token choice drift with history {history:?}",
        );

        let next_token = TokenId(cpu_next);
        if next_token == eos_id {
            break TerminationReason::EndOfSequence;
        }

        let position = cpu_hidden_cache.len();
        let cpu_step = execute_cpu_step(
            cpu_backend,
            cpu_graph,
            model,
            next_token,
            position,
            &mean_hidden_state(&cpu_hidden_cache, hidden_size),
        )?;
        let metal_step = execute_metal_step(
            metal_backend,
            metal_graph,
            model,
            next_token,
            position,
            &mean_hidden_state(&metal_hidden_cache, hidden_size),
        )?;
        assert_hidden_within_budget(
            position,
            &cpu_step.hidden,
            &metal_step.hidden,
            hidden_budget,
        )?;
        assert_logits_within_budget(position, &cpu_step.logits, &metal_step.logits, logit_budget)?;

        generated_tokens.push(next_token);
        history.push(next_token.as_u32());
        last_cpu_logits = cpu_step.logits;
        last_metal_logits = metal_step.logits;
        cpu_hidden_cache.push(cpu_step.hidden);
        metal_hidden_cache.push(metal_step.hidden);
    };

    Ok(GenerationTrace {
        generated_tokens,
        termination,
    })
}

#[cfg(target_os = "macos")]
fn execute_cpu_step(
    backend: &mut CpuBackend,
    graph: &GenerationGraph,
    model: &ArtifactWordDecoder,
    token: TokenId,
    position: usize,
    context: &[f32],
) -> Result<GenerationStep, Box<dyn std::error::Error>> {
    let config = &model.descriptor().config;
    let mut runtime_inputs = BTreeMap::new();
    runtime_inputs.insert(
        graph.token_input_id,
        backend.input_buffer(
            Shape::new(vec![1, config.vocab_size]),
            one_hot(config.vocab_size, token.as_u32() as usize),
        )?,
    );
    runtime_inputs.insert(
        graph.position_input_id,
        backend.input_buffer(
            Shape::new(vec![1, config.max_context]),
            one_hot(config.max_context, position),
        )?,
    );
    runtime_inputs.insert(
        graph.context_input_id,
        backend.input_buffer(Shape::new(vec![1, config.hidden_size]), context.to_vec())?,
    );

    let result = backend.compile_and_execute(&graph.graph, &runtime_inputs)?;
    let hidden = result
        .outputs
        .get(&graph.hidden_output_id)
        .ok_or("missing cpu hidden output")?
        .as_f32_slice()
        .ok_or("missing cpu dense hidden output")?
        .to_vec();
    let logits = result
        .outputs
        .get(&graph.logits_output_id)
        .ok_or("missing cpu logits output")?
        .as_f32_slice()
        .ok_or("missing cpu dense logits output")?
        .to_vec();
    Ok(GenerationStep { hidden, logits })
}

#[cfg(target_os = "macos")]
fn execute_metal_step(
    backend: &mut MetalBackend,
    graph: &GenerationGraph,
    model: &ArtifactWordDecoder,
    token: TokenId,
    position: usize,
    context: &[f32],
) -> Result<GenerationStep, Box<dyn std::error::Error>> {
    let config = &model.descriptor().config;
    let mut runtime_inputs = BTreeMap::new();
    runtime_inputs.insert(
        graph.token_input_id,
        backend.input_buffer(
            Shape::new(vec![1, config.vocab_size]),
            one_hot(config.vocab_size, token.as_u32() as usize),
        )?,
    );
    runtime_inputs.insert(
        graph.position_input_id,
        backend.input_buffer(
            Shape::new(vec![1, config.max_context]),
            one_hot(config.max_context, position),
        )?,
    );
    runtime_inputs.insert(
        graph.context_input_id,
        backend.input_buffer(Shape::new(vec![1, config.hidden_size]), context.to_vec())?,
    );

    let result = backend.compile_and_execute(&graph.graph, &runtime_inputs)?;
    let hidden = result
        .outputs
        .get(&graph.hidden_output_id)
        .ok_or("missing metal hidden output")?
        .read_f32()?;
    let logits = result
        .outputs
        .get(&graph.logits_output_id)
        .ok_or("missing metal logits output")?
        .read_f32()?;
    Ok(GenerationStep { hidden, logits })
}

#[cfg(target_os = "macos")]
fn assert_hidden_within_budget(
    position: usize,
    expected: &[f32],
    actual: &[f32],
    budget: psionic_runtime::EmbeddingParityBudget,
) -> Result<(), Box<dyn std::error::Error>> {
    let summary = compare_embedding_vectors(expected, actual, budget)?;
    assert!(
        summary.within_budget,
        "hidden parity drift at position {position}: max_abs_delta={} max_rel_delta={} cosine_similarity={} budget={:?}",
        summary.max_abs_delta, summary.max_rel_delta, summary.cosine_similarity, budget,
    );
    Ok(())
}

#[cfg(target_os = "macos")]
fn assert_logits_within_budget(
    position: usize,
    expected: &[f32],
    actual: &[f32],
    budget: psionic_runtime::LogitParityBudget,
) -> Result<(), Box<dyn std::error::Error>> {
    let summary = compare_logits(expected, actual, budget)?;
    assert!(
        summary.within_budget,
        "logit parity drift at position {position}: max_abs_delta={} max_rel_delta={} expected_top_token={} actual_top_token={} top_token_rank_drift={} budget={:?}",
        summary.max_abs_delta,
        summary.max_rel_delta,
        summary.expected_top_token,
        summary.actual_top_token,
        summary.top_token_rank_drift,
        budget,
    );
    Ok(())
}

#[cfg(target_os = "macos")]
fn runtime_sampling_policy(options: &GenerationOptions) -> SamplingPolicy {
    SamplingPolicy {
        strategy: match options.decode_strategy {
            DecodeStrategy::Greedy => SamplingStrategy::Greedy,
            DecodeStrategy::Sample => SamplingStrategy::Sample,
        },
        temperature: options.temperature,
        top_k: options.top_k,
        top_p: options.top_p,
        repeat_penalty: options.repeat_penalty,
        presence_penalty: options.presence_penalty,
        frequency_penalty: options.frequency_penalty,
        seed: options.seed,
    }
}

#[cfg(target_os = "macos")]
fn mean_hidden_state(hidden_states: &[Vec<f32>], width: usize) -> Vec<f32> {
    if hidden_states.is_empty() {
        return vec![0.0; width];
    }

    let mut output = vec![0.0; width];
    for state in hidden_states {
        for (accumulator, value) in output.iter_mut().zip(state.iter()) {
            *accumulator += *value;
        }
    }
    let scale = 1.0 / (hidden_states.len() as f32);
    for value in &mut output {
        *value *= scale;
    }
    output
}

#[cfg(target_os = "macos")]
fn one_hot(width: usize, index: usize) -> Vec<f32> {
    let mut output = vec![0.0; width];
    output[index] = 1.0;
    output
}
