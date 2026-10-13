//! The joint head on the device in one round trip (#11446).
//!
//! The staged head ([`crate::clef::head::run_head`] through the
//! `head_*` calls) made about 25 round trips per request: each product
//! batch, memory-attention block and feed-forward uploaded its inputs and
//! biases, waited, and read its outputs back, and the LayerNorms, the
//! field self-attention, the routing and the scorer ran on the host in
//! between. On an H100 that left about 4.6 ms of the GPU idle per request
//! (clef-native.md Round 9).
//!
//! Here the head's vectors (norms, biases, the type embedding, the scorer
//! output) are uploaded once at load next to its matrices, and a request
//! uploads its pooled inputs once (question and option span means, the
//! lexical means, the last row, which question owns each option), runs the
//! whole head in one submission and reads back one logit per option. The
//! kernels between the cuBLAS products follow the host head step by step
//! (`clef_prefill.cu`, "the joint head on the device").
//!
//! `PSIONIC_CLEF_CUDA_HEAD=staged` keeps the staged head.

use super::*;
use crate::clef::head::{
    ClefHeadConfig, ClefHeadWeights, DeviceHeadInputs, HEAD_NORM_EPSILON, Matrix,
};
use psionic_backend_cuda::clef_prefill::ClefHeadScalars;

struct LayerNormBuffers {
    weight: CudaBuffer,
    bias: CudaBuffer,
}

struct AttentionBiases {
    q: CudaBuffer,
    k: CudaBuffer,
    v: CudaBuffer,
    out: CudaBuffer,
}

struct EvidenceVectors {
    query_norm: LayerNormBuffers,
    attention: AttentionBiases,
    feedforward_norm: LayerNormBuffers,
    up_bias: CudaBuffer,
    down_bias: CudaBuffer,
}

struct FieldVectors {
    self_norm: LayerNormBuffers,
    self_attention: AttentionBiases,
    cross_norm: LayerNormBuffers,
    cross_attention: AttentionBiases,
    feedforward_norm: LayerNormBuffers,
    up_bias: CudaBuffer,
    down_bias: CudaBuffer,
}

/// The head's vectors on the device, uploaded once.
pub(super) struct DeviceHead {
    config: ClefHeadConfig,
    digest: String,
    scalars: ClefHeadScalars,
    evidence: Vec<EvidenceVectors>,
    fields: Vec<FieldVectors>,
    option_summary_norm: LayerNormBuffers,
    field_norm: LayerNormBuffers,
    option_norm: LayerNormBuffers,
    type_embedding: CudaBuffer,
    type_count: usize,
    scorer_bias: CudaBuffer,
    scorer_out: CudaBuffer,
    pub(super) bytes: u64,
    work: Option<Workspace>,
}

/// Per-request activations, grown to the largest request seen.
struct Workspace {
    questions: usize,
    options: usize,
    arena: CudaBuffer,
    layout: Layout,
}

/// Element offsets into the workspace arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Layout {
    // inputs (one upload)
    questions: usize,
    context: usize,
    lexical: usize,
    global: usize,
    owner: usize,
    meta: usize,
    input_end: usize,
    // activations
    question_projected: usize,
    context_projected: usize,
    lexical_projected: usize,
    routed: usize,
    normalized: usize,
    update: usize,
    hidden: usize,
    base: usize,
    global_projected: usize,
    route_scores: usize,
    fields: usize,
    q: usize,
    k: usize,
    v: usize,
    context_out: usize,
    self_scores: usize,
    fields_normed: usize,
    options_normed: usize,
    features: usize,
    scorer_hidden: usize,
    logits: usize,
    end: usize,
}

