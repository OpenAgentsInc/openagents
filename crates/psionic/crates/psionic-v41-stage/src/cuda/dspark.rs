//! Single-box DSpark decode with exact target verification (#12469).

use super::{CheckpointSource, CudaModel, CudaWeights, chain::ChainOptions};
use crate::{PromptMode, Refused, Sampling, TextStream};
use psionic_backend_cuda::{
    CudaBackend,
    v41_stage::{
        V41Sampling, V41Stage, V41StepInput,
        dspark::{BLOCK, Drafter, Snapshot, VERIFY},
    },
};
use serde_json::{Value, json};
use std::time::Instant;

fn bad(e: impl std::fmt::Display) -> Refused {
    Refused(format!("DSpark: {e}"))
}

/// The temperature/nucleus distribution used by both the proposal and
/// target. Rejection sampling needs the full normalized probabilities,
/// including tokens the draft's nucleus excludes.
fn distribution(logits: &[f32], sampling: Sampling) -> Result<Vec<f64>, Refused> {
    if logits.is_empty()
        || logits.iter().any(|v| !v.is_finite())
        || !(0.0..=2.0).contains(&sampling.temperature)
        || !(sampling.top_p > 0.0 && sampling.top_p <= 1.0)
    {
        return Err(bad("invalid logits or sampling settings"));
    }
    let mut p = vec![0.0; logits.len()];
    if sampling.temperature == 0.0 {
        let token = crate::sample(logits, sampling, 0)?;
        p[token as usize] = 1.0;
        return Ok(p);
    }
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut order: Vec<(usize, f64)> = logits
        .iter()
        .enumerate()
        .map(|(i, v)| {
            (
                i,
                ((f64::from(*v) - f64::from(max)) / sampling.temperature).exp(),
            )
        })
        .collect();
    let sum: f64 = order.iter().map(|(_, p)| p).sum();
    order.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut mass = 0.0;
    for (i, v) in order {
        p[i] = v / sum;
        mass += p[i];
        if mass >= sampling.top_p {
            break;
        }
    }
    for v in &mut p {
        *v /= mass;
    }
    Ok(p)
}

fn draw(p: &[f64], uniform: f64) -> u32 {
    let mut threshold = uniform;
    for (i, v) in p.iter().enumerate() {
        if threshold < *v {
            return i as u32;
        }
        threshold -= v;
    }
    p.iter().rposition(|v| *v > 0.0).unwrap_or(0) as u32
}

fn uniform(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut x = *state;
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    ((x ^ (x >> 31)) >> 11) as f64 / (1_u64 << 53) as f64
}

/// Accept with min(1,p/q); after rejection sample normalized (p-q)+.
fn check_proposal(p: &[f64], q: &[f64], token: u32) -> Result<(), Refused> {
    if p.len() != q.len() || token as usize >= q.len() || q[token as usize] <= 0.0 {
        return Err(bad(
            "the proposal has no probability for its token or uses another vocabulary",
        ));
    }
    Ok(())
}

fn accept(p: &[f64], q: &[f64], token: u32, uniform: f64) -> Result<(), Vec<f64>> {
    let at = token as usize;
    if q[at] > 0.0 && uniform * q[at] < p[at] {
        return Ok(());
    }
    let mut residual: Vec<f64> = p.iter().zip(q).map(|(p, q)| (p - q).max(0.0)).collect();
    let mass: f64 = residual.iter().sum();
    // Equal distributions cannot reject a token in q's support. Treat a
    // zero residual as target sampling only for roundoff at the boundary.
    if mass == 0.0 {
        return Err(p.to_vec());
    }
    for v in &mut residual {
        *v /= mass;
    }
    Err(residual)
}

fn step(
    stage: &mut V41Stage,
    backend: &CudaBackend,
    position: usize,
    tokens: &[u32],
    emit: bool,
) -> Result<(), Refused> {
    stage
        .step_with_output(
            backend,
            &V41StepInput {
                position,
                tokens,
                residual: None,
                pre_mix: None,
                source_rows: None,
                selection: None,
                candidates: None,
                sampling: V41Sampling::greedy(),
            },
            false,
            emit,
        )
        .map_err(bad)?;
    Ok(())
}

