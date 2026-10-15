//! Device-resident float32 encoder; only token inputs and final logits cross the boundary.
use super::*;
use psionic_backend_cuda::clef_prefill::ClefOperand;
use psionic_backend_cuda::{CudaBackend, CudaBuffer, CudaCommandWait, CudaGraphExec};
use psionic_core::Shape;
use std::sync::mpsc::{SyncSender, sync_channel};

fn error(e: impl std::fmt::Display) -> Error {
    Error(format!("Native CUDA encoder: {e}"))
}
struct Request {
    ids: Vec<i32>,
    mask: Vec<i32>,
    reply: SyncSender<Result<Tensors>>,
}
pub(super) struct Encoder {
    sender: Option<SyncSender<Request>>,
    worker: Option<std::thread::JoinHandle<()>>,
    pub descriptor: serde_json::Value,
    pub device: String,
    calls: u64,
    input_bytes: u64,
}
impl Drop for Encoder {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
struct LayerWeights {
    attn_norm: Option<Vec<f32>>,
    mlp_norm: Vec<f32>,
    qkv: Vec<f32>,
    attn_out: Vec<f32>,
    up: Vec<f32>,
    down: Vec<f32>,
}
struct Weights {
    config: ModernBertConfig,
    embedding: Vec<u8>,
    embedding_norm: Vec<f32>,
    final_norm: Vec<f32>,
    layers: Vec<LayerWeights>,
    head_weights: Vec<f32>,
    head_bias: Vec<f32>,
    heads: Vec<(String, usize)>,
}
impl Encoder {
    pub(super) fn load(model: &mut ModernBertEngine) -> Result<Self> {
        let mut head_weights = Vec::new();
        for row in 0..model.config.hidden {
            for head in model.heads.values() {
                head_weights.extend_from_slice(
                    &head.weight.values[row * head.weight.cols..(row + 1) * head.weight.cols],
                );
            }
        }
        let weights = Weights {
            config: model.config.clone(),
            embedding: std::mem::take(&mut model.embedding),
            embedding_norm: std::mem::take(&mut model.embedding_norm),
            final_norm: std::mem::take(&mut model.final_norm),
            layers: model
                .layers
                .iter_mut()
                .map(|l| LayerWeights {
                    attn_norm: l.attn_norm.take(),
                    mlp_norm: std::mem::take(&mut l.mlp_norm),
                    qkv: std::mem::take(&mut l.qkv.values),
                    attn_out: std::mem::take(&mut l.attn_out.values),
                    up: std::mem::take(&mut l.up.values),
                    down: std::mem::take(&mut l.down.values),
                })
                .collect(),
            head_weights,
            head_bias: model
                .heads
                .values()
                .flat_map(|h| h.bias.iter().copied())
                .collect(),
            heads: model
                .heads
                .iter()
                .map(|(name, h)| (name.clone(), h.bias.len()))
                .collect(),
        };
        let (sender, requests) = sync_channel::<Request>(1);
        let (ready, initialization) = sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("psionic-encoder-cuda".into())
            .spawn(move || {
                let mut state = match State::load(weights) {
                    Ok(state) => state,
                    Err(e) => {
                        let _ = ready.send(Err(e));
                        return;
                    }
                };
                if ready.send(Ok(state.descriptor.clone())).is_err() {
                    return;
                }
                while let Ok(request) = requests.recv() {
                    let _ = request.reply.send(state.run(&request.ids, &request.mask));
                }
            })?;
        let descriptor = initialization.recv().map_err(error)??;
        Ok(Self {
            device: descriptor["device_name"]
                .as_str()
                .unwrap_or("CUDA device 0")
                .into(),
            descriptor,
            sender: Some(sender),
            worker: Some(worker),
            calls: 0,
            input_bytes: 0,
        })
    }
    pub(super) fn run(&mut self, ids: &[i64], mask: &[i64]) -> Result<Tensors> {
        let (reply, result) = sync_channel(1);
        let request = Request {
            ids: ids
                .iter()
                .map(|id| i32::try_from(*id).map_err(error))
                .collect::<Result<_>>()?,
            mask: mask.iter().map(|v| *v as i32).collect(),
            reply,
        };
        self.sender
            .as_ref()
            .ok_or_else(|| error("Worker stopped"))?
            .send(request)
            .map_err(error)?;
        let output = result.recv().map_err(error)??;
        self.calls += 1;
        self.input_bytes += (ids.len() * 8) as u64;
        Ok(output)
    }
    pub(super) fn report(&self) -> serde_json::Value {
        serde_json::json!({"device":self.descriptor,"placement":"GPU encoder and heads","cpu_operations":"input validation and request dispatch","gpu_operations":"uint8 embedding gather/dequantization, LayerNorm, GEMMs, RoPE, masked local/global attention, exact-erf GeGLU, residuals, pooling and heads","calls":self.calls,"host_to_device_logical_bytes":self.input_bytes,"device_to_host_logical_bytes":self.calls*22*4,"request_completion_waits":self.calls,"activation_allocations_per_call":0,"cuda_graph_cache_max_lengths":64,"graph_note":"Capture on first use of a sequence length; bounded LRU cache","host_control_threads":1,"nvidia_tf32_override":std::env::var("NVIDIA_TF32_OVERRIDE").ok()})
    }
}
struct DeviceLayer {
    attn_norm: Option<CudaBuffer>,
    mlp_norm: CudaBuffer,
    qkv: CudaBuffer,
    attn_out: CudaBuffer,
    up: CudaBuffer,
    down: CudaBuffer,
    frequencies: CudaBuffer,
}
struct State {
    graphs: BTreeMap<usize, CudaGraphExec>,
    graph_order: std::collections::VecDeque<usize>,
    backend: CudaBackend,
    descriptor: serde_json::Value,
    config: ModernBertConfig,
    vocab: usize,
    embedding: CudaBuffer,
    embedding_norm: CudaBuffer,
    final_norm: CudaBuffer,
    zero: CudaBuffer,
    layers: Vec<DeviceLayer>,
    head_weights: CudaBuffer,
    head_bias: CudaBuffer,
    heads: Vec<(String, usize)>,
    ids: CudaBuffer,
    mask: CudaBuffer,
    hidden: CudaBuffer,
    normalized: CudaBuffer,
    qkv: CudaBuffer,
    rotated: CudaBuffer,
    context: CudaBuffer,
    scores: CudaBuffer,
    packed_context: CudaBuffer,
    projected: CudaBuffer,
    up: CudaBuffer,
    gated: CudaBuffer,
    logits: CudaBuffer,
}
fn upload(backend: &mut CudaBackend, values: Vec<f32>) -> Result<CudaBuffer> {
    backend
        .input_buffer(Shape::new(vec![values.len()]), values)
        .map_err(error)
}
impl State {
    fn load(weights: Weights) -> Result<Self> {
        let mut backend = CudaBackend::new();
        let descriptor = serde_json::to_value(
            backend
                .selected_device()
                .ok_or_else(|| error("CUDA is unavailable"))?,
        )?;
        let h = weights.config.hidden;
        let f = weights.config.intermediate;
        let vocab = weights.embedding.len() / h;
        let embedding = backend.byte_buffer(&weights.embedding).map_err(error)?;
        let embedding_norm = upload(&mut backend, weights.embedding_norm)?;
        let final_norm = upload(&mut backend, weights.final_norm)?;
        let zero = upload(&mut backend, vec![0.0; h])?;
        let mut layers = Vec::new();
        for (layer, config) in weights.layers.into_iter().zip(&weights.config.layers) {
            layers.push(DeviceLayer {
                attn_norm: layer
                    .attn_norm
                    .map(|v| upload(&mut backend, v))
                    .transpose()?,
                mlp_norm: upload(&mut backend, layer.mlp_norm)?,
                qkv: upload(&mut backend, layer.qkv)?,
                attn_out: upload(&mut backend, layer.attn_out)?,
                up: upload(&mut backend, layer.up)?,
                down: upload(&mut backend, layer.down)?,
                frequencies: upload(&mut backend, config.inverse_frequencies.clone())?,
            });
        }
        let head_weights = upload(&mut backend, weights.head_weights)?;
        let head_bias = upload(&mut backend, weights.head_bias)?;
        let ids = backend.i32_buffer(512).map_err(error)?;
        let mask = backend.i32_buffer(512).map_err(error)?;
        let hidden = backend.f32_buffer(512 * h).map_err(error)?;
        let normalized = backend.f32_buffer(512 * h).map_err(error)?;
        let qkv = backend.f32_buffer(512 * 3 * h).map_err(error)?;
        let rotated = backend.f32_buffer(512 * 3 * h).map_err(error)?;
        let context = backend.f32_buffer(512 * h).map_err(error)?;
        let scores = backend
            .f32_buffer(weights.config.heads * 512 * 512)
            .map_err(error)?;
        let packed_context = backend.f32_buffer(512 * h).map_err(error)?;
        let projected = backend.f32_buffer(512 * h).map_err(error)?;
        let up = backend.f32_buffer(512 * 2 * f).map_err(error)?;
        let gated = backend.f32_buffer(512 * f).map_err(error)?;
        let logits = backend.f32_buffer(22).map_err(error)?;
        Ok(Self {
            graphs: BTreeMap::new(),
            graph_order: std::collections::VecDeque::new(),
            backend,
            descriptor,
            config: weights.config,
            vocab,
            embedding,
            embedding_norm,
            final_norm,
            zero,
            layers,
            head_weights,
            head_bias,
            heads: weights.heads,
            ids,
            mask,
            hidden,
            normalized,
            qkv,
            rotated,
            context,
            scores,
            packed_context,
            projected,
            up,
            gated,
            logits,
        })
    }
    fn run(&mut self, ids: &[i32], mask: &[i32]) -> Result<Tensors> {
        self.ids.write_i32_at_offset(0, ids).map_err(error)?;
        self.mask.write_i32_at_offset(0, mask).map_err(error)?;
        let n = ids.len();
        let h = self.config.hidden;
        let f = self.config.intermediate;
        let eps = self.config.epsilon;
        if let Some(graph) = self.graphs.get(&n) {
            graph.launch(CudaCommandWait::Completed).map_err(error)?;
            self.graph_order.retain(|length| *length != n);
            self.graph_order.push_back(n);
            return self.outputs();
        }
        let mut s = self.backend.begin_captured_submission().map_err(error)?;
        s.encoder_embed(
            &self.embedding,
            &self.ids,
            &self.hidden,
            n,
            h,
            self.vocab,
            self.config.embedding_zero,
            self.config.embedding_scale,
        )
        .map_err(error)?;
        s.clef_layer_norm_f32(
            &self.hidden,
            Some((&self.embedding_norm, &self.zero)),
            &self.hidden,
            n,
            h,
            eps,
        )
        .map_err(error)?;
        // The first encoder layer skips attention normalization.
        for (i, layer) in self.layers.iter().enumerate() {
            if let Some(norm) = &layer.attn_norm {
                s.clef_layer_norm_f32(
                    &self.hidden,
                    Some((norm, &self.zero)),
                    &self.normalized,
                    n,
                    h,
                    eps,
                )
                .map_err(error)?;
            }
            s.matmul(
                if i == 0 {
                    &self.hidden
                } else {
                    &self.normalized
                },
                &layer.qkv,
                &self.qkv,
                n,
                h,
                3 * h,
            )
            .map_err(error)?;
            s.encoder_rope(
                &self.qkv,
                &layer.frequencies,
                &self.rotated,
                n,
                self.config.heads,
                h / self.config.heads,
            )
            .map_err(error)?;
            let rows = if i + 1 == self.layers.len() { 1 } else { n };
            if n >= 128 && rows > 1 {
                let heads = self.config.heads;
                let d = h / heads;
                let extent = heads * n * d;
                s.clef_gemm_strided_batched(
                    true,
                    false,
                    n,
                    n,
                    d,
                    ClefOperand::f32(&self.rotated, extent),
                    d,
                    n * d,
                    extent,
                    ClefOperand::f32(&self.rotated, 0),
                    d,
                    n * d,
                    extent,
                    ClefOperand::f32(&self.scores, 0),
                    n,
                    n * n,
                    heads * n * n,
                    heads,
                )
                .map_err(error)?;
                s.encoder_softmax(
                    &self.scores,
                    &self.mask,
                    n,
                    heads,
                    self.config.layers[i].window,
                )
                .map_err(error)?;
                s.clef_gemm_strided_batched(
                    false,
                    false,
                    d,
                    n,
                    n,
                    ClefOperand::f32(&self.rotated, 2 * extent),
                    d,
                    n * d,
                    extent,
                    ClefOperand::f32(&self.scores, 0),
                    n,
                    n * n,
                    heads * n * n,
                    ClefOperand::f32(&self.packed_context, 0),
                    d,
                    n * d,
                    extent,
                    heads,
                )
                .map_err(error)?;
                s.encoder_merge(&self.packed_context, &self.context, n, heads, d)
                    .map_err(error)?;
            } else {
                s.encoder_attention(
                    &self.rotated,
                    &self.mask,
                    &self.context,
                    n,
                    self.config.heads,
                    h / self.config.heads,
                    self.config.layers[i].window,
                    rows,
                )
                .map_err(error)?;
            }
            s.matmul(&self.context, &layer.attn_out, &self.projected, rows, h, h)
                .map_err(error)?;
            s.clef_head_add(
                ClefOperand::f32(&self.hidden, 0),
                ClefOperand::f32(&self.projected, 0),
                rows * h,
            )
            .map_err(error)?;
            let residual = &self.hidden;
            s.clef_layer_norm_f32(
                residual,
                Some((&layer.mlp_norm, &self.zero)),
                &self.context,
                rows,
                h,
                eps,
            )
            .map_err(error)?;
            s.matmul(&self.context, &layer.up, &self.up, rows, h, 2 * f)
                .map_err(error)?;
            s.encoder_geglu(&self.up, &self.gated, rows, f)
                .map_err(error)?;
            s.matmul(&self.gated, &layer.down, &self.projected, rows, f, h)
                .map_err(error)?;
            s.clef_head_add(
                ClefOperand::f32(&self.hidden, 0),
                ClefOperand::f32(&self.projected, 0),
                rows * h,
            )
            .map_err(error)?;
        }
        s.clef_layer_norm_f32(
            &self.hidden,
            Some((&self.final_norm, &self.zero)),
            &self.normalized,
            1,
            h,
            eps,
        )
        .map_err(error)?;
        s.matmul(&self.normalized, &self.head_weights, &self.logits, 1, h, 22)
            .map_err(error)?;
        s.clef_bias_act(&self.logits, 0, &self.head_bias, 0, 1, 22, false)
            .map_err(error)?;
        let (_, graph) = s
            .commit_captured(CudaCommandWait::Completed)
            .map_err(error)?;
        if self.graphs.len() >= 64 {
            if let Some(oldest) = self.graph_order.pop_front() {
                self.graphs.remove(&oldest);
            }
        }
        self.graphs.insert(n, graph);
        self.graph_order.push_back(n);
        self.outputs()
    }
    fn outputs(&self) -> Result<Tensors> {
        let logits = self.logits.read_f32().map_err(error)?;
        let mut outputs = Tensors::new();
        let mut offset = 0;
        for (name, width) in &self.heads {
            outputs.insert(
                name.clone(),
                Tensor {
                    shape: vec![1, *width],
                    data: TensorData::F32(logits[offset..offset + width].to_vec()),
                },
            );
            offset += width;
        }
        Ok(outputs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires an NVIDIA GPU and compiled CUDA kernels"]
    fn device_attention_matches_cpu_with_padding_windows_and_empty_masks() {
        let mut backend = CudaBackend::new();
        for n in [1, 4, 65, 129, 512] {
            let h = 64;
            let heads = 1;
            let qkv: Vec<_> = (0..n * 3 * h)
                .map(|i| ((i * 17 % 101) as f32 - 50.0) / 50.0)
                .collect();
            let input = upload(&mut backend, qkv.clone()).unwrap();
            let rotated = backend.f32_buffer(3 * n * h).unwrap();
            let out = backend.f32_buffer(n * h).unwrap();
            let frequencies: Vec<_> = (0..32).map(|i| 1.0 / (i + 1) as f32).collect();
            let freq = upload(&mut backend, frequencies.clone()).unwrap();
            for window in [None, Some(1), Some(64)] {
                for empty in [false, true] {
                    let mask: Vec<i64> = (0..n).map(|i| i64::from(!empty && i % 3 != 2)).collect();
                    let device_mask = backend
                        .input_i32_buffer(
                            Shape::new(vec![n]),
                            mask.iter().map(|v| *v as i32).collect::<Vec<_>>(),
                        )
                        .unwrap();
                    let config = LayerConfig {
                        window,
                        inverse_frequencies: frequencies.clone(),
                    };
                    let expected = super::super::attention(&qkv, n, h, heads, &mask, &config);
                    for _ in 0..3 {
                        let mut s = backend.begin_submission().unwrap();
                        s.encoder_rope(&input, &freq, &rotated, n, heads, h)
                            .unwrap();
                        s.encoder_attention(&rotated, &device_mask, &out, n, heads, h, window, n)
                            .unwrap();
                        s.commit(CudaCommandWait::Completed).unwrap();
                        let actual = out.read_f32().unwrap();
                        let distance = actual
                            .iter()
                            .zip(&expected)
                            .map(|(a, b)| (a - b).abs())
                            .fold(0.0_f32, f32::max);
                        assert!(
                            distance < 1e-5,
                            "n={n}, window={window:?}, empty={empty}, max error={distance}"
                        );
                    }
                }
            }
        }
    }
}