impl Layout {
    fn new(config: ClefHeadConfig, questions: usize, options: usize) -> Result<Self, String> {
        let (d, w) = (config.hidden_size, config.width);
        let rows = questions.max(options).max(1);
        let mut at = 0usize;
        let mut take = |count: usize| -> Result<usize, String> {
            let start = at;
            at = at
                .checked_add(count)
                .ok_or("CUDA head workspace size overflow")?;
            Ok(start)
        };
        let product = |a: usize, b: usize| {
            a.checked_mul(b)
                .ok_or_else(|| String::from("CUDA head workspace size overflow"))
        };
        Ok(Self {
            questions: take(product(questions, d)?)?,
            context: take(product(options, d)?)?,
            lexical: take(product(options, d)?)?,
            global: take(d)?,
            owner: take(options)?,
            meta: take(product(questions, 3)?)?,
            input_end: take(0)?,
            question_projected: take(product(questions, w)?)?,
            context_projected: take(product(options, w)?)?,
            lexical_projected: take(product(options, w)?)?,
            routed: take(product(options, w)?)?,
            normalized: take(product(rows, w)?)?,
            update: take(product(rows, w)?)?,
            hidden: take(product(rows, config.feedforward)?)?,
            base: take(product(questions, w)?)?,
            global_projected: take(w)?,
            route_scores: take(options)?,
            fields: take(product(questions, w)?)?,
            q: take(product(questions, w)?)?,
            k: take(product(questions, w)?)?,
            v: take(product(questions, w)?)?,
            context_out: take(product(questions, w)?)?,
            self_scores: take(product(product(questions, config.heads)?, questions)?)?,
            fields_normed: take(product(questions, w)?)?,
            options_normed: take(product(options, w)?)?,
            features: take(product(options, 4 * w)?)?,
            scorer_hidden: take(product(options, w)?)?,
            logits: take(options)?,
            end: take(0)?,
        })
    }
}

/// `PSIONIC_CLEF_CUDA_HEAD`: `device` (default) or `staged`.
fn device_head_enabled() -> Result<bool, String> {
    match std::env::var("PSIONIC_CLEF_CUDA_HEAD") {
        Err(std::env::VarError::NotPresent) => Ok(true),
        Ok(value) if value == "device" => Ok(true),
        Ok(value) if value == "staged" => Ok(false),
        _ => Err(String::from(
            "PSIONIC_CLEF_CUDA_HEAD must be device or staged",
        )),
    }
}

