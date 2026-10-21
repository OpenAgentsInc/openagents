//! Fault tests for the V4.1 stage contract with a synthetic stage executor,
//! wire sizes for the synthetic and real geometries, and placement from the
//! checkpoint's header bytes. No weights are needed.
//!
//! The synthetic executor keeps V4.1's 40-layer sharing layout (KV sources,
//! index sources, layer-20 candidates, Engram layers) with tiny widths and
//! simple deterministic arithmetic, so a split that drops or misroutes shared
//! state changes the answer.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use super::*;
use crate::stage_state::{
    InboxSummary, STAGE_STATE_MAGIC, StageExecutor, StageInbox, StageWorker, WorkerError, decode,
};

// ---------------------------------------------------------------------------
// Synthetic executor
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct SyntheticError(String);

impl fmt::Display for SyntheticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SyntheticError {}

#[derive(Default, Debug)]
struct RequestState {
    kv: BTreeMap<u32, Vec<f32>>,
    keys: BTreeMap<u32, Vec<f32>>,
    history: Vec<u32>,
    sent_rows: BTreeMap<SectionKind, u64>,
}

#[derive(Debug)]
struct SyntheticStage {
    g: V41StageGeometry,
    layers: Range<u32>,
    next: Option<StageContract>,
    requests: BTreeMap<String, RequestState>,
    steps_run: u64,
}

const PAD: u32 = u32::MAX;

fn err(message: impl Into<String>) -> SyntheticError {
    SyntheticError(message.into())
}

impl SyntheticStage {
    fn new(g: V41StageGeometry, layers: Range<u32>, next: Option<StageContract>) -> Self {
        Self {
            g,
            layers,
            next,
            requests: BTreeMap::new(),
            steps_run: 0,
        }
    }
}

fn top_by_score(scores: &[(usize, f32)], k: usize) -> Vec<usize> {
    let mut sorted = scores.to_vec();
    sorted.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    sorted.into_iter().take(k).map(|(i, _)| i).collect()
}

impl StageExecutor for SyntheticStage {
    type Error = SyntheticError;

    #[allow(clippy::too_many_lines)]
    fn step(&mut self, m: &StageMessage) -> Result<Vec<Section>, SyntheticError> {
        self.steps_run += 1;
        let g = self.g;
        let (streams, hidden) = (g.streams as usize, g.hidden as usize);
        let width = streams * hidden;
        let tokens = m.tokens.len();
        let state = self.requests.entry(m.request_id.clone()).or_default();
        let first_stage = self.layers.start == 0;

        // Inbound state.
        let mut residual = if first_stage {
            let mut r = Vec::with_capacity(tokens * width);
            for &t in &m.tokens {
                for i in 0..width {
                    r.push(((t as usize * 31 + i * 7) % 17) as f32 / 17.0);
                }
            }
            r
        } else {
            m.section(SectionKind::Residual)
                .ok_or_else(|| err("no residual"))?
                .to_f32()
                .map_err(|e| err(e.to_string()))?
        };
        // Each token's last pre-mix: the next layer collapses with it.
        let mut pre: Vec<f32> = if first_stage {
            (0..tokens)
                .flat_map(|_| (0..streams).map(|s| if s == 0 { 1.0 } else { 0.0 }))
                .collect()
        } else {
            m.section(SectionKind::PreMix)
                .ok_or_else(|| err("no pre-mix"))?
                .to_f32()
                .map_err(|e| err(e.to_string()))?
        };
        let mut selections: BTreeMap<u32, Vec<Vec<i32>>> = BTreeMap::new();
        let mut candidates: Option<Vec<Vec<u32>>> = None;
        let mut before: Vec<u32> = Vec::new();
        if first_stage {
            let n = (g.max_ngram - 1) as usize;
            before = vec![PAD; n.saturating_sub(state.history.len())];
            let start = state.history.len().saturating_sub(n);
            before.extend_from_slice(&state.history[start..]);
        }
        for s in &m.sections {
            match s.kind {
                SectionKind::SourceKv { layer } => state
                    .kv
                    .entry(layer)
                    .or_default()
                    .extend(s.to_f32().map_err(|e| err(e.to_string()))?),
                SectionKind::SourceIndexKeys { layer } => state
                    .keys
                    .entry(layer)
                    .or_default()
                    .extend(s.to_f32().map_err(|e| err(e.to_string()))?),
                SectionKind::IndexSelection { layer } => {
                    let v = s.to_i32().map_err(|e| err(e.to_string()))?;
                    selections.insert(
                        layer,
                        v.chunks(g.index_top_k as usize)
                            .map(<[i32]>::to_vec)
                            .collect(),
                    );
                }
                SectionKind::Candidates { .. } => {
                    let v = s.to_u32().map_err(|e| err(e.to_string()))?;
                    candidates = Some(
                        v.chunks(g.candidate_blocks as usize)
                            .map(<[u32]>::to_vec)
                            .collect(),
                    );
                }
                SectionKind::EngramHistory => {
                    before = s.to_u32().map_err(|e| err(e.to_string()))?;
                }
                _ => {}
            }
        }

        // Layers, each over the whole step, as the model runs them.
        let (hd, id) = (g.head_dim as usize, g.index_dim as usize);
        for layer in self.layers.clone() {
            if INDEX_SOURCES.contains(&layer) {
                selections.insert(layer, vec![Vec::new(); tokens]);
            }
            if layer == CANDIDATE_SOURCE {
                candidates = Some(vec![Vec::new(); tokens]);
            }
            for i in 0..tokens {
                let p = m.position as usize + i;
                let r = &mut residual[i * width..(i + 1) * width];
                if ENGRAM_LAYERS.contains(&layer) {
                    let mut gram: Vec<u32> = before.clone();
                    gram.extend_from_slice(&m.tokens[..=i]);
                    let gram = &gram[gram.len() - g.max_ngram as usize..];
                    let h = gram.iter().fold(7u64, |h, t| {
                        h.wrapping_mul(1_000_003).wrapping_add(u64::from(*t))
                    });
                    for (j, v) in r.iter_mut().enumerate() {
                        *v += ((h as usize + j * 7) % 13) as f32 * 0.01;
                    }
                }
                let ratio = compress_ratio(layer) as usize;
                if ratio > 0 && KV_SOURCES.contains(&layer) && (p + 1) % ratio == 0 {
                    let kv: Vec<f32> = (0..hd).map(|d| r[d] - 0.5 * r[hidden + d]).collect();
                    let key: Vec<f32> = (0..id).map(|d| r[2 * hidden + d] + 0.25 * r[d]).collect();
                    state.kv.entry(layer).or_default().extend(kv);
                    state.keys.entry(layer).or_default().extend(key);
                }
                if let Some(owner) = kv_owner(layer) {
                    let visible = (p + 1) / compress_ratio(owner) as usize;
                    let kv = state.kv.get(&owner).map_or(&[][..], Vec::as_slice);
                    if kv.len() < visible * hd {
                        return Err(err(format!("layer {layer} lacks layer {owner} KV")));
                    }
                    let selected: Vec<i32> = if INDEX_SOURCES.contains(&layer) {
                        let keys = state.keys.get(&owner).map_or(&[][..], Vec::as_slice);
                        if keys.len() < visible * id {
                            return Err(err(format!("layer {layer} lacks index keys")));
                        }
                        let query: Vec<f32> = (0..id).map(|d| r[3 * hidden + d]).collect();
                        let scores: Vec<(usize, f32)> = (0..visible)
                            .map(|j| {
                                let k = &keys[j * id..(j + 1) * id];
                                (j, k.iter().zip(&query).map(|(a, b)| a * b).sum())
                            })
                            .collect();
                        let rows = g.candidate_block_rows as usize;
                        if layer == CANDIDATE_SOURCE {
                            let blocks = visible.div_ceil(rows);
                            let block_scores: Vec<(usize, f32)> = (0..blocks)
                                .map(|b| {
                                    let best = scores[b * rows..((b + 1) * rows).min(visible)]
                                        .iter()
                                        .map(|s| s.1)
                                        .fold(f32::NEG_INFINITY, f32::max);
                                    (b, best)
                                })
                                .collect();
                            let mut ids: Vec<u32> =
                                top_by_score(&block_scores, g.candidate_blocks as usize)
                                    .into_iter()
                                    .map(|b| b as u32)
                                    .collect();
                            ids.resize(g.candidate_blocks as usize, PAD);
                            candidates.as_mut().unwrap()[i] = ids;
                        }
                        let allowed: Vec<(usize, f32)> = if layer > CANDIDATE_SOURCE {
                            let blocks =
                                &candidates.as_ref().ok_or_else(|| err("no candidates"))?[i];
                            scores
                                .into_iter()
                                .filter(|(j, _)| blocks.contains(&((j / rows) as u32)))
                                .collect()
                        } else {
                            scores
                        };
                        let mut picked: Vec<i32> = top_by_score(&allowed, g.index_top_k as usize)
                            .into_iter()
                            .map(|j| j as i32)
                            .collect();
                        picked.resize(g.index_top_k as usize, -1);
                        selections.get_mut(&layer).unwrap()[i] = picked.clone();
                        picked
                    } else {
                        let source = index_owner(layer).unwrap();
                        selections
                            .get(&source)
                            .and_then(|rows| rows.get(i))
                            .cloned()
                            .ok_or_else(|| err(format!("layer {layer} lacks a selection")))?
                    };
                    let mut attend = vec![0.0f32; hd];
                    for j in selected.into_iter().filter(|j| *j >= 0) {
                        let j = j as usize;
                        if j >= visible {
                            return Err(err("selection past causal KV"));
                        }
                        for (d, a) in attend.iter_mut().enumerate() {
                            *a += kv[j * hd + d];
                        }
                    }
                    for (j, v) in r.iter_mut().enumerate() {
                        *v += 0.01 * attend[j % hd];
                    }
                }
                let q = &mut pre[i * streams..(i + 1) * streams];
                let collapsed: f32 = (0..streams).map(|s| q[s] * r[s * hidden]).sum();
                for v in r.iter_mut() {
                    *v += 0.01 * collapsed;
                }
                for (s, w) in q.iter_mut().enumerate() {
                    *w = 0.5 * *w + 0.1 * r[s * hidden + layer as usize % hidden];
                }
                let copy = r.to_vec();
                for s in 0..streams {
                    for d in 0..hidden {
                        r[s * hidden + d] = copy[s * hidden + d] * 0.9
                            + 0.05 * copy[((s + 1) % streams) * hidden + d]
                            + 0.001 * layer as f32;
                    }
                }
            }
        }
        if first_stage {
            state.history.extend_from_slice(&m.tokens);
        }

        // Outbound state.
        let Some(next) = &self.next else {
            let vocab = g.vocabulary as usize;
            let mut logits = Vec::with_capacity(tokens * vocab);
            for i in 0..tokens {
                let r = &residual[i * width..(i + 1) * width];
                for v in 0..vocab {
                    logits.push(
                        r.iter()
                            .enumerate()
                            .map(|(j, x)| x * (((v * 31 + j * 3) % 11) as f32 - 5.0))
                            .sum(),
                    );
                }
            }
            return Ok(vec![
                Section::floats(
                    SectionKind::Logits,
                    WireDtype::F32,
                    vec![tokens as u32, g.vocabulary],
                    0,
                    &logits,
                )
                .map_err(|e| err(e.to_string()))?,
            ]);
        };
        let mut out = Vec::new();
        for spec in &next.sections {
            let section = match spec.kind {
                SectionKind::Residual => Section::floats(
                    spec.kind,
                    spec.dtype,
                    vec![tokens as u32, g.streams, g.hidden],
                    0,
                    &residual,
                ),
                SectionKind::PreMix => Section::floats(
                    spec.kind,
                    spec.dtype,
                    vec![tokens as u32, g.streams],
                    0,
                    &pre,
                ),
                SectionKind::SourceKv { layer } | SectionKind::SourceIndexKeys { layer } => {
                    let (table, w) = if matches!(spec.kind, SectionKind::SourceKv { .. }) {
                        (state.kv.get(&layer), hd)
                    } else {
                        (state.keys.get(&layer), id)
                    };
                    let table = table.map_or(&[][..], Vec::as_slice);
                    let sent = state.sent_rows.get(&spec.kind).copied().unwrap_or(0);
                    let total = (table.len() / w) as u64;
                    state.sent_rows.insert(spec.kind, total);
                    Section::floats(
                        spec.kind,
                        spec.dtype,
                        vec![(total - sent) as u32, w as u32],
                        sent,
                        &table[sent as usize * w..],
                    )
                }
                SectionKind::IndexSelection { layer } => {
                    let rows = selections
                        .get(&layer)
                        .ok_or_else(|| err("selection to forward"))?;
                    Section::i32s(
                        spec.kind,
                        vec![tokens as u32, g.index_top_k],
                        &rows.concat(),
                    )
                }
                SectionKind::Candidates { .. } => Section::u32s(
                    spec.kind,
                    vec![tokens as u32, g.candidate_blocks],
                    &candidates
                        .as_ref()
                        .ok_or_else(|| err("candidates to forward"))?
                        .concat(),
                ),
                SectionKind::EngramHistory => {
                    Section::u32s(spec.kind, vec![g.max_ngram - 1], &before)
                }
                other => return Err(err(format!("can't forward {other}"))),
            };
            out.push(section.map_err(|e| err(e.to_string()))?);
        }
        Ok(out)
    }

