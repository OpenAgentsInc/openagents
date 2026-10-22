#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::ops::Range;

use super::{synthetic, *};
use crate::{
    deepseek_v41_cpu::{generation::argmax, transformer::CpuTransformer},
    stage_state::{
        CodecLimits, StageInbox, StageRefusal, StageWorker, StepKind, WorkerError, encode,
    },
};

const DIGEST: [u8; 32] = [7; 32];
const PROMPT: [u32; 6] = [3, 5, 0, 6, 4, 7];
const CONTEXT: usize = 64;

fn weights() -> &'static synthetic::SyntheticWeights {
    Box::leak(Box::new(synthetic::weights().unwrap()))
}

fn workers(
    weights: &'static synthetic::SyntheticWeights,
    split: &[Range<u32>],
    arithmetic: Arithmetic,
) -> Vec<StageWorker<V41CpuStageExecutor<'static>>> {
    split
        .iter()
        .enumerate()
        .map(|(i, layers)| {
            let runtime = LayerRangeRuntime::new(
                weights,
                RangeConfig {
                    transformer: synthetic::config().unwrap(),
                    layers: layers.start as usize..layers.end as usize,
                    max_context: CONTEXT,
                    arithmetic,
                },
                synthetic::map().unwrap(),
            )
            .unwrap();
            let executor = V41CpuStageExecutor::new(runtime, i as u16);
            let inbox = StageInbox::new(DIGEST, 1, executor.contract(), CodecLimits::default());
            StageWorker::new(inbox, executor)
        })
        .collect()
}

fn message(
    split: &[Range<u32>],
    request: &str,
    step: u64,
    position: u64,
    tokens: Vec<u32>,
) -> StageMessage {
    StageMessage {
        manifest_digest: DIGEST,
        generation: 1,
        request_id: request.into(),
        attempt: 1,
        stage: 0,
        layers: split[0].clone(),
        step,
        kind: if tokens.len() > 1 {
            StepKind::Prefill
        } else {
            StepKind::Decode
        },
        position,
        tokens,
        sections: Vec::new(),
    }
}