impl ClefCudaTrunk {
    /// Uploads the head's vectors so the whole head runs on the device
    /// ([`Self::head_device`]); a no-op with `PSIONIC_CLEF_CUDA_HEAD=staged`.
    /// Every matrix of the head must already be resident (load uploads them).
    pub fn prepare_device_head(&self, weights: &ClefHeadWeights) -> Result<(), String> {
        if !device_head_enabled()? {
            return Ok(());
        }
        let c = weights.config;
        if c.width != self.dims.width || c.hidden_size != self.dims.hidden {
            return Err(String::from("clef cuda head: head and trunk widths differ"));
        }
        if c.heads == 0 || c.width % c.heads != 0 {
            return Err(String::from(
                "clef cuda head: width is not a multiple of heads",
            ));
        }
        if weights.evidence_layers.len() != c.routing_layers || weights.layers.len() != c.layers {
            return Err(String::from("clef cuda head: layer count mismatch"));
        }
        let mut guard = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = &mut *guard;
        if state.evidence_norms.len() != weights.evidence_layers.len() {
            return Err(String::from("clef cuda head: evidence norm count mismatch"));
        }
        for matrix in weights.linear_matrices() {
            let entry = state
                .head_matrices
                .get(&(matrix.values.as_ptr() as usize))
                .ok_or("clef cuda head: a head matrix is not on the device")?;
            if (entry.1, entry.2) != (matrix.rows, matrix.columns) {
                return Err(String::from("clef cuda head: device matrix shape mismatch"));
            }
        }
        if weights.type_embedding.columns != c.width
            || weights.scorer_out.len() != c.width
            || weights.scorer_bias.len() != c.width
        {
            return Err(String::from("clef cuda head: vector shape mismatch"));
        }
        let mut bytes = 0u64;
        let backend = &mut state.backend;
        let mut upload = |values: &[f32], len: usize| -> Result<CudaBuffer, String> {
            if values.len() != len || values.iter().any(|value| !value.is_finite()) {
                return Err(String::from("clef cuda head: invalid head vector"));
            }
            bytes += (values.len() * 4) as u64;
            upload_f32(backend, values)
        };
        let w = c.width;
        let ff = c.feedforward;
        macro_rules! norm {
            ($norm:expr) => {
                LayerNormBuffers {
                    weight: upload(&$norm.weight, w)?,
                    bias: upload(&$norm.bias, w)?,
                }
            };
        }
        macro_rules! biases {
            ($a:expr) => {
                AttentionBiases {
                    q: upload(&$a.q_bias, w)?,
                    k: upload(&$a.k_bias, w)?,
                    v: upload(&$a.v_bias, w)?,
                    out: upload(&$a.out_bias, w)?,
                }
            };
        }
        let mut evidence = Vec::with_capacity(weights.evidence_layers.len());
        for layer in &weights.evidence_layers {
            evidence.push(EvidenceVectors {
                query_norm: norm!(layer.query_norm),
                attention: biases!(layer.attention),
                feedforward_norm: norm!(layer.feedforward_norm),
                up_bias: upload(&layer.up_bias, ff)?,
                down_bias: upload(&layer.down_bias, w)?,
            });
        }
        let mut fields = Vec::with_capacity(weights.layers.len());
        for layer in &weights.layers {
            fields.push(FieldVectors {
                self_norm: norm!(layer.self_norm),
                self_attention: biases!(layer.self_attention),
                cross_norm: norm!(layer.cross_norm),
                cross_attention: biases!(layer.cross_attention),
                feedforward_norm: norm!(layer.feedforward_norm),
                up_bias: upload(&layer.up_bias, ff)?,
                down_bias: upload(&layer.down_bias, w)?,
            });
        }
        let head = DeviceHead {
            config: c,
            digest: weights.digest.clone(),
            scalars: ClefHeadScalars {
                scorer_out_bias: weights.scorer_out_bias,
                prior_scale: weights.prior_scale,
                joint_scale: weights.joint_scale,
                gate: weights.gate,
            },
            evidence,
            fields,
            option_summary_norm: norm!(weights.option_summary_norm),
            field_norm: norm!(weights.field_norm),
            option_norm: norm!(weights.option_norm),
            type_embedding: upload(
                &weights.type_embedding.values,
                weights.type_embedding.rows * w,
            )?,
            type_count: weights.type_embedding.rows,
            scorer_bias: upload(&weights.scorer_bias, w)?,
            scorer_out: upload(&weights.scorer_out, w)?,
            bytes: 0,
            work: None,
        };
        let head = DeviceHead { bytes, ..head };
        state.weight_bytes += head.bytes;
        state.device_head = Some(head);
        Ok(())
    }