/// Decides the accepted prefix before committing it. Any invalid target
/// distribution or failed prefix replay restores the pre-verification state.
fn finish_verification(
    stage: &mut V41Stage,
    backend: &CudaBackend,
    snapshot: &Snapshot,
    position: usize,
    inputs: &[u32],
    target: &[Vec<f32>],
    q: &[Vec<f64>],
    sampling: Sampling,
    rng: &mut u64,
    eos: Option<u32>,
) -> Result<(usize, Option<u32>), Refused> {
    let initial_rng = *rng;
    let result = (|| {
        if inputs.len() != VERIFY || target.len() != VERIFY || q.len() != BLOCK {
            return Err(bad("incomplete draft verification"));
        }
        let mut accepted = 0;
        let mut next = None;
        let mut ended = false;
        for i in 0..BLOCK {
            let p = distribution(&target[i], sampling)?;
            check_proposal(&p, &q[i], inputs[i + 1])?;
            match accept(&p, &q[i], inputs[i + 1], uniform(rng)) {
                Ok(()) => {
                    accepted += 1;
                    if Some(inputs[i + 1]) == eos {
                        ended = true;
                        break;
                    }
                }
                Err(residual) => {
                    next = Some(draw(&residual, uniform(rng)));
                    break;
                }
            }
        }
        if next.is_none() && !ended {
            next = Some(draw(&distribution(&target[BLOCK], sampling)?, uniform(rng)));
        }
        if accepted < BLOCK {
            stage.restore_dspark(backend, snapshot).map_err(bad)?;
            step(stage, backend, position, &inputs[..accepted + 1], false)?;
        }
        Ok((accepted, next))
    })();
    if result.is_err() {
        *rng = initial_rng;
        stage.restore_dspark(backend, snapshot).map_err(bad)?;
    }
    result
}