    fn reset(&mut self, request_id: &str) {
        self.requests.remove(request_id);
    }
}

// ---------------------------------------------------------------------------
// A group driver
// ---------------------------------------------------------------------------

const DIGEST: [u8; 32] = [7; 32];

struct Group {
    split: Vec<Range<u32>>,
    workers: Vec<StageWorker<SyntheticStage>>,
    /// Encoded messages delivered to each stage, in order.
    sent: Vec<Vec<Vec<u8>>>,
}

fn worker(g: V41StageGeometry, split: &[Range<u32>], k: usize) -> StageWorker<SyntheticStage> {
    let contract = stage_contract(&g, k as u16, split[k].clone());
    let next = split
        .get(k + 1)
        .map(|layers| stage_contract(&g, (k + 1) as u16, layers.clone()));
    StageWorker::new(
        StageInbox::new(DIGEST, 1, contract, CodecLimits::default()),
        SyntheticStage::new(g, split[k].clone(), next),
    )
}

impl Group {
    fn new(split: &[Range<u32>]) -> Self {
        let g = V41StageGeometry::synthetic();
        Self {
            split: split.to_vec(),
            workers: (0..split.len()).map(|k| worker(g, split, k)).collect(),
            sent: vec![Vec::new(); split.len()],
        }
    }

    fn step(
        &mut self,
        request: &str,
        step: u64,
        kind: StepKind,
        position: u64,
        tokens: Vec<u32>,
    ) -> Vec<f32> {
        let mut message = StageMessage {
            manifest_digest: DIGEST,
            generation: 1,
            request_id: request.to_owned(),
            attempt: 0,
            stage: 0,
            layers: self.split[0].clone(),
            step,
            kind,
            position,
            tokens,
            sections: Vec::new(),
        };
        for k in 0..self.split.len() {
            let bytes = encode(&message, &CodecLimits::default()).unwrap();
            self.sent[k].push(bytes.clone());
            let out = self.workers[k].step_bytes(&bytes).unwrap();
            if k + 1 == self.split.len() {
                return out[0].to_f32().unwrap();
            }
            message = message.next_hop((k + 1) as u16, self.split[k + 1].clone(), out);
        }
        unreachable!()
    }

    /// A seven-token prompt, then greedy decode.
    fn generate(&mut self, request: &str, decode_steps: u64) -> (Vec<u32>, Vec<f32>) {
        let prompt = vec![3, 1, 4, 1, 5, 9, 2];
        let mut logits = self.step(request, 0, StepKind::Prefill, 0, prompt.clone());
        let mut tokens = prompt;
        for step in 1..=decode_steps {
            let vocab = V41StageGeometry::synthetic().vocabulary as usize;
            let last = &logits[logits.len() - vocab..];
            let next =
                top_by_score(&last.iter().copied().enumerate().collect::<Vec<_>>(), 1)[0] as u32;
            let position = tokens.len() as u64;
            tokens.push(next);
            logits = self.step(request, step, StepKind::Decode, position, vec![next]);
        }
        (tokens, logits)
    }
}

fn even(n: u32) -> Vec<Range<u32>> {
    let per = V41_LAYERS / n;
    (0..n).map(|k| k * per..(k + 1) * per).collect()
}

const ODD_SPLIT: [Range<u32>; 7] = [0..3, 3..9, 9..15, 15..21, 21..25, 25..37, 37..40];

fn bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|v| v.to_bits()).collect()
}

// ---------------------------------------------------------------------------
// Topology and contracts
// ---------------------------------------------------------------------------