    /// The whole head after this request's prefill in one submission: one
    /// upload, one wait, one read of the `[options]` logits (grouped by
    /// question). `None` when the device head is off.
    pub fn head_device(
        &self,
        weights: &ClefHeadWeights,
        inputs: &DeviceHeadInputs<'_>,
    ) -> Result<Option<Vec<f32>>, String> {
        let mut guard = self
            .device
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = &mut *guard;
        let Some(head) = state.device_head.as_ref() else {
            return Ok(None);
        };
        let c = head.config;
        if c != weights.config || head.digest != weights.digest {
            return Err(String::from(
                "clef cuda head: the head changed after upload",
            ));
        }
        let (d, w) = (c.hidden_size, c.width);
        let questions = inputs.question_types.len();
        let options = inputs.owner.len();
        if questions == 0 || options == 0 {
            return Err(String::from("clef cuda head: no questions or options"));
        }
        if inputs.questions.len() != questions * d
            || inputs.context.len() != options * d
            || inputs.lexical.len() != options * d
            || inputs.global.len() != d
        {
            return Err(String::from("clef cuda head: input shape mismatch"));
        }
        // options are grouped by question, in order
        let mut meta = vec![0i32; questions * 3];
        for (option, &owner) in inputs.owner.iter().enumerate() {
            if owner >= questions || (option > 0 && owner < inputs.owner[option - 1]) {
                return Err(String::from(
                    "clef cuda head: options out of question order",
                ));
            }
            if meta[owner * 3 + 1] == 0 {
                meta[owner * 3] = i32::try_from(option).map_err(err)?;
            }
            meta[owner * 3 + 1] += 1;
        }
        for (question, &kind) in inputs.question_types.iter().enumerate() {
            if kind >= head.type_count {
                return Err(String::from("clef cuda head: unknown question type"));
            }
            if meta[question * 3 + 1] == 0 {
                return Err(String::from("clef cuda head: a question has no options"));
            }
            meta[question * 3 + 2] = i32::try_from(kind).map_err(err)?;
        }
        let length = state
            .request
            .as_ref()
            .ok_or("no prefill on the device")?
            .tokens;

        // buffers
        ensure_workspace(state, questions, options)?;
        let rows = questions.max(options);
        ensure_projected_attention(state, rows, c.heads, w, length)?;
        let mut normalize = Vec::new();
        for layer in 0..c.routing_layers {
            if ensure_normalized_memory(state, layer, length, w)? {
                normalize.push(layer);
            }
        }

        // one upload
        let head = state.device_head.as_mut().ok_or("device head")?;
        let work = head.work.as_mut().ok_or("head workspace")?;
        // this request's own layout: packed for its sizes, so the input is
        // one contiguous upload; it fits inside the arena's capacity layout
        let layout = Layout::new(c, questions, options)?;
        if layout.end > work.layout.end {
            return Err(String::from("clef cuda head: workspace too small"));
        }
        let mut packed = Vec::with_capacity(layout.input_end);
        packed.extend_from_slice(inputs.questions);
        packed.extend_from_slice(inputs.context);
        packed.extend_from_slice(inputs.lexical);
        packed.extend_from_slice(inputs.global);
        packed.extend(
            inputs
                .owner
                .iter()
                .map(|&owner| f32::from_bits(owner as u32)),
        );
        packed.extend(meta.iter().map(|&value| f32::from_bits(value as u32)));
        if packed.len() != layout.input_end {
            return Err(String::from("clef cuda head: input size mismatch"));
        }
        work.arena
            .write_bytes_at_offset(0, f32_bytes(&packed))
            .map_err(err)?;

        let head = state.device_head.as_ref().ok_or("device head")?;
        let arena = &head.work.as_ref().ok_or("head workspace")?.arena;
        let at = |offset: usize| ClefOperand::f32(arena, offset);
        let request = state.request.as_ref().ok_or("no prefill")?;
        let attention = state
            .projected_attention
            .as_ref()
            .ok_or("projected attention scratch")?;
        let matrix = |m: &Matrix| -> Result<&CudaBuffer, String> {
            state
                .head_matrices
                .get(&(m.values.as_ptr() as usize))
                .map(|entry| &entry.0)
                .ok_or_else(|| String::from("clef cuda head: a head matrix is not on the device"))
        };
        let eps = HEAD_NORM_EPSILON;
        let heads = c.heads;
        let ff = c.feedforward;
        let scale = 1.0 / ((w / heads) as f32).sqrt();
        let mut s = state.backend.begin_submission().map_err(err)?;

        // `out = x W^T (+ b)` for `n` rows into the arena.
        let linear = |s: &mut CudaSubmission,
                      x: ClefOperand<'_>,
                      m: &Matrix,
                      bias: Option<&CudaBuffer>,
                      out: usize,
                      n: usize|
         -> Result<(), String> {
            s.clef_linear(
                x,
                ClefOperand::f32(matrix(m)?, 0),
                at(out),
                n,
                m.rows,
                m.columns,
                false,
                false,
            )
            .map_err(err)?;
            if let Some(bias) = bias {
                s.clef_bias_act(arena, out, bias, 0, n, m.rows, false)
                    .map_err(err)?;
            }
            Ok(())
        };
        let layer_norm = |s: &mut CudaSubmission,
                          x: usize,
                          norm: &LayerNormBuffers,
                          out: usize,
                          n: usize|
         -> Result<(), String> {
            s.clef_head_layer_norm(
                at(x),
                ClefOperand::f32(&norm.weight, 0),
                ClefOperand::f32(&norm.bias, 0),
                at(out),
                n,
                w,
                eps,
            )
            .map_err(err)
        };
        // `update = down(gelu(up normalized + b)) + b` for `n` rows.
        let feedforward = |s: &mut CudaSubmission,
                           up: &Matrix,
                           up_bias: &CudaBuffer,
                           down: &Matrix,
                           down_bias: &CudaBuffer,
                           n: usize|
         -> Result<(), String> {
            s.clef_linear(
                at(layout.normalized),
                ClefOperand::f32(matrix(up)?, 0),
                at(layout.hidden),
                n,
                ff,
                w,
                false,
                false,
            )
            .map_err(err)?;
            s.clef_bias_act(arena, layout.hidden, up_bias, 0, n, ff, true)
                .map_err(err)?;
            linear(
                s,
                at(layout.hidden),
                down,
                Some(down_bias),
                layout.update,
                n,
            )
        };
        // A memory-attention block of `n` normalized rows over `view`:
        // `update = W_out attend(W_q x + b_q) + b_out` (the staged
        // `head_attend_block`).
        let attend = |s: &mut CudaSubmission,
                      a: &crate::clef::head::Attention,
                      biases: &AttentionBiases,
                      view: &CudaBuffer,
                      n: usize|
         -> Result<(), String> {
            let side_rows = n * heads;
            s.clef_linear(
                at(layout.normalized),
                ClefOperand::f32(matrix(&a.q)?, 0),
                ClefOperand::f32(&attention.query, 0),
                n,
                w,
                w,
                false,
                false,
            )
            .map_err(err)?;
            s.clef_bias_act(&attention.query, 0, &biases.q, 0, n, w, false)
                .map_err(err)?;
            s.clef_head_side(
                &attention.query,
                matrix(&a.k)?,
                &attention.side,
                n,
                heads,
                w,
            )
            .map_err(err)?;
            s.clef_linear(
                ClefOperand::f32(&attention.side, 0),
                ClefOperand::f32(view, 0),
                ClefOperand::f32(&attention.scores, 0),
                side_rows,
                length,
                w,
                false,
                false,
            )
            .map_err(err)?;
            s.clef_softmax_rows_f32(&attention.scores, side_rows, length, scale)
                .map_err(err)?;
            // mixed^T [w, side_rows] = M^T [w, L] . P^T [L, side_rows]
            s.clef_gemm_strided_batched(
                false,
                false,
                w,
                side_rows,
                length,
                ClefOperand::f32(view, 0),
                w,
                0,
                length * w,
                ClefOperand::f32(&attention.scores, 0),
                length,
                0,
                side_rows * length,
                ClefOperand::f32(&attention.mixed, 0),
                w,
                0,
                side_rows * w,
                1,
            )
            .map_err(err)?;
            s.clef_head_context(
                &attention.mixed,
                matrix(&a.v)?,
                &biases.v,
                &attention.context,
                n,
                heads,
                w,
            )
            .map_err(err)?;
            linear(
                s,
                ClefOperand::f32(&attention.context, 0),
                &a.out,
                Some(&biases.out),
                layout.update,
                n,
            )
        };
        let add = |s: &mut CudaSubmission, x: usize, n: usize| -> Result<(), String> {
            s.clef_head_add(at(x), at(layout.update), n * w)
                .map_err(err)
        };

        // R = W_oc c + W_ol l + W_oq q
        linear(
            &mut s,
            at(layout.questions),
            &weights.option_question_projection,
            None,
            layout.question_projected,
            questions,
        )?;
        linear(
            &mut s,
            at(layout.context),
            &weights.option_context_projection,
            None,
            layout.context_projected,
            options,
        )?;
        linear(
            &mut s,
            at(layout.lexical),
            &weights.option_lexical_projection,
            None,
            layout.lexical_projected,
            options,
        )?;
        s.clef_head_route(
            at(layout.context_projected),
            at(layout.lexical_projected),
            at(layout.question_projected),
            questions,
            at(layout.owner),
            at(layout.routed),
            options,
            w,
        )
        .map_err(err)?;

        // evidence layers
        for (index, (layer, vectors)) in weights
            .evidence_layers
            .iter()
            .zip(&head.evidence)
            .enumerate()
        {
            let view = request.normalized_memory[index]
                .as_ref()
                .ok_or("normalized memory")?;
            if normalize.contains(&index) {
                let (nw, nb) = &state.evidence_norms[index];
                s.clef_layer_norm_f32(
                    &request.memory,
                    Some((nw, nb)),
                    view,
                    length,
                    w,
                    self.dims.head_eps,
                )
                .map_err(err)?;
            }
            layer_norm(
                &mut s,
                layout.routed,
                &vectors.query_norm,
                layout.normalized,
                options,
            )?;
            attend(&mut s, &layer.attention, &vectors.attention, view, options)?;
            add(&mut s, layout.routed, options)?;
            layer_norm(
                &mut s,
                layout.routed,
                &vectors.feedforward_norm,
                layout.normalized,
                options,
            )?;
            feedforward(
                &mut s,
                &layer.up,
                &vectors.up_bias,
                &layer.down,
                &vectors.down_bias,
                options,
            )?;
            add(&mut s, layout.routed, options)?;
        }

        // fields
        linear(
            &mut s,
            at(layout.questions),
            &weights.question_projection,
            None,
            layout.base,
            questions,
        )?;
        linear(
            &mut s,
            at(layout.global),
            &weights.global_projection,
            None,
            layout.global_projected,
            1,
        )?;
        s.clef_head_route_scores(
            at(layout.routed),
            at(layout.base),
            questions,
            at(layout.owner),
            at(layout.route_scores),
            options,
            w,
        )
        .map_err(err)?;
        s.clef_head_fields(
            at(layout.routed),
            at(layout.route_scores),
            options,
            at(layout.base),
            at(layout.global_projected),
            ClefOperand::f32(&head.type_embedding, 0),
            head.type_count,
            at(layout.meta),
            (
                ClefOperand::f32(&head.option_summary_norm.weight, 0),
                ClefOperand::f32(&head.option_summary_norm.bias, 0),
            ),
            at(layout.fields),
            questions,
            w,
            eps,
        )
        .map_err(err)?;

        // field layers
        for (layer, vectors) in weights.layers.iter().zip(&head.fields) {
            let a = &layer.self_attention;
            let b = &vectors.self_attention;
            layer_norm(
                &mut s,
                layout.fields,
                &vectors.self_norm,
                layout.normalized,
                questions,
            )?;
            linear(
                &mut s,
                at(layout.normalized),
                &a.k,
                Some(&b.k),
                layout.k,
                questions,
            )?;
            linear(
                &mut s,
                at(layout.normalized),
                &a.v,
                Some(&b.v),
                layout.v,
                questions,
            )?;
            linear(
                &mut s,
                at(layout.normalized),
                &a.q,
                Some(&b.q),
                layout.q,
                questions,
            )?;
            s.clef_head_self_attention(
                at(layout.q),
                at(layout.k),
                at(layout.v),
                at(layout.self_scores),
                at(layout.context_out),
                questions,
                heads,
                w,
                scale,
            )
            .map_err(err)?;
            linear(
                &mut s,
                at(layout.context_out),
                &a.out,
                Some(&b.out),
                layout.update,
                questions,
            )?;
            add(&mut s, layout.fields, questions)?;
            // memory is not normalized in the field layers
            layer_norm(
                &mut s,
                layout.fields,
                &vectors.cross_norm,
                layout.normalized,
                questions,
            )?;
            attend(
                &mut s,
                &layer.cross_attention,
                &vectors.cross_attention,
                &request.memory,
                questions,
            )?;
            add(&mut s, layout.fields, questions)?;
            layer_norm(
                &mut s,
                layout.fields,
                &vectors.feedforward_norm,
                layout.normalized,
                questions,
            )?;
            feedforward(
                &mut s,
                &layer.up,
                &vectors.up_bias,
                &layer.down,
                &vectors.down_bias,
                questions,
            )?;
            add(&mut s, layout.fields, questions)?;
        }

        // scores
        layer_norm(
            &mut s,
            layout.fields,
            &head.field_norm,
            layout.fields_normed,
            questions,
        )?;
        layer_norm(
            &mut s,
            layout.routed,
            &head.option_norm,
            layout.options_normed,
            options,
        )?;
        s.clef_head_features(
            at(layout.fields_normed),
            questions,
            at(layout.options_normed),
            at(layout.owner),
            at(layout.features),
            options,
            w,
        )
        .map_err(err)?;
        linear(
            &mut s,
            at(layout.features),
            &weights.scorer,
            Some(&head.scorer_bias),
            layout.scorer_hidden,
            options,
        )?;
        s.clef_head_logits(
            at(layout.lexical),
            at(layout.questions),
            questions,
            at(layout.global),
            at(layout.owner),
            at(layout.fields_normed),
            at(layout.options_normed),
            at(layout.scorer_hidden),
            ClefOperand::f32(&head.scorer_out, 0),
            at(layout.logits),
            options,
            d,
            w,
            head.scalars,
        )
        .map_err(err)?;
        s.commit(CudaCommandWait::Completed).map_err(err)?;
        arena
            .read_f32_at_offset(layout.logits, options)
            .map(Some)
            .map_err(err)
    }
}