/// One step through every stage, each hop encoded and decoded as on the wire.
fn step_group(
    workers: &mut [StageWorker<V41CpuStageExecutor<'static>>],
    split: &[Range<u32>],
    mut message: StageMessage,
) -> Vec<f32> {
    for i in 0..workers.len() {
        let bytes = encode(&message, &CodecLimits::default()).unwrap();
        let sections = workers[i].step_bytes(&bytes).unwrap();
        if i + 1 == workers.len() {
            let logits = sections
                .iter()
                .find(|s| s.kind == SectionKind::Logits)
                .unwrap();
            return logits.to_f32().unwrap();
        }
        message = message.next_hop(i as u16 + 1, split[i + 1].clone(), sections);
    }
    unreachable!()
}

/// Prefill and `decode` greedy steps through a split group, checked bit for
/// bit against the unsplit transformer at every step.
fn check_split(split: &[Range<u32>], arithmetic: Arithmetic, decode: usize) -> Vec<u32> {
    let w = weights();
    let mut reference = CpuTransformer::new(
        w,
        synthetic::config().unwrap(),
        synthetic::map().unwrap(),
        CONTEXT,
        arithmetic,
    )
    .unwrap();
    let mut group = workers(w, split, arithmetic);
    let mut tokens = PROMPT.to_vec();
    let mut position = 0;
    let mut generated = Vec::new();
    for step in 0..=decode {
        let expected = reference.forward_tokens(&tokens).unwrap().logits;
        let got = step_group(
            &mut group,
            split,
            message(split, "r1", step as u64, position, tokens.clone()),
        );
        assert_eq!(
            got.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            expected.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
            "split {split:?} step {step} ({arithmetic:?})"
        );
        position += tokens.len() as u64;
        let next = argmax(&got).unwrap();
        generated.push(next);
        tokens = vec![next];
    }
    generated
}

fn even(stages: u32) -> Vec<Range<u32>> {
    (0..stages)
        .map(|i| i * 40 / stages..(i + 1) * 40 / stages)
        .collect()
}

#[test]
fn every_split_matches_the_unsplit_transformer_bit_for_bit() {
    let uneven = [0..2, 2..3, 3..9, 9..15, 15..21, 21..25, 25..37, 37..40];
    let reference = check_split(&even(1), Arithmetic::Float32, 12);
    for split in [even(2), even(3), even(4), even(8), uneven.to_vec()] {
        assert_eq!(check_split(&split, Arithmetic::Float32, 12), reference);
    }
    // Every layer is its own stage, so every boundary is crossed.
    assert_eq!(check_split(&even(40), Arithmetic::Float32, 12), reference);
}

#[test]
fn a_second_request_starts_from_empty_state() {
    let split = even(4);
    let w = weights();
    let mut group = workers(w, &split, Arithmetic::Float32);
    let first = step_group(
        &mut group,
        &split,
        message(&split, "a", 0, 0, PROMPT.to_vec()),
    );
    step_group(&mut group, &split, message(&split, "a", 1, 6, vec![2]));
    for worker in &mut group {
        worker.reset("a", 1);
    }
    let again = step_group(
        &mut group,
        &split,
        message(&split, "b", 0, 0, PROMPT.to_vec()),
    );
    assert_eq!(first, again);
}

#[test]
fn refused_messages_leave_the_stage_unchanged() {
    let split = even(2);
    let w = weights();
    let mut group = workers(w, &split, Arithmetic::Float32);
    let first = message(&split, "r", 0, 0, PROMPT.to_vec());
    let sections = group[0].step(first.clone()).unwrap();
    let hop = first.next_hop(1, split[1].clone(), sections);
    let before = group[1].executor().runtime().position();
    let mut stale = hop.clone();
    stale.generation = 0;
    assert!(matches!(
        group[1].step(stale),
        Err(WorkerError::Refused(StageRefusal::StaleGeneration { .. }))
    ));
    let mut wrong = hop.clone();
    wrong.layers = 20..30;
    assert!(matches!(
        group[1].step(wrong),
        Err(WorkerError::Refused(StageRefusal::WrongStage { .. }))
    ));
    let mut missing = hop.clone();
    missing.sections.retain(|s| s.kind != SectionKind::PreMix);
    assert!(matches!(
        group[1].step(missing),
        Err(WorkerError::Refused(StageRefusal::MissingSection(
            SectionKind::PreMix
        )))
    ));
    assert_eq!(group[1].executor().runtime().position(), before);
    group[1].step(hop.clone()).unwrap();
    assert!(matches!(group[1].step(hop), Err(WorkerError::Refused(_))));
    assert_eq!(group[1].executor().runtime().position(), 6);
}

#[test]
fn ranges_outside_the_model_are_refused() {
    let w = weights();
    for layers in [0..0, 30..41, 12..12] {
        assert!(
            LayerRangeRuntime::new(
                w,
                RangeConfig {
                    transformer: synthetic::config().unwrap(),
                    layers,
                    max_context: CONTEXT,
                    arithmetic: Arithmetic::Float32,
                },
                synthetic::map().unwrap(),
            )
            .is_err()
        );
    }
}

#[test]
fn boundaries_carry_the_pre_mix() {
    let config = synthetic::config().unwrap();
    for layer in 1..40 {
        let kinds = boundary_kinds(&config, layer);
        assert_eq!(kinds[..2], [SectionKind::Residual, SectionKind::PreMix]);
    }
}

/// Recording uses the actual scores and picks, and changes no forward.
#[test]
fn index_observations_preserve_outputs_and_explain_every_selection() {
    let w = weights();
    let split = even(1);
    let make = |observe| {
        let runtime = LayerRangeRuntime::new(
            w,
            RangeConfig {
                transformer: synthetic::config().unwrap(),
                layers: 0..40,
                max_context: CONTEXT,
                arithmetic: Arithmetic::Float32,
            },
            synthetic::map().unwrap(),
        )
        .unwrap();
        let executor = V41CpuStageExecutor::new(runtime, 0);
        let executor = if observe {
            executor.with_index_observations()
        } else {
            executor
        };
        let inbox = StageInbox::new(DIGEST, 1, executor.contract(), CodecLimits::default());
        vec![StageWorker::new(inbox, executor)]
    };
    let mut plain = make(false);
    let mut observed = make(true);
    let mut position = 0;
    let mut tokens = PROMPT.to_vec();
    for step in 0..4 {
        let msg = message(&split, "index", step, position, tokens.clone());
        let a = step_group(&mut plain, &split, msg.clone());
        let b = step_group(&mut observed, &split, msg);
        assert_eq!(
            a.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            b.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
        assert!(plain[0].executor().index_observations().is_empty());
        let g = observed[0]
            .executor()
            .runtime()
            .config()
            .transformer
            .block
            .geometry;
        let traces = observed[0].executor().index_observations();
        assert_eq!(traces.len(), 8);
        for (layer, sites) in traces {
            let ratio = compress_ratio(*layer).unwrap();
            let width = CONTEXT / ratio + 1;
            assert_eq!(sites["IndexScores"].len(), tokens.len() * width);
            for row in 0..tokens.len() {
                let visible = (position as usize + row + 1) / ratio;
                let scores: Vec<f32> = sites["IndexScores"][row * width..row * width + visible]
                    .iter()
                    .map(|w| f32::from_bits(*w))
                    .collect();
                assert!(scores.iter().all(|v| v.is_finite()));
                let flags: Vec<u8> = sites
                    .get("CandidateFlags")
                    .map(|words| {
                        let stride = words.len() / tokens.len();
                        words[row * stride..(row + 1) * stride]
                            .iter()
                            .flat_map(|w| w.to_le_bytes())
                            .collect()
                    })
                    .unwrap_or_default();
                let mask: Vec<bool> = (0..visible)
                    .map(|i| flags.get(i / CANDIDATE_BLOCK_ROWS).is_some_and(|b| *b != 0))
                    .collect();
                let mut want = attention::select_indices(
                    &scores,
                    visible,
                    g.index_top_k,
                    0,
                    (*layer > CANDIDATE_SOURCE as usize).then_some(mask.as_slice()),
                )
                .unwrap();
                want.resize(g.index_top_k, -1);
                let got: Vec<i32> = sites["Selection"]
                    [row * g.index_top_k..(row + 1) * g.index_top_k]
                    .iter()
                    .map(|w| *w as i32)
                    .collect();
                assert_eq!(got, want, "layer {layer} row {row}");
            }
        }
        position += tokens.len() as u64;
        tokens = vec![argmax(&a).unwrap()];
    }
}