#[test]
fn topology_matches_the_cpu_port() {
    use crate::deepseek_v41_cpu::{projection, shared_state};
    assert_eq!(
        KV_SOURCES.map(|l| l as usize),
        shared_state::KV_SOURCES,
        "KV sources"
    );
    assert_eq!(
        INDEX_SOURCES.map(|l| l as usize),
        shared_state::INDEX_SOURCES
    );
    for layer in 0..V41_LAYERS {
        assert_eq!(
            compress_ratio(layer) as usize,
            projection::compress_ratio(layer as usize).unwrap()
        );
        let owners = shared_state::LayerOwners::for_layer(layer as usize).unwrap();
        assert_eq!(owners.kv, kv_owner(layer).map(|l| l as usize));
        assert_eq!(owners.index, index_owner(layer).map(|l| l as usize));
    }
}

fn kinds(boundary: u32) -> Vec<SectionKind> {
    boundary_sections(&V41StageGeometry::flash(), boundary)
        .into_iter()
        .map(|s| s.kind)
        .collect()
}

#[test]
fn boundaries_carry_what_later_layers_read() {
    use SectionKind::*;
    assert!(kinds(0).is_empty());
    assert_eq!(kinds(1), [Residual, PreMix, EngramHistory]);
    assert_eq!(
        kinds(3),
        [
            Residual,
            PreMix,
            SourceKv { layer: 2 },
            IndexSelection { layer: 2 },
            EngramHistory
        ]
    );
    assert_eq!(
        kinds(10),
        [
            Residual,
            PreMix,
            SourceKv { layer: 8 },
            IndexSelection { layer: 8 },
            EngramHistory
        ]
    );
    assert_eq!(kinds(14), [Residual, PreMix, EngramHistory]);
    assert_eq!(
        kinds(15),
        [
            Residual,
            PreMix,
            SourceKv { layer: 14 },
            IndexSelection { layer: 14 }
        ]
    );
    // Layer 20 produces everything layers 20-39 read.
    assert_eq!(kinds(20), [Residual, PreMix]);
    assert_eq!(
        kinds(30),
        [
            Residual,
            PreMix,
            SourceKv { layer: 20 },
            SourceIndexKeys { layer: 20 },
            Candidates { layer: 20 },
            IndexSelection { layer: 28 }
        ]
    );
    assert_eq!(
        kinds(24),
        [
            Residual,
            PreMix,
            SourceKv { layer: 20 },
            SourceIndexKeys { layer: 20 },
            Candidates { layer: 20 }
        ]
    );
    // Past the last index source, only the KV and the selection remain.
    assert_eq!(
        kinds(37),
        [
            Residual,
            PreMix,
            SourceKv { layer: 20 },
            IndexSelection { layer: 36 }
        ]
    );
}

// ---------------------------------------------------------------------------
// Split equivalence
// ---------------------------------------------------------------------------

fn reference_run() -> (Vec<u32>, Vec<f32>) {
    Group::new(&[0..40]).generate("ref", 14)
}

#[test]
fn every_split_matches_the_unsplit_run() {
    let (tokens, logits) = reference_run();
    assert!(logits.iter().all(|v| v.is_finite()));
    let one_per_layer: Vec<Range<u32>> = (0..V41_LAYERS).map(|l| l..l + 1).collect();
    for split in [even(4), even(8), ODD_SPLIT.to_vec(), one_per_layer] {
        let (t, l) = Group::new(&split).generate("ref", 14);
        assert_eq!(t, tokens, "tokens for {split:?}");
        assert_eq!(bits(&l), bits(&logits), "logits for {split:?}");
    }
}

#[test]
fn the_synthetic_model_reads_every_section_it_receives() {
    // Corrupting any one section in transit changes the answer, so each
    // section the contract carries is state the later layers really read.
    let (_, reference) = reference_run();
    let mut seen = Vec::new();
    for k in 1..ODD_SPLIT.len() {
        for spec in boundary_sections(&V41StageGeometry::synthetic(), ODD_SPLIT[k].start) {
            let mut group = Group::new(&ODD_SPLIT);
            let mut corrupt = |m: &mut StageMessage| {
                let s = m.sections.iter_mut().find(|s| s.kind == spec.kind).unwrap();
                match s.kind {
                    SectionKind::EngramHistory => s.data.iter_mut().for_each(|b| *b = 0),
                    SectionKind::Candidates { .. } => {
                        s.data = Section::u32s(s.kind, s.shape.clone(), &vec![0; s.elements()])
                            .unwrap()
                            .data;
                    }
                    SectionKind::IndexSelection { .. } => {
                        s.data = Section::i32s(s.kind, s.shape.clone(), &vec![-1; s.elements()])
                            .unwrap()
                            .data;
                    }
                    _ => {
                        let v: Vec<f32> = s
                            .to_f32()
                            .unwrap()
                            .iter()
                            .enumerate()
                            .map(|(j, x)| x + (j % 7) as f32)
                            .collect();
                        s.data = Section::floats(s.kind, s.dtype, s.shape.clone(), 0, &v)
                            .unwrap()
                            .data;
                    }
                }
            };
            let logits = run_with_tamper(&mut group, k, &mut corrupt);
            assert_ne!(
                bits(&logits),
                bits(&reference),
                "{} into stage {k} made no difference",
                spec.kind
            );
            seen.push(spec.kind);
        }
    }
    for kind in [
        SectionKind::Residual,
        SectionKind::SourceKv { layer: 2 },
        SectionKind::SourceKv { layer: 20 },
        SectionKind::SourceIndexKeys { layer: 20 },
        SectionKind::Candidates { layer: 20 },
        SectionKind::IndexSelection { layer: 36 },
        SectionKind::EngramHistory,
    ] {
        assert!(seen.contains(&kind), "{kind} not exercised");
    }
}

/// Runs the reference prompt and decode, applying `tamper` to every message
/// into stage `target`.
fn run_with_tamper(
    group: &mut Group,
    target: usize,
    tamper: &mut dyn FnMut(&mut StageMessage),
) -> Vec<f32> {
    let (tokens, _) = reference_run();
    let mut logits = Vec::new();
    for (step, (position, chunk)) in [(0usize, tokens[..7].to_vec())]
        .into_iter()
        .chain((7..tokens.len()).map(|p| (p, vec![tokens[p]])))
        .enumerate()
    {
        let mut message = StageMessage {
            manifest_digest: DIGEST,
            generation: 1,
            request_id: String::from("ref"),
            attempt: 0,
            stage: 0,
            layers: group.split[0].clone(),
            step: step as u64,
            kind: if step == 0 {
                StepKind::Prefill
            } else {
                StepKind::Decode
            },
            position: position as u64,
            tokens: chunk,
            sections: Vec::new(),
        };
        for k in 0..group.split.len() {
            if k == target {
                tamper(&mut message);
            }
            let out = group.workers[k].step(message.clone()).unwrap();
            if k + 1 == group.split.len() {
                logits = out[0].to_f32().unwrap();
                break;
            }
            message = message.next_hop((k + 1) as u16, group.split[k + 1].clone(), out);
        }
    }
    logits
}

// ---------------------------------------------------------------------------
// Faults
// ---------------------------------------------------------------------------

/// Messages a four-stage group sent to stage 2 (layers 20-29) and stage 3
/// (layers 30-39) over a prefill and six decode steps.
fn recorded() -> Group {
    let mut group = Group::new(&even(4));
    group.generate("a", 6);
    group
}

fn fresh(k: usize) -> StageWorker<SyntheticStage> {
    worker(V41StageGeometry::synthetic(), &even(4), k)
}