fn ensure_workspace(
    state: &mut DeviceState,
    questions: usize,
    options: usize,
) -> Result<(), String> {
    let head = state.device_head.as_ref().ok_or("device head")?;
    let config = head.config;
    let (questions, options) = match &head.work {
        Some(work) if work.questions >= questions && work.options >= options => return Ok(()),
        Some(work) => (questions.max(work.questions), options.max(work.options)),
        None => (questions, options),
    };
    let questions = memory_admission::capacity(questions, 16)?;
    let options = memory_admission::capacity(options, 64)?;
    let layout = Layout::new(config, questions, options)?;
    let elements = layout.end.max(1);
    admit_allocation(
        &state.backend,
        "head_workspace",
        &[memory_admission::bytes(&[elements], 4)?],
    )?;
    // drop the old arena before allocating the new one
    if let Some(head) = state.device_head.as_mut() {
        head.work = None;
    }
    let arena = state.backend.f32_buffer(elements).map_err(err)?;
    let head = state.device_head.as_mut().ok_or("device head")?;
    head.work = Some(Workspace {
        questions,
        options,
        arena,
        layout,
    });
    Ok(())
}

/// Grows [`ProjectedAttention`] to `rows` queries over `length` memory rows.
fn ensure_projected_attention(
    state: &mut DeviceState,
    rows: usize,
    heads: usize,
    w: usize,
    length: usize,
) -> Result<(), String> {
    let side_rows = rows
        .checked_mul(heads)
        .ok_or("CUDA memory estimate overflow")?;
    let query_capacity = memory_admission::capacity(rows, 16)?;
    let grow = state
        .projected_attention
        .as_ref()
        .is_none_or(|scratch| scratch.rows < side_rows || scratch.length < length);
    if !grow {
        return Ok(());
    }
    let capacity = memory_admission::capacity(side_rows, 64)?;
    admit_allocation(
        &state.backend,
        "projected_attention_scores",
        &[memory_admission::projected_attention_bytes(
            rows, heads, w, length,
        )?],
    )?;
    state.projected_attention = None;
    state.projected_attention = Some(ProjectedAttention {
        rows: capacity,
        length,
        query: state
            .backend
            .f32_buffer(query_capacity.max(capacity / heads) * w)
            .map_err(err)?,
        side: state.backend.f32_buffer(capacity * w).map_err(err)?,
        scores: state.backend.f32_buffer(capacity * length).map_err(err)?,
        mixed: state.backend.f32_buffer(capacity * w).map_err(err)?,
        context: state
            .backend
            .f32_buffer(query_capacity.max(capacity / heads) * w)
            .map_err(err)?,
        bias: state.backend.f32_buffer(w).map_err(err)?,
    });
    Ok(())
}