/// Runs only the new explicit single-box drafting path. Other chain
/// profiles and every qualification bundle retain their ordinary decode.
pub fn run(options: &ChainOptions) -> Result<Value, Refused> {
    if options.split != [0..40]
        || options.gpus.len() != 1
        || options.first_stage_only
        || options.force_ids.is_some()
        || options.capture.is_some()
        || options.calibrate_out.is_some()
        || options.load_only
        || options.max_step_tokens < VERIFY
    {
        return Err(bad(
            "--dspark needs --split 0-40, one GPU, and at least six step tokens; capture, forcing, calibration, and load-only use ordinary decode",
        ));
    }
    let mut model = CudaModel::load_dspark(&options.weights)?;
    model.experts = options.experts;
    model.experts_on_host = options.experts == crate::Experts::Host;
    model.tiers = options.tiers.clone();
    model.tiers.batched_prefill = true;
    let CudaWeights::Checkpoint(checkpoint, _) = &model.kind else {
        return Err(bad(
            "--dspark needs the V4.1-Flash-1017 checkpoint's matching mtp tensors",
        ));
    };
    let mut backend = CudaBackend::with_device_index(options.gpus[0]);
    if backend.selected_device().is_none() || !backend.quantized_kernels_available() {
        return Err(bad("CUDA unavailable"));
    }
    let started = Instant::now();
    let reservation = Drafter::reservation(&mut backend).map_err(bad)?;
    let mut stage = model.stage_sized(
        &mut backend,
        0..40,
        options.max_context,
        options.max_step_tokens,
    )?;
    stage.enable_dspark(&mut backend).map_err(bad)?;
    drop(reservation);
    let mut drafter = Drafter::load(
        &mut backend,
        &stage,
        &CheckpointSource(checkpoint),
        256,
        128_799,
    )
    .map_err(bad)?;
    let mut snapshot = stage.dspark_snapshot(&mut backend).map_err(bad)?;
    let cache_at_load = stage.expert_cache_stats().map(|s| {
        json!({"slots_per_layer": s.slots_per_layer, "layers": s.layers,
            "device_bytes": s.device_bytes, "staging_bytes": s.staging_bytes})
    });
    let load_seconds = started.elapsed().as_secs_f64();
    let prompts = if let Some(ids) = &options.prompt_ids {
        vec![(ids.clone(), options.mode)]
    } else if options.prompts.is_empty() {
        vec![(
            model
                .vocabulary
                .encode(options.prompt.as_ref(), model.pad, options.mode)?,
            options.mode,
        )]
    } else {
        options
            .prompts
            .iter()
            .map(|p| {
                let mode = if p.get("prompt_ids").is_some() {
                    PromptMode::of(p)?
                } else {
                    options.mode
                };
                let ids = match p.get("prompt_ids") {
                    Some(ids) => serde_json::from_value(ids.clone()).map_err(bad)?,
                    None => model.vocabulary.encode(Some(p), model.pad, mode)?,
                };
                Ok((ids, mode))
            })
            .collect::<Result<Vec<_>, Refused>>()?
    };
    let tokenizer = model.vocabulary.tokenizer();
    let mut answers = Vec::new();
    for round in 0..prompts.len().max(options.repeat_answer) {
        let (prompt, mode) = &prompts[round % prompts.len()];
        if prompt.is_empty() || prompt.len() >= options.max_context {
            return Err(bad("prompt exceeds the context"));
        }
        drafter.reset();
        let started = Instant::now();
        let chunk = options
            .prefill_chunk
            .unwrap_or(options.max_step_tokens)
            .min(options.max_step_tokens)
            .max(1);
        let mut position = 0;
        for tokens in prompt.chunks(chunk) {
            step(
                &mut stage,
                &backend,
                position,
                tokens,
                position + tokens.len() == prompt.len(),
            )?;
            position += tokens.len();
        }
        let prefill_seconds = started.elapsed().as_secs_f64();
        let mut rng = u64::from(options.sampling.seed) ^ 0x4453_7061_726b;
        let mut token = draw(
            &distribution(&stage.logits().map_err(bad)?, options.sampling)?,
            uniform(&mut rng),
        );
        let mut tokens = Vec::new();
        let mut proposed = 0_u64;
        let mut kept = 0_u64;
        let mut blocks = 0_u64;
        let mut replay_tokens = 0_usize;
        let mut accepted_lengths = Vec::new();
        let mut confidence_sum = 0.0_f64;
        let started = Instant::now();
        if options.max_tokens > 0 {
            tokens.push(token);
        }
        while tokens.len() < options.max_tokens as usize
            && token != model.eos
            && position < options.max_context
        {
            // Near the end there is no room for a complete proposal. The
            // ordinary one-token step has the same target distribution.
            if position + VERIFY > options.max_context
                || tokens.len() + BLOCK + 1 > options.max_tokens as usize
            {
                step(&mut stage, &backend, position, &[token], true)?;
                position += 1;
                token = draw(
                    &distribution(&stage.logits().map_err(bad)?, options.sampling)?,
                    uniform(&mut rng),
                );
                tokens.push(token);
                continue;
            }
            let mut proposal = drafter
                .forward(&backend, &stage, position, token)
                .map_err(bad)?;
            let mut drafts = Vec::new();
            let mut q = Vec::new();
            let hidden = stage.config().shape.hidden;
            let mut preceding = token;
            for i in 0..BLOCK {
                confidence_sum += f64::from(
                    drafter
                        .finish_token(
                            &backend,
                            preceding,
                            &proposal.hidden[i * hidden..(i + 1) * hidden],
                            &mut proposal.logits[i],
                        )
                        .map_err(bad)?,
                );
                let distribution = distribution(&proposal.logits[i], options.sampling)?;
                preceding = draw(&distribution, uniform(&mut rng));
                drafts.push(preceding);
                q.push(distribution);
            }
            stage.save_dspark(&backend, &mut snapshot).map_err(bad)?;
            let mut inputs = vec![token];
            inputs.extend(&drafts);
            let target = match stage.verify_dspark(&backend, position, &inputs) {
                Ok(target) => target,
                Err(e) => {
                    stage.restore_dspark(&backend, &snapshot).map_err(bad)?;
                    return Err(bad(e));
                }
            };
            blocks += 1;
            proposed += BLOCK as u64;
            let (accepted, next) = finish_verification(
                &mut stage,
                &backend,
                &snapshot,
                position,
                &inputs,
                &target,
                &q,
                options.sampling,
                &mut rng,
                Some(model.eos),
            )?;
            accepted_lengths.push(accepted);
            kept += accepted as u64;
            tokens.extend_from_slice(&drafts[..accepted]);
            if accepted < BLOCK {
                replay_tokens += accepted + 1;
            }
            position += accepted + 1;
            if tokens.last() == Some(&model.eos) {
                break;
            }
            token = next.ok_or_else(|| bad("missing verified next token"))?;
            tokens.push(token);
        }
        let decode_seconds = started.elapsed().as_secs_f64();
        let mut stream = TextStream::default();
        let text: String = tokens.iter().map(|t| stream.push(&tokenizer, *t)).collect();
        let answer = json!({"round": round, "prompt_tokens": prompt.len(), "prompt_ids": prompt, "mode": mode.as_str(),
            "tokens": tokens, "text": text, "prefill_seconds": prefill_seconds,
            "decode_seconds": decode_seconds, "decode_tokens_per_second": tokens.len().saturating_sub(1) as f64 / decode_seconds,
            "dspark": {"blocks": blocks, "proposed": proposed, "accepted": kept,
                "accepted_lengths": accepted_lengths,
                "acceptance_rate": if proposed == 0 {0.0} else {kept as f64 / proposed as f64},
                "replay_tokens": replay_tokens, "mean_confidence": if proposed == 0 {0.0} else {confidence_sum / proposed as f64}}});
        eprintln!(
            "DSpark round {round}: {:.2} tok/s, {kept}/{proposed} drafts accepted",
            tokens.len().saturating_sub(1) as f64 / decode_seconds
        );
        answers.push(answer);
    }
    if let Some(path) = &options.answers_out {
        let mut out = String::new();
        for answer in &answers {
            out.push_str(&serde_json::to_string(answer).map_err(bad)?);
            out.push('\n');
        }
        std::fs::write(path, out).map_err(bad)?;
    }
    Ok(
        json!({"schema": "openagents.deepseek_v41.dspark_run.v1", "load_seconds": load_seconds,
        "expert_cache_at_load": cache_at_load,
        "profile": model.tiers.profile(crate::device_class(crate::Backend::Cuda)),
        "draft_block_size": BLOCK, "answers": answers}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejection_uses_the_positive_target_minus_draft_distribution() {
        let p = [0.2, 0.3, 0.5];
        let q = [0.8, 0.2, 0.0];
        let residual = accept(&p, &q, 0, 0.5).unwrap_err();
        assert_eq!(residual[0], 0.0);
        assert!((residual[1] - 1.0 / 6.0).abs() < 1e-15);
        assert!((residual[2] - 5.0 / 6.0).abs() < 1e-15);
        assert!(accept(&p, &q, 0, 0.1).is_ok());
        assert!(accept(&p, &q, 1, 0.999).is_ok());
    }

    #[test]
    fn nucleus_support_changes_are_kept_in_rejection_probabilities() {
        let s = Sampling {
            temperature: 1.0,
            top_p: 0.7,
            seed: 1,
        };
        let p = distribution(&[2.0, 1.0, 0.0], s).unwrap();
        let q = distribution(&[0.0, 1.0, 2.0], s).unwrap();
        assert_eq!(p[2], 0.0);
        assert_eq!(q[0], 0.0);
        let r = accept(&p, &q, 2, 0.5).unwrap_err();
        assert_eq!(draw(&r, 0.5), 0);
    }

    #[test]
    fn greedy_keeps_only_the_target_argmax_and_preserves_ties() {
        let p = distribution(&[1.0, 3.0, 3.0], Sampling::GREEDY).unwrap();
        let same = distribution(&[0.0, 1.0, 0.0], Sampling::GREEDY).unwrap();
        let wrong = distribution(&[4.0, 1.0, 0.0], Sampling::GREEDY).unwrap();
        assert!(accept(&p, &same, 1, 0.9).is_ok());
        let correction = accept(&p, &wrong, 0, 0.0).unwrap_err();
        assert_eq!(draw(&correction, 0.8), 1);
    }
}

/// Lookahead for the ordinary stage link. The head still sends and
/// receives one token at a time. A changed input or sampling setting
/// rewinds the pending block before doing new work.
pub(super) struct Streaming {
    drafter: Drafter,
    snapshot: psionic_backend_cuda::v41_stage::dspark::Snapshot,
    pending: std::collections::VecDeque<(usize, u32, u32, Vec<f32>)>,
    baseline: usize,
    consumed: Vec<u32>,
    sampling: Option<V41Sampling>,
    rng: u64,
    pub(super) logits: Option<Vec<f32>>,
}

impl Streaming {
    pub(super) fn load(
        model: &CudaModel,
        backend: &mut CudaBackend,
        stage: &mut V41Stage,
    ) -> Result<Self, Refused> {
        stage.enable_dspark(backend).map_err(bad)?;
        let drafter = match &model.kind {
            CudaWeights::Checkpoint(checkpoint, _) => {
                Drafter::load(backend, stage, &CheckpointSource(checkpoint), 256, 128_799)
            }
            CudaWeights::Synthetic(store) => Drafter::load(backend, stage, store.as_ref(), 32, 0),
        }
        .map_err(bad)?;
        let snapshot = stage.dspark_snapshot(backend).map_err(bad)?;
        Ok(Self {
            drafter,
            snapshot,
            pending: std::collections::VecDeque::new(),
            baseline: 0,
            consumed: Vec::new(),
            sampling: None,
            rng: 0,
            logits: None,
        })
    }

    pub(super) fn reset(&mut self) {
        self.pending.clear();
        self.consumed.clear();
        self.sampling = None;
        self.logits = None;
        self.drafter.reset();
    }

    fn rewind(&mut self, stage: &mut V41Stage, backend: &CudaBackend) -> Result<(), Refused> {
        if !self.pending.is_empty() {
            stage.restore_dspark(backend, &self.snapshot).map_err(bad)?;
            if !self.consumed.is_empty() {
                step(stage, backend, self.baseline, &self.consumed, false)?;
            }
        }
        self.pending.clear();
        self.consumed.clear();
        Ok(())
    }

    pub(super) fn step(
        &mut self,
        stage: &mut V41Stage,
        backend: &CudaBackend,
        input: &V41StepInput<'_>,
        emit: bool,
    ) -> Result<psionic_backend_cuda::v41_stage::V41StepOutput, Refused> {
        if input.position == 0 {
            self.reset();
        }
        if self.sampling == Some(input.sampling) && input.tokens.len() == 1 && emit {
            if let Some((position, expected, _, _)) = self.pending.front() {
                if *position == input.position && *expected == input.tokens[0] {
                    let (_, _, token, logits) = self
                        .pending
                        .pop_front()
                        .ok_or_else(|| bad("empty draft cache"))?;
                    self.consumed.push(input.tokens[0]);
                    self.logits = Some(logits);
                    return Ok(psionic_backend_cuda::v41_stage::V41StepOutput {
                        token: Some(token),
                        ..Default::default()
                    });
                }
            }
        }
        self.rewind(stage, backend)?;
        if self.sampling != Some(input.sampling) {
            self.sampling = Some(input.sampling);
            self.rng = input.sampling.seed ^ 0x4453_7061_726b;
        }
        if input.position == 0
            || input.tokens.len() != 1
            || !emit
            || input.position + VERIFY > stage.config().max_context
        {
            let out = stage
                .step_with_output(backend, input, false, emit)
                .map_err(bad)?;
            self.logits = if emit {
                Some(stage.logits().map_err(bad)?)
            } else {
                None
            };
            return Ok(out);
        }
        let sampling = Sampling {
            temperature: f64::from(input.sampling.temperature),
            top_p: f64::from(input.sampling.top_p),
            seed: input.sampling.seed as u32,
        };
        let mut proposal = self
            .drafter
            .forward(backend, stage, input.position, input.tokens[0])
            .map_err(bad)?;
        let h = stage.config().shape.hidden;
        let mut q = Vec::new();
        let mut inputs = input.tokens.to_vec();
        for i in 0..BLOCK {
            self.drafter
                .finish_token(
                    backend,
                    *inputs.last().ok_or_else(|| bad("draft input"))?,
                    &proposal.hidden[i * h..(i + 1) * h],
                    &mut proposal.logits[i],
                )
                .map_err(bad)?;
            let p = distribution(&proposal.logits[i], sampling)?;
            inputs.push(draw(&p, uniform(&mut self.rng)));
            q.push(p);
        }
        stage
            .save_dspark(backend, &mut self.snapshot)
            .map_err(bad)?;
        let target = match stage.verify_dspark(backend, input.position, &inputs) {
            Ok(target) => target,
            Err(e) => {
                stage.restore_dspark(backend, &self.snapshot).map_err(bad)?;
                return Err(bad(e));
            }
        };
        let (accepted, correction) = finish_verification(
            stage,
            backend,
            &self.snapshot,
            input.position,
            &inputs,
            &target,
            &q,
            sampling,
            &mut self.rng,
            None,
        )?;
        let correction = correction.ok_or_else(|| bad("missing verified next token"))?;
        let mut outputs = inputs[1..accepted + 1].to_vec();
        outputs.push(correction);
        self.baseline = input.position;
        self.consumed = input.tokens.to_vec();
        self.logits = Some(target[0].clone());
        for i in 1..outputs.len() {
            self.pending.push_back((
                input.position + i,
                outputs[i - 1],
                outputs[i],
                target[i].clone(),
            ));
        }
        eprintln!("DSpark: accepted {accepted}/{BLOCK} draft tokens");
        Ok(psionic_backend_cuda::v41_stage::V41StepOutput {
            token: Some(outputs[0]),
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod streaming_tests {
    use super::*;

    #[test]
    fn dspark_streaming_rewinds_changed_inputs_and_resets_prefixes()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let ready = backend.selected_device().is_some() && backend.quantized_kernels_available();
        assert!(
            ready || std::env::var_os("PSIONIC_REQUIRE_CUDA").is_none(),
            "CUDA required"
        );
        if !ready {
            return Ok(());
        }
        let root = tempfile::tempdir()?;
        crate::write_synthetic(root.path(), "dspark-stream")?;
        let mut model = CudaModel::load(root.path(), None)?;
        model.tiers.dspark = true;
        let mut stage = model.stage_sized(&mut backend, 0..40, 64, 24)?;
        let mut plain = model.stage_sized(&mut backend, 0..40, 64, 24)?;
        let mut stream = Streaming::load(&model, &mut backend, &mut stage)?;
        fn input(position: usize, tokens: &[u32]) -> V41StepInput<'_> {
            V41StepInput {
                position,
                tokens,
                residual: None,
                pre_mix: None,
                source_rows: None,
                selection: None,
                candidates: None,
                sampling: V41Sampling::greedy(),
            }
        }
        let prompt = [3, 5, 7, 11, 13, 17, 19, 23, 29];
        plain.step(&backend, &input(0, &prompt), false)?;
        stream.step(&mut stage, &backend, &input(0, &prompt), true)?;
        let baseline = prompt.len();
        let inputs = [2, 3, 5, 7, 11, 13];
        for bad_row in [0, BLOCK] {
            stage.save_dspark(&backend, &mut stream.snapshot)?;
            let mut targets = stage.verify_dspark(&backend, baseline, &inputs)?;
            let mut q = Vec::new();
            for i in 0..BLOCK {
                targets[i].fill(0.0);
                targets[i][inputs[i + 1] as usize] = 1.0;
                q.push(distribution(&targets[i], Sampling::GREEDY)?);
            }
            targets[bad_row][0] = f32::NAN;
            let mut rng = 17;
            let error = finish_verification(
                &mut stage,
                &backend,
                &stream.snapshot,
                baseline,
                &inputs,
                &targets,
                &q,
                Sampling::GREEDY,
                &mut rng,
                None,
            )
            .unwrap_err();
            assert!(error.0.contains("invalid logits"));
            assert_eq!(rng, 17);
            assert_eq!(stage.logits()?, plain.logits()?);
        }
        stage.save_dspark(&backend, &mut stream.snapshot)?;
        let targets = stage.verify_dspark(&backend, baseline, &inputs)?;
        stream.baseline = baseline;
        stream.consumed.clear();
        stream.sampling = Some(V41Sampling::greedy());
        for (i, logits) in targets.iter().enumerate() {
            let token = crate::sample(logits, Sampling::GREEDY, 0)?;
            stream
                .pending
                .push_back((baseline + i, inputs[i], token, logits.clone()));
        }
        // Read two cached steps, then force a different token. This must
        // discard four future inputs and restore the logically used prefix.
        for i in 0..10 {
            let token = if i < 2 {
                inputs[i]
            } else if i == 2 {
                31
            } else {
                crate::sample(&plain.logits()?, Sampling::GREEDY, 0)?
            };
            let input = input(baseline + i, std::slice::from_ref(&token));
            let expected = plain.step(&backend, &input, false)?;
            let actual = stream.step(&mut stage, &backend, &input, true)?;
            assert_eq!(expected.token, actual.token, "stream step {i}");
            assert_eq!(
                plain.logits()?,
                stream.logits.clone().ok_or("stream logits")?
            );
        }
        // A short changed prefix follows an unfinished speculative block.
        let changed = [41, 43, 47];
        let expected = plain.step(&backend, &input(0, &changed), false)?;
        let actual = stream.step(&mut stage, &backend, &input(0, &changed), true)?;
        assert_eq!(expected.token, actual.token);
        assert_eq!(plain.logits()?, stream.logits.ok_or("reset logits")?);
        Ok(())
    }
}