fn refusal(
    worker: &mut StageWorker<SyntheticStage>,
    bytes: &[u8],
) -> crate::stage_state::StageRefusal {
    let before = (worker.inbox().summary(), worker.executor().steps_run);
    match worker.step_bytes(bytes) {
        Err(WorkerError::Refused(refusal)) => {
            assert_eq!(
                (worker.inbox().summary(), worker.executor().steps_run),
                before,
                "a refusal changed state"
            );
            refusal
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn feed(worker: &mut StageWorker<SyntheticStage>, bytes: &[u8]) -> Vec<Section> {
    worker.step_bytes(bytes).unwrap()
}

#[test]
fn a_duplicate_step_is_refused_before_it_runs() {
    let group = recorded();
    let mut w = fresh(3);
    feed(&mut w, &group.sent[3][0]);
    feed(&mut w, &group.sent[3][1]);
    assert_eq!(
        refusal(&mut w, &group.sent[3][1]),
        StageRefusal::Duplicate { step: 1 }
    );
    assert_eq!(
        refusal(&mut w, &group.sent[3][0]),
        StageRefusal::Duplicate { step: 0 }
    );
    feed(&mut w, &group.sent[3][2]);
}

#[test]
fn a_missing_step_is_refused() {
    let group = recorded();
    let mut w = fresh(3);
    feed(&mut w, &group.sent[3][0]);
    assert_eq!(
        refusal(&mut w, &group.sent[3][2]),
        StageRefusal::OutOfOrder {
            expected: 1,
            got: 2
        }
    );
}

#[test]
fn reordered_steps_are_refused_and_the_order_still_matches_afterward() {
    let group = recorded();
    let mut w = fresh(3);
    assert_eq!(
        refusal(&mut w, &group.sent[3][1]),
        StageRefusal::OutOfOrder {
            expected: 0,
            got: 1
        }
    );
    let mut last = Vec::new();
    for (i, bytes) in group.sent[3].iter().enumerate() {
        if i == 3 {
            assert!(matches!(
                refusal(&mut w, &group.sent[3][4]),
                StageRefusal::OutOfOrder {
                    expected: 3,
                    got: 4
                }
            ));
        }
        last = feed(&mut w, bytes);
    }
    // The stage that refused out-of-order steps answers as the clean run did.
    let mut clean = fresh(3);
    let mut expected = Vec::new();
    for bytes in &group.sent[3] {
        expected = feed(&mut clean, bytes);
    }
    assert_eq!(last, expected);
}

#[test]
fn a_cross_request_message_is_refused() {
    let mut group = recorded();
    for w in &mut group.workers {
        w.reset("a", 0);
    }
    group.generate("b", 1);
    let mut w = fresh(2);
    feed(&mut w, &group.sent[2][0]);
    let b0 = &group.sent[2][7];
    assert_eq!(decode(b0, &CodecLimits::default()).unwrap().request_id, "b");
    assert_eq!(
        refusal(&mut w, b0),
        StageRefusal::CrossRequest {
            active: String::from("a"),
            got: String::from("b")
        }
    );
}

#[test]
fn stale_and_future_generations_are_refused() {
    let group = recorded();
    let mut w = fresh(2);
    feed(&mut w, &group.sent[2][0]);
    w.advance_generation(2).unwrap();
    assert_eq!(
        refusal(&mut w, &group.sent[2][1]),
        StageRefusal::StaleGeneration { current: 2, got: 1 }
    );
    let mut later = decode(&group.sent[2][0], &CodecLimits::default()).unwrap();
    later.generation = 3;
    assert_eq!(
        refusal(&mut w, &encode(&later, &CodecLimits::default()).unwrap()),
        StageRefusal::FutureGeneration { current: 2, got: 3 }
    );
    assert_eq!(
        w.advance_generation(2),
        Err(StageRefusal::StaleGeneration { current: 2, got: 2 })
    );
}

#[test]
fn a_new_generation_drops_the_open_request() {
    let group = recorded();
    let mut w = fresh(2);
    feed(&mut w, &group.sent[2][0]);
    feed(&mut w, &group.sent[2][1]);
    w.advance_generation(2).unwrap();
    assert_eq!(w.inbox().summary().active, None);
    assert!(w.executor().requests.is_empty());
    // A step that continues the old cache under the new generation is
    // refused: the request must start over.
    let mut resumed = decode(&group.sent[2][2], &CodecLimits::default()).unwrap();
    resumed.generation = 2;
    assert_eq!(
        refusal(&mut w, &encode(&resumed, &CodecLimits::default()).unwrap()),
        StageRefusal::OutOfOrder {
            expected: 0,
            got: 2
        }
    );
}

#[test]
fn a_late_message_after_reset_is_refused() {
    let group = recorded();
    let mut w = fresh(3);
    feed(&mut w, &group.sent[3][0]);
    feed(&mut w, &group.sent[3][1]);
    w.reset("a", 0);
    assert!(w.executor().requests.is_empty());
    // A late step, and a replay of the first step, of the ended attempt.
    for bytes in [&group.sent[3][2], &group.sent[3][0]] {
        assert_eq!(
            refusal(&mut w, bytes),
            StageRefusal::StaleAttempt {
                request_id: String::from("a"),
                attempt: 0
            }
        );
    }
    // The next attempt starts cleanly.
    let mut retry = decode(&group.sent[3][0], &CodecLimits::default()).unwrap();
    retry.attempt = 1;
    w.step(retry.clone()).unwrap();
    // An older attempt is stale; a newer one waits for a reset.
    let mut old = decode(&group.sent[3][1], &CodecLimits::default()).unwrap();
    old.attempt = 0;
    assert!(matches!(
        refusal(&mut w, &encode(&old, &CodecLimits::default()).unwrap()),
        StageRefusal::StaleAttempt { attempt: 0, .. }
    ));
    retry.attempt = 2;
    assert_eq!(
        refusal(&mut w, &encode(&retry, &CodecLimits::default()).unwrap()),
        StageRefusal::AttemptOpen { current: 1, got: 2 }
    );
}

#[test]
fn identity_and_contract_violations_are_refused() {
    let group = recorded();
    let limits = CodecLimits::default();
    let base = decode(&group.sent[3][0], &limits).unwrap();
    let cases: Vec<(Box<dyn Fn(&mut StageMessage)>, fn(&StageRefusal) -> bool)> = vec![
        (Box::new(|m| m.manifest_digest[0] ^= 1), |r| {
            *r == StageRefusal::WrongManifest
        }),
        (Box::new(|m| m.stage = 2), |r| {
            matches!(r, StageRefusal::WrongStage { .. })
        }),
        (Box::new(|m| m.layers = 30..39), |r| {
            matches!(r, StageRefusal::WrongStage { .. })
        }),
        (Box::new(|m| m.position = 1), |r| {
            *r == StageRefusal::PositionMismatch {
                expected: 0,
                got: 1,
            }
        }),
        (Box::new(|m| m.kind = StepKind::Decode), |r| {
            matches!(r, StageRefusal::Step(_))
        }),
        (
            Box::new(|m| m.sections.retain(|s| s.kind != SectionKind::Residual)),
            |r| *r == StageRefusal::MissingSection(SectionKind::Residual),
        ),
        (
            Box::new(|m| {
                m.sections
                    .push(Section::u32s(SectionKind::EngramHistory, vec![3], &[0, 0, 0]).unwrap());
            }),
            |r| *r == StageRefusal::UnexpectedSection(SectionKind::EngramHistory),
        ),
        (
            Box::new(|m| {
                let s = &mut m.sections[0];
                let v = s.to_f32().unwrap();
                *s = Section::floats(s.kind, WireDtype::Bf16, s.shape.clone(), 0, &v).unwrap();
            }),
            |r| matches!(r, StageRefusal::Section { .. }),
        ),
        (
            Box::new(|m| {
                let s = &mut m.sections[0];
                let v = s.to_f32().unwrap();
                let n = v.len() as u32;
                *s = Section::floats(s.kind, s.dtype, vec![n], 0, &v).unwrap();
            }),
            |r| matches!(r, StageRefusal::Section { .. }),
        ),
        (
            Box::new(|m| {
                let s = m
                    .sections
                    .iter_mut()
                    .find(|s| s.kind == SectionKind::SourceKv { layer: 20 })
                    .unwrap();
                s.first_row = 1;
            }),
            |r| {
                *r == StageRefusal::RowGap {
                    kind: SectionKind::SourceKv { layer: 20 },
                    expected: 0,
                    got: 1,
                }
            },
        ),
        (
            Box::new(|m| {
                // One fewer KV row than layer 20 has produced.
                let s = m
                    .sections
                    .iter_mut()
                    .find(|s| s.kind == SectionKind::SourceKv { layer: 20 })
                    .unwrap();
                let width = s.shape[1] as usize;
                let v = s.to_f32().unwrap();
                let v = &v[..v.len() - width];
                *s = Section::floats(s.kind, s.dtype, vec![s.shape[0] - 1, s.shape[1]], 0, v)
                    .unwrap();
            }),
            |r| matches!(r, StageRefusal::Section { .. }),
        ),
    ];
    for (i, (mutate, expected)) in cases.into_iter().enumerate() {
        let mut w = fresh(3);
        let mut m = base.clone();
        mutate(&mut m);
        let r = refusal(&mut w, &encode(&m, &limits).unwrap());
        assert!(expected(&r), "case {i}: {r:?}");
    }
}

#[test]
fn malformed_and_oversize_bytes_are_refused() {
    let group = recorded();
    let good = group.sent[3][0].clone();
    let limits = CodecLimits::default();
    let header = crate::stage_state::MESSAGE_HEADER_BYTES;
    let mut cases: Vec<(&str, Vec<u8>)> = Vec::new();
    cases.push(("empty", Vec::new()));
    cases.push(("truncated", good[..good.len() - 1].to_vec()));
    cases.push(("trailing", [good.clone(), vec![0]].concat()));
    let mut magic = good.clone();
    magic[0] = b'X';
    cases.push(("magic", magic));
    let mut version = good.clone();
    version[4] = 9;
    cases.push(("version", version));
    let mut flags = good.clone();
    flags[6] = 1;
    cases.push(("flags", flags));
    let mut step_kind = good.clone();
    step_kind[4 + 2 + 2 + 32 + 8 + 4 + 2] = 9;
    cases.push(("step kind", step_kind));
    // The first section header follows the request ID and the tokens.
    let m = decode(&good, &limits).unwrap();
    let first_section = header + m.request_id.len() + 4 * m.tokens.len();
    let mut tag = good.clone();
    tag[first_section] = 99;
    cases.push(("section tag", tag));
    let mut dtype = good.clone();
    dtype[first_section + 1] = 9;
    cases.push(("dtype", dtype));
    let mut rank = good.clone();
    rank[first_section + 2] = 0;
    cases.push(("rank", rank));
    let mut length = good.clone();
    length[first_section + 16..first_section + 24].copy_from_slice(&u64::MAX.to_le_bytes());
    cases.push(("section length", length));
    let mut shape = good.clone();
    shape[first_section + SECTION_HEADER_BYTES_FOR_TEST] ^= 1;
    cases.push(("shape", shape));
    let mut token_count = good.clone();
    token_count[header - 6..header - 2].copy_from_slice(&u32::MAX.to_le_bytes());
    cases.push(("token count", token_count));
    let mut doubled = m.clone();
    doubled.sections.push(doubled.sections[0].clone());
    let mut bytes = encode(&m, &limits).unwrap();
    let count = doubled.sections.len() as u16;
    bytes[header - 2..header].copy_from_slice(&count.to_le_bytes());
    bytes.extend_from_slice(&good[first_section..first_section + m.sections[0].wire_bytes()]);
    cases.push(("two residual sections", bytes));
    for (name, bytes) in cases {
        let mut w = fresh(3);
        let r = refusal(&mut w, &bytes);
        assert!(
            matches!(
                r,
                StageRefusal::Malformed(_)
                    | StageRefusal::UnsupportedVersion(_)
                    | StageRefusal::Oversize { .. }
            ),
            "{name}: {r:?}"
        );
    }
    assert!(encode(&doubled, &limits).is_err());

    // Each limit is enforced on both sides.
    let tight = CodecLimits {
        max_message_bytes: good.len() - 1,
        ..limits
    };
    assert!(matches!(
        decode(&good, &tight),
        Err(StageRefusal::Oversize {
            what: "message bytes",
            ..
        })
    ));
    assert!(matches!(
        encode(&m, &tight),
        Err(StageRefusal::Oversize { .. })
    ));
    let tight = CodecLimits {
        max_section_bytes: 16,
        ..limits
    };
    assert!(matches!(
        decode(&good, &tight),
        Err(StageRefusal::Oversize {
            what: "section bytes",
            ..
        })
    ));
    let tight = CodecLimits {
        max_request_id_bytes: 0,
        ..limits
    };
    assert!(matches!(
        decode(&good, &tight),
        Err(StageRefusal::Oversize { .. })
    ));
    let mut w = StageWorker::new(
        StageInbox::new(
            DIGEST,
            1,
            stage_contract(&V41StageGeometry::synthetic(), 3, 30..40),
            CodecLimits {
                max_tokens: 6,
                ..limits
            },
        ),
        SyntheticStage::new(V41StageGeometry::synthetic(), 30..40, None),
    );
    assert!(matches!(
        refusal(&mut w, &good),
        StageRefusal::Oversize { what: "tokens", .. }
    ));
}

// Section header bytes before the shape.
const SECTION_HEADER_BYTES_FOR_TEST: usize = crate::stage_state::SECTION_HEADER_BYTES;

#[test]
fn the_codec_round_trips_and_widens_bf16() {
    let group = recorded();
    let limits = CodecLimits::default();
    for stage in &group.sent {
        for bytes in stage {
            let m = decode(bytes, &limits).unwrap();
            assert_eq!(&encode(&m, &limits).unwrap(), bytes);
            assert_eq!(m.wire_bytes(), bytes.len());
        }
    }
    assert_eq!(&group.sent[1][0][..4], &STAGE_STATE_MAGIC);
    let values = [1.0f32, -2.5, 3.140_625, 0.0];
    let s = Section::floats(
        SectionKind::Residual,
        WireDtype::Bf16,
        vec![1, 1, 4],
        0,
        &values,
    )
    .unwrap();
    assert_eq!(s.data.len(), 8);
    assert_eq!(s.to_f32().unwrap(), values);
    assert!(Section::floats(SectionKind::Residual, WireDtype::F32, vec![3], 0, &values).is_err());
    assert!(s.to_i32().is_err());
}

#[test]
fn an_executor_failure_ends_the_attempt() {
    let group = recorded();
    let mut w = fresh(3);
    let mut m = decode(&group.sent[3][0], &CodecLimits::default()).unwrap();
    // Admissible shapes, but a selection past causal KV.
    let s = m
        .sections
        .iter_mut()
        .find(|s| matches!(s.kind, SectionKind::IndexSelection { .. }))
        .unwrap();
    *s = Section::i32s(s.kind, s.shape.clone(), &vec![1000; s.elements()]).unwrap();
    assert!(matches!(w.step(m), Err(WorkerError::Executor(_))));
    assert_eq!(w.inbox().summary().active, None);
    assert!(matches!(
        refusal(&mut w, &group.sent[3][1]),
        StageRefusal::StaleAttempt { .. }
    ));
}

#[test]
fn inbox_summary_tracks_appended_rows() {
    let group = recorded();
    let mut w = fresh(3);
    for bytes in &group.sent[3] {
        feed(&mut w, bytes);
    }
    let InboxSummary {
        active,
        rows,
        admitted,
        ..
    } = w.inbox().summary();
    // Seven prompt tokens and six decode tokens.
    assert_eq!(active, Some((String::from("a"), 0, 7, 13)));
    assert_eq!(admitted, 7);
    // Layer 20 has ratio 1: one row per position.
    assert_eq!(rows[&SectionKind::SourceKv { layer: 20 }], 13);
    assert_eq!(rows[&SectionKind::SourceIndexKeys { layer: 20 }], 13);
}

// ---------------------------------------------------------------------------
// Wire bytes
// ---------------------------------------------------------------------------

#[test]
fn wire_bytes_per_token_for_both_geometries() {
    let flash = V41StageGeometry::flash();
    let synthetic = V41StageGeometry::synthetic();
    let mut reports = Vec::new();
    for (name, g) in [
        ("synthetic", synthetic),
        ("flash bf16", flash),
        (
            "flash f32",
            flash.with_dtypes(WireDtype::F32, WireDtype::F32),
        ),
    ] {
        for split in [even(4), even(8)] {
            let report = wire_report(&g, &split, 36).unwrap();
            for (b, k) in report.boundaries.iter().zip(1..) {
                let contract = stage_contract(&g, k as u16, split[k].clone());
                assert_eq!(b.peak_bytes, contract.decode_step_bytes(36, 1));
                assert_eq!(
                    b.mean_bytes,
                    (b.peak_bytes + contract.decode_step_bytes(36, 2)) as f64 / 2.0
                );
            }
            eprintln!(
                "{name}, {} stages: {}",
                split.len(),
                serde_json::to_string(&report).unwrap()
            );
            reports.push((name, split.len(), report));
        }
    }
    let header = crate::stage_state::MESSAGE_HEADER_BYTES + 36 + 4;
    let section =
        |shape: usize, bytes: usize| crate::stage_state::SECTION_HEADER_BYTES + 4 * shape + bytes;
    // Flash, BF16, four stages: boundaries at 10, 20, and 30.
    let r = &reports[2].2;
    // The residual streams (BF16) and the pre-mix (F32) cross every boundary.
    let residual = section(3, 4 * 5120 * 2) + section(2, 4 * 4);
    assert_eq!(residual, 40_996 + 48);
    let at10 = header + residual + section(2, 512 * 2) + section(2, 512 * 4) + section(1, 3 * 4);
    let at20 = header + residual;
    let at30 = header
        + residual
        + section(2, 512 * 2)
        + section(2, 128 * 2)
        + section(2, 2048 * 4)
        + section(2, 512 * 4);
    assert_eq!(
        r.boundaries
            .iter()
            .map(|b| b.peak_bytes)
            .collect::<Vec<_>>(),
        [at10, at20, at30]
    );
    assert_eq!(r.boundaries[1].peak_bytes, 41_172);
    let logits = header + section(2, 129_280 * 4);
    assert_eq!(r.logits_return_bytes, logits);
    assert_eq!(r.token_peak_bytes, at10 + at20 + at30 + logits);
    // At ratio 2, layer 8's KV adds a row every other token.
    assert_eq!(
        r.boundaries[0].mean_bytes,
        (at10 + at10 - 512 * 2) as f64 / 2.0
    );
    // Synthetic, four stages.
    let r = &reports[0].2;
    let residual = section(3, 4 * 8 * 4) + section(2, 4 * 4);
    assert_eq!(
        r.boundaries
            .iter()
            .map(|b| b.peak_bytes)
            .collect::<Vec<_>>(),
        [
            header + residual + section(2, 4 * 4) + section(2, 2 * 4) + section(1, 12),
            header + residual,
            header
                + residual
                + section(2, 4 * 4)
                + section(2, 2 * 4)
                + section(2, 2 * 4)
                + section(2, 2 * 4),
        ]
    );
}

// ---------------------------------------------------------------------------
// Manifest and placement
// ---------------------------------------------------------------------------

const GB: u64 = 1_000_000_000;

fn gpus(hosts: &[&str], per_host: u32, bytes: u64) -> Vec<GpuSlot> {
    hosts
        .iter()
        .flat_map(|h| {
            (0..per_host).map(move |gpu| GpuSlot {
                host: (*h).to_owned(),
                gpu,
                bytes,
            })
        })
        .collect()
}

fn ram(hosts: &[&str], bytes: u64) -> Vec<HostRam> {
    hosts
        .iter()
        .map(|h| HostRam {
            host: (*h).to_owned(),
            bytes,
        })
        .collect()
}

const RESERVE: RuntimeReserve = RuntimeReserve {
    per_stage_bytes: 6 * GB,
    per_layer_bytes: GB / 2,
};

#[test]
fn header_bytes_reproduce_the_plan_four_stage_table() {
    let memory = HeaderBytes::flash(RuntimeReserve {
        per_stage_bytes: 0,
        per_layer_bytes: 0,
    })
    .unwrap();
    let total: u64 = (0..V41_LAYERS)
        .map(|l| memory.layer_gpu_bytes(l))
        .sum::<u64>()
        + memory.embeddings_bytes()
        + memory.head_bytes();
    assert_eq!(total, 298_624_579_008);
    let (candidate, manifest) = evaluate_split(
        &memory,
        &gpus(&["a", "b"], 2, 96 * GB),
        &ram(&["a", "b"], 256 * GB),
        &even(4),
    );
    assert_eq!(candidate.outcome, CandidateOutcome::Fits);
    assert_eq!(
        candidate.gpu_bytes,
        [
            75_408_745_584,
            74_068_724_592,
            73_917_111_152,
            75_229_997_680
        ]
    );
    let manifest = manifest.unwrap();
    manifest.validate().unwrap();
    let engram: Vec<(u32, u16, u64)> = manifest
        .stages
        .iter()
        .flat_map(|s| s.engram.iter().map(move |e| (e.layer, s.stage, e.bytes)))
        .collect();
    assert_eq!(engram, [(1, 0, 101_377_628_352), (14, 1, 101_380_404_048)]);
    // Stages 0 and 1 share host "a", which must hold both tables.
    let (short, _) = evaluate_split(
        &memory,
        &gpus(&["a", "b"], 2, 96 * GB),
        &ram(&["a", "b"], 150 * GB),
        &even(4),
    );
    assert_eq!(
        short.outcome,
        CandidateOutcome::HostShort {
            host: String::from("a"),
            need: 202_758_032_400,
            have: 150 * GB
        }
    );
}

#[test]
fn four_stages_are_evaluated_first_and_win_when_they_fit() {
    let memory = HeaderBytes::flash(RESERVE).unwrap();
    let request = PlacementRequest::new(
        gpus(&["a", "b", "c", "d"], 1, 96 * GB),
        ram(&["a", "b", "c", "d"], 128 * GB),
    );
    let placement = plan_group(&memory, &request).unwrap();
    assert_eq!(placement.candidates.len(), 1);
    assert_eq!(placement.candidates[0].stages, 4);
    let m = &placement.manifest;
    m.validate().unwrap();
    assert_eq!(m.memory_source, "header_bytes");
    assert_eq!(m.stages.len(), 4);
    assert_eq!(
        m.stage_of_layer(1).unwrap().engram[0].location,
        EngramLocation::HostMemory
    );
    assert_eq!(m.stage_of_layer(14).unwrap().engram[0].layer, 14);
    for s in &m.stages {
        assert!(s.gpu_weight_bytes + s.gpu_runtime_bytes <= s.gpu_capacity_bytes);
    }
    eprintln!(
        "four-stage plan {}: {:?}",
        m.digest_hex(),
        placement.candidates[0]
    );
    // The selection and contract come from the manifest.
    let selection = m.selection(0).unwrap();
    assert!(selection.embeddings && !selection.head && !selection.mtp);
    assert_eq!(
        m.contract(&V41StageGeometry::flash(), 2).unwrap().layers,
        m.stages[2].layers()
    );
}

#[test]
fn eight_stages_are_the_fallback_and_nothing_fitting_is_refused() {
    let memory = HeaderBytes::flash(RESERVE).unwrap();
    let hosts = ["a", "b", "c", "d"];
    let request = PlacementRequest::new(gpus(&hosts, 2, 64 * GB), ram(&hosts, 128 * GB));
    let placement = plan_group(&memory, &request).unwrap();
    assert_eq!(placement.candidates.len(), 2);
    assert!(matches!(
        placement.candidates[0].outcome,
        CandidateOutcome::GpuShort { .. }
    ));
    assert_eq!(placement.candidates[0].stages, 4);
    assert_eq!(placement.candidates[1].outcome, CandidateOutcome::Fits);
    assert_eq!(placement.manifest.stages.len(), 8);
    placement.manifest.validate().unwrap();

    // 24 GB GPUs can't hold five layers.
    let refused = plan_group(
        &memory,
        &PlacementRequest::new(gpus(&hosts, 2, 24 * GB), ram(&hosts, 512 * GB)),
    )
    .unwrap_err();
    assert_eq!(refused.candidates.len(), 2);
    assert!(
        refused
            .candidates
            .iter()
            .all(|c| matches!(c.outcome, CandidateOutcome::GpuShort { .. }))
    );

    // Enough GPU memory but no room for the Engram tables.
    let refused = plan_group(
        &memory,
        &PlacementRequest::new(gpus(&hosts, 1, 96 * GB), ram(&hosts, 64 * GB)),
    )
    .unwrap_err();
    assert!(matches!(
        refused.candidates[0].outcome,
        CandidateOutcome::HostShort { .. }
    ));
    assert!(matches!(
        refused.candidates[1].outcome,
        CandidateOutcome::NotEnoughGpus { have: 4, need: 8 }
    ));
    assert!(refused.to_string().starts_with("no placement fits"));
}

#[test]
fn a_manifest_names_its_digest_and_refuses_bad_plans() {
    let memory = HeaderBytes::flash(RESERVE).unwrap();
    let hosts = ["a", "b", "c", "d"];
    let m = plan_group(
        &memory,
        &PlacementRequest::new(gpus(&hosts, 1, 96 * GB), ram(&hosts, 128 * GB)),
    )
    .unwrap()
    .manifest;
    let json = serde_json::to_string(&m).unwrap();
    let back: V41GroupManifest = serde_json::from_str(&json).unwrap();
    assert_eq!(back.digest(), m.digest());
    let mut moved = m.clone();
    moved.stages[1].gpu = 1;
    assert_ne!(moved.digest(), m.digest());

    let broken: Vec<fn(&mut V41GroupManifest)> = vec![
        |m| m.schema = String::from("other"),
        |m| m.stages[1].first_layer += 1,
        |m| m.stages[3].end_layer = 39,
        |m| m.stages[1].embeddings = true,
        |m| m.stages[3].head = false,
        |m| m.stages[0].engram.clear(),
        |m| {
            let e = m.stages[0].engram.remove(0);
            m.stages[2].engram.push(e);
        },
        |m| m.stages[1].engram[0].host = String::from("z"),
        |m| m.stages[2].gpu_capacity_bytes = GB,
        |m| m.stages.swap(1, 2),
    ];
    for (i, f) in broken.into_iter().enumerate() {
        let mut bad = m.clone();
        f(&mut bad);
        assert!(bad.validate().is_err(), "case {i}");
    }
}

#[test]
fn a_measured_input_plugs_into_the_same_planner() {
    // A stand-in for W3's measured memory: every layer 9 GB and a large
    // reserve, which pushes the plan from four stages to eight.
    struct Measured;
    impl PlacementMemory for Measured {
        fn source(&self) -> String {
            String::from("measured")
        }
        fn layer_gpu_bytes(&self, _: u32) -> u64 {
            9 * GB
        }
        fn layer_engram_bytes(&self, layer: u32) -> u64 {
            if ENGRAM_LAYERS.contains(&layer) {
                100 * GB
            } else {
                0
            }
        }
        fn embeddings_bytes(&self) -> u64 {
            GB
        }
        fn head_bytes(&self) -> u64 {
            GB
        }
        fn stage_runtime_bytes(&self, _: Range<u32>) -> u64 {
            10 * GB
        }
    }
    let hosts = ["a", "b", "c", "d"];
    let placement = plan_group(
        &Measured,
        &PlacementRequest::new(gpus(&hosts, 2, 96 * GB), ram(&hosts, 256 * GB)),
    )
    .unwrap();
    assert_eq!(placement.manifest.memory_source, "measured");
    assert_eq!(placement.manifest.stages.len(), 8);
    assert!(matches!(
        placement.candidates[0].outcome,
        CandidateOutcome::GpuShort { stage: _, .. }
    ));
}

/// Pylon's named placements (#12402) read these manifests as JSON files,
/// since Pylon can't link Psionic's workspace. Each file is the manifest's
/// own serialization, so its SHA-256 is [`V41GroupManifest::digest`].
/// Regenerate with `PYLON_PLACEMENTS_WRITE=1`.
#[test]
fn pylon_placement_manifests_match_the_committed_files() {
    const GIB: u64 = 1 << 30;
    let memory = HeaderBytes::flash(RESERVE).unwrap();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../pylon/placements/deepseek-v41");
    // RTX PRO 6000 machines: g4-standard-48 (1 GPU, 180 GiB),
    // g4-standard-96 (2 GPUs, 360 GiB), and g4-standard-192 (4 GPUs,
    // 720 GiB).
    for (name, hosts, per_host, host_ram) in [
        ("four-single", &["m0", "m1", "m2", "m3"][..], 1, 180 * GIB),
        ("two-dual", &["m0", "m1"][..], 2, 360 * GIB),
        ("one-quad", &["m0"][..], 4, 720 * GIB),
    ] {
        let request = PlacementRequest {
            gpus: gpus(hosts, per_host, 96 * GB),
            hosts: ram(hosts, host_ram),
            stage_counts: vec![4],
        };
        let manifest = plan_group(&memory, &request).unwrap().manifest;
        manifest.validate().unwrap();
        let bytes = serde_json::to_vec(&manifest).unwrap();
        assert_eq!(
            hex::encode(Sha256::digest(&bytes)),
            manifest.digest_hex(),
            "{name}"
        );
        let path = dir.join(format!("{name}.json"));
        if std::env::var_os("PYLON_PLACEMENTS_WRITE").is_some() {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(&path, &bytes).unwrap();
        }
        let committed = std::fs::read(&path).unwrap_or_default();
        assert!(
            committed == bytes,
            "{} is stale; regenerate with PYLON_PLACEMENTS_WRITE=1",
            path.display()
        );
    }
    // quad-mac (#12420): layers 0-38 on one g4-standard-192 and the last
    // two on this computer's Apple GPU (host `local`, 60 GB of an M5 Max's
    // unified memory), planned from the measured memory.
    let memory = MeasuredMemory::flash(8192).unwrap();
    let mut slots: Vec<DeviceSlot> = (0..4)
        .map(|gpu| slot("m0", gpu, "cuda-sm_120", DeviceKind::Gpu, 101_973_819_392))
        .collect();
    slots.push(slot(
        "local",
        0,
        "metal-apple10",
        DeviceKind::Gpu,
        60_000_000_000,
    ));
    let hosts = [
        HostRam {
            host: "m0".into(),
            bytes: 720 * GIB,
        },
        HostRam {
            host: "local".into(),
            bytes: 60_000_000_000,
        },
    ];
    let split = [0..10, 10..20, 20..30, 30..38, 38..40];
    let manifest = plan_devices(&memory, &slots, &hosts, Some(&split)).unwrap();
    manifest.validate().unwrap();
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let path = dir.join("quad-mac.json");
    if std::env::var_os("PYLON_PLACEMENTS_WRITE").is_some() {
        std::fs::write(&path, &bytes).unwrap();
    }
    assert!(
        std::fs::read(&path).unwrap_or_default() == bytes,
        "{} is stale; regenerate with PYLON_PLACEMENTS_WRITE=1",
        path.display()
    );
}

/// A zero-filled prefill message of real shape: `tokens` tokens at position 0.
fn zero_prefill(contract: &StageContract, tokens: u32) -> StageMessage {
    let sections = contract
        .sections
        .iter()
        .map(|spec| {
            let rows = match spec.rows {
                RowRule::PerToken => tokens,
                RowRule::Append { ratio } => tokens / ratio.max(1),
                RowRule::Fixed(rows) => rows,
            };
            let mut shape = vec![rows];
            shape.extend_from_slice(&spec.inner);
            let elements: usize = shape.iter().map(|d| *d as usize).product();
            Section {
                kind: spec.kind,
                dtype: spec.dtype,
                shape,
                first_row: 0,
                data: vec![0; elements * spec.dtype.bytes()],
            }
        })
        .collect();
    StageMessage {
        manifest_digest: [0; 32],
        generation: 1,
        request_id: "r".repeat(128),
        attempt: 1,
        stage: contract.stage,
        layers: contract.layers.clone(),
        step: 0,
        kind: StepKind::Prefill,
        position: 0,
        tokens: vec![0; tokens as usize],
        sections,
    }
}

#[test]
fn prefill_sizes_match_encoded_messages_and_bound_the_step() {
    let limits = CodecLimits::default();
    // The CUDA stage's wire: F32 residuals and BF16 cache rows.
    let cuda = V41StageGeometry::flash().with_dtypes(WireDtype::F32, WireDtype::Bf16);
    for split in [even(4), even(8)] {
        for (k, layers) in split.iter().enumerate().skip(1) {
            let contract = stage_contract(&cuda, k as u16, layers.clone());
            for tokens in [1, 2, 7, 128, 1000] {
                let message = zero_prefill(&contract, tokens);
                let (total, largest) = contract.step_bytes(128, u64::from(tokens), 0);
                assert_eq!(total, encode(&message, &limits).unwrap().len());
                assert_eq!(
                    largest,
                    message.sections.iter().map(|s| s.data.len()).max().unwrap()
                );
            }
            let most = contract.max_step_tokens(&limits, 128);
            let fits = zero_prefill(&contract, most as u32);
            assert!(encode(&fits, &limits).is_ok(), "boundary {}", layers.start);
            let over = zero_prefill(&contract, most as u32 + 1);
            assert!(matches!(
                encode(&over, &limits),
                Err(StageRefusal::Oversize { .. })
            ));
        }
    }
    // The residual alone, `[tokens, 4, 5120]` in F32, passes 256 MiB at
    // 3,277 tokens, so no CUDA boundary carries that many in one step.
    let residual = |tokens: u64| tokens * 4 * 5120 * 4;
    assert!(residual(3276) <= limits.max_section_bytes as u64);
    assert!(residual(3277) > limits.max_section_bytes as u64);
    for split in [even(4), even(8)] {
        for (k, layers) in split.iter().enumerate().skip(1) {
            let most =
                stage_contract(&cuda, k as u16, layers.clone()).max_step_tokens(&limits, 128);
            assert!(most < 3277, "boundary {}: {most}", layers.start);
        }
    }
    // A step never carries more tokens than the codec allows.
    let synthetic = V41StageGeometry::synthetic();
    let contract = stage_contract(&synthetic, 1, 20..40);
    assert_eq!(
        contract.max_step_tokens(&limits, 128),
        limits.max_tokens as u64
    );
}

// ---------------------------------------------------------------------------
// W3's measured placement input (#12390)
// ---------------------------------------------------------------------------

/// An RTX PRO 6000's device memory, as `cudaMemGetInfo` reports it.
const RTX_PRO_6000: u64 = 101_973_819_392;

fn stage_weights(memory: &dyn PlacementMemory, first: u32, end: u32) -> u64 {
    stage_weight_bytes(memory, &(first..end), first == 0, end == V41_LAYERS)
}

#[test]
fn measured_memory_covers_every_measured_stage_peak() {
    let memory = MeasuredMemory::flash(8192).unwrap();
    let file = memory.file();
    assert_eq!(file.status, "provisional");
    assert!(!file.checks.is_empty());
    for check in &file.checks {
        let at =
            MeasuredMemory::from_json(FLASH_MEASURED, check.max_context, check.max_step_tokens)
                .unwrap();
        let layers = check.first_layer..check.end_layer;
        let need = stage_weights(&at, check.first_layer, check.end_layer)
            + at.stage_runtime_bytes(layers.clone());
        assert!(need >= check.peak_used_bytes, "{check:?}: {need}");
        // The model is a bound, not a guess: within 1 GiB of every peak.
        assert!(need - check.peak_used_bytes < 1 << 30, "{check:?}: {need}");
    }
    assert!(
        memory.source().starts_with("measured:"),
        "{}",
        memory.source()
    );
    assert!(memory.source().ends_with(":provisional"));
}

#[test]
fn measured_memory_places_the_served_group_on_one_quad() {
    // A serving stage's step equals its context (one prefill step).
    let memory = MeasuredMemory::flash(8192).unwrap();
    let card = RTX_PRO_6000;
    let request = PlacementRequest::new(gpus(&["m0"], 4, card), ram(&["m0"], 700 * GB));
    let placement = plan_group(&memory, &request).unwrap();
    let m = &placement.manifest;
    m.validate().unwrap();
    assert_eq!(m.stages.len(), 4);
    assert!(m.memory_source.starts_with("measured:"));
    assert_eq!(m.split(), [0..10, 10..20, 20..30, 30..40]);
    for s in &m.stages {
        assert!(s.gpu_weight_bytes + s.gpu_runtime_bytes <= card);
    }
}

#[test]
fn measured_step_buffers_push_long_served_contexts_to_eight_stages() {
    // A one-step prefill of 262,144 positions needs about 48 GB of step
    // buffers per stage: four cards no longer hold the model, eight do.
    let memory = MeasuredMemory::flash(262_144).unwrap();
    let hosts = ["a", "b", "c", "d"];
    let request = PlacementRequest::new(gpus(&hosts, 2, RTX_PRO_6000), ram(&hosts, 360 * GB));
    let placement = plan_group(&memory, &request).unwrap();
    assert!(matches!(
        placement.candidates[0].outcome,
        CandidateOutcome::GpuShort { .. }
    ));
    assert_eq!(placement.manifest.stages.len(), 8);
    // With 8,192-token steps (chunked prefill) the same context fits four.
    let chunked = MeasuredMemory::from_json(FLASH_MEASURED, 262_144, 8192).unwrap();
    let request = PlacementRequest::new(gpus(&hosts, 1, RTX_PRO_6000), ram(&hosts, 360 * GB));
    assert_eq!(
        plan_group(&chunked, &request)
            .unwrap()
            .manifest
            .stages
            .len(),
        4
    );
}

#[test]
fn measured_memory_refuses_another_file() {
    let other = FLASH_MEASURED.replace(MEASURED_MEMORY_SCHEMA, "other.v1");
    assert!(MeasuredMemory::from_json(&other, 8192, 8192).is_err());
    let other = FLASH_MEASURED.replace(DEEPSEEK_V41_FLASH_REVISION, "0000");
    assert!(MeasuredMemory::from_json(&other, 8192, 8192).is_err());
    assert!(MeasuredMemory::from_json(FLASH_MEASURED, 8192, 16_384).is_err());
    assert!(MeasuredMemory::from_json(FLASH_MEASURED, 0, 0).is_err());
}

fn slot(host: &str, gpu: u32, class: &str, kind: DeviceKind, bytes: u64) -> DeviceSlot {
    DeviceSlot {
        host: host.into(),
        gpu,
        class: class.into(),
        kind,
        bytes,
        experts: ExpertMemory::Device,
        per_stage_bytes: None,
    }
}

#[test]
fn routed_experts_are_nearly_all_of_a_layer() {
    let memory = MeasuredMemory::flash(8192).unwrap();
    for layer in 0..V41_LAYERS {
        let bytes = memory.layer_gpu_bytes(layer);
        assert!(FLASH_ROUTED_EXPERT_BYTES < bytes, "layer {layer}");
        // Attention, the router, and the shared expert stay under 400 MB.
        assert!(
            bytes - FLASH_ROUTED_EXPERT_BYTES < 400 * GB / 1000,
            "layer {layer}"
        );
    }
}

#[test]
fn a_mixed_group_places_layers_by_each_device_class() {
    let memory = MeasuredMemory::flash(8192).unwrap();
    let mut small = slot("home", 0, "cuda-sm_89", DeviceKind::Gpu, 5 * GB);
    small.experts = ExpertMemory::Host;
    small.per_stage_bytes = Some(GB / 2);
    let slots = vec![
        slot("g4", 0, "cuda-sm_120", DeviceKind::Gpu, RTX_PRO_6000),
        slot("g4", 1, "cuda-sm_120", DeviceKind::Gpu, RTX_PRO_6000),
        small.clone(),
        slot("cpu", 0, "cpu-x86_64", DeviceKind::Cpu, 64 * GB),
        slot("g4b", 0, "cuda-sm_120", DeviceKind::Gpu, RTX_PRO_6000),
        slot("g4b", 1, "cuda-sm_120", DeviceKind::Gpu, RTX_PRO_6000),
    ];
    let hosts = vec![
        HostRam {
            host: "g4".into(),
            bytes: 300 * GB,
        },
        HostRam {
            host: "home".into(),
            bytes: 40 * GB,
        },
        HostRam {
            host: "cpu".into(),
            bytes: 200 * GB,
        },
        HostRam {
            host: "g4b".into(),
            bytes: 300 * GB,
        },
    ];
    let m = plan_devices(&memory, &slots, &hosts, None).unwrap();
    m.validate().unwrap();
    assert!(m.memory_source.ends_with(":devices"));
    assert_eq!(m.stages.len(), 6);
    for (s, slot) in m.stages.iter().zip(&slots) {
        let device = s.device.as_ref().unwrap();
        assert_eq!(device.class, slot.class);
        assert!(s.gpu_weight_bytes + s.gpu_runtime_bytes <= slot.bytes);
    }
    // The 16 GB card holds a layer only with its experts on the host.
    let home = &m.stages[2];
    assert!(home.layers().len() >= 1);
    let experts = home.layers().len() as u64 * FLASH_ROUTED_EXPERT_BYTES;
    assert_eq!(home.device.as_ref().unwrap().expert_host_bytes, experts);
    // An explicit split is checked as given.
    let split = m.split();
    assert_eq!(
        plan_devices(&memory, &slots, &hosts, Some(&split)).unwrap(),
        m
    );
    // The same card with its experts on the device can't hold one layer.
    let mut slots_on_device = slots.clone();
    slots_on_device[2].experts = ExpertMemory::Device;
    let refused = plan_devices(&memory, &slots_on_device, &hosts, Some(&split)).unwrap_err();
    assert!(matches!(
        refused.candidates[0].outcome,
        CandidateOutcome::GpuShort { stage: 2, .. }
    ));
    // And without host memory for those experts the plan is refused.
    let mut tight = hosts.clone();
    tight[1].bytes = GB;
    let refused = plan_devices(&memory, &slots, &tight, Some(&split)).unwrap_err();
    assert!(matches!(
        refused.candidates[0].outcome,
        CandidateOutcome::HostShort { .. }
    ));
}

#[test]
fn manifests_without_devices_keep_their_bytes() {
    let memory = MeasuredMemory::flash(8192).unwrap();
    let request = PlacementRequest::new(gpus(&["m0"], 4, RTX_PRO_6000), ram(&["m0"], 700 * GB));
    let m = plan_group(&memory, &request).unwrap().manifest;
    let json = serde_json::to_string(&m).unwrap();
    assert!(!json.contains("\"device\""));
    let back: V41GroupManifest = serde_json::from_str(&json).unwrap();
    assert_eq!(back.digest(), m.digest());
}