/// Allocates evidence layer `layer`'s `LN_m(M)` for the current request;
/// `true` when the caller still has to compute it.
fn ensure_normalized_memory(
    state: &mut DeviceState,
    layer: usize,
    length: usize,
    w: usize,
) -> Result<bool, String> {
    let request = state.request.as_mut().ok_or("no prefill")?;
    if request.normalized_memory.len() <= layer {
        request.normalized_memory.resize_with(layer + 1, || None);
    }
    if request.normalized_memory[layer].is_some() {
        return Ok(false);
    }
    admit_allocation(
        &state.backend,
        "normalized_memory",
        &[memory_admission::bytes(&[length, w], 4)?],
    )?;
    let buffer = state.backend.f32_buffer(length * w).map_err(err)?;
    let request = state.request.as_mut().ok_or("no prefill")?;
    request.bytes += buffer.byte_len() as u64;
    request.normalized_memory[layer] = Some(buffer);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> ClefHeadConfig {
        ClefHeadConfig {
            hidden_size: 64,
            width: 32,
            routing_layers: 2,
            layers: 4,
            heads: 4,
            feedforward: 128,
        }
    }

    #[test]
    fn layout_regions_do_not_overlap() {
        let layout = Layout::new(config(), 3, 7).expect("layout");
        let starts = [
            (layout.questions, 3 * 64),
            (layout.context, 7 * 64),
            (layout.lexical, 7 * 64),
            (layout.global, 64),
            (layout.owner, 7),
            (layout.meta, 9),
            (layout.question_projected, 3 * 32),
            (layout.context_projected, 7 * 32),
            (layout.lexical_projected, 7 * 32),
            (layout.routed, 7 * 32),
            (layout.normalized, 7 * 32),
            (layout.update, 7 * 32),
            (layout.hidden, 7 * 128),
            (layout.base, 3 * 32),
            (layout.global_projected, 32),
            (layout.route_scores, 7),
            (layout.fields, 3 * 32),
            (layout.q, 3 * 32),
            (layout.k, 3 * 32),
            (layout.v, 3 * 32),
            (layout.context_out, 3 * 32),
            (layout.self_scores, 3 * 4 * 3),
            (layout.fields_normed, 3 * 32),
            (layout.options_normed, 7 * 32),
            (layout.features, 7 * 128),
            (layout.scorer_hidden, 7 * 32),
            (layout.logits, 7),
        ];
        let mut end = 0;
        for (start, len) in starts {
            assert_eq!(start, end, "regions are packed in order");
            end = start + len;
        }
        assert_eq!(end, layout.end);
        assert_eq!(layout.input_end, layout.question_projected);
    }

    #[test]
    fn a_request_layout_fits_its_capacity_layout() {
        let capacity = Layout::new(config(), 16, 64).expect("capacity");
        for (questions, options) in [(1, 1), (3, 7), (16, 64), (16, 1), (1, 64)] {
            let layout = Layout::new(config(), questions, options).expect("layout");
            assert!(layout.end <= capacity.end);
        }
    }

    #[test]
    fn layout_refuses_overflow() {
        assert!(Layout::new(config(), usize::MAX / 2, 1).is_err());
    }
}
