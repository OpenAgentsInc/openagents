//! Device-resident float32 encoder; only token inputs and final logits cross the boundary.
use super::*;
use psionic_backend_metal::small_models::{ResidentBuffer, ResidentMetal};
use std::sync::mpsc::{SyncSender, sync_channel};

fn error(e: impl std::fmt::Display) -> Error {
    Error(format!("Native Metal encoder: {e}"))
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
            .name("psionic-encoder-metal".into())
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
                .unwrap_or("Metal device 0")
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
        serde_json::json!({"device":self.descriptor,"placement":"GPU encoder and heads","cpu_operations":"input validation and request dispatch","gpu_operations":"uint8 embedding gather/dequantization, LayerNorm, float32 GEMMs, RoPE, masked attention, erf GeGLU, residuals and heads","calls":self.calls,"host_to_device_logical_bytes":self.input_bytes,"device_to_host_logical_bytes":self.calls*22*4,"request_completion_waits":self.calls,"activation_allocations_per_call":0,"command_buffers_per_call":1,"host_control_threads":1,"precision":"float32; fast math disabled","qualification":"Apple device correctness and Core ML performance comparison required"})
    }
}
struct DeviceLayer {
    attn_norm: Option<ResidentBuffer>,
    mlp_norm: ResidentBuffer,
    qkv: ResidentBuffer,
    attn_out: ResidentBuffer,
    up: ResidentBuffer,
    down: ResidentBuffer,
    frequencies: ResidentBuffer,
}
struct State {
    backend: ResidentMetal,
    descriptor: serde_json::Value,
    config: ModernBertConfig,
    vocab: usize,
    embedding: ResidentBuffer,
    embedding_norm: ResidentBuffer,
    final_norm: ResidentBuffer,
    layers: Vec<DeviceLayer>,
    head_weights: ResidentBuffer,
    head_bias: ResidentBuffer,
    heads: Vec<(String, usize)>,
    ids: ResidentBuffer,
    mask: ResidentBuffer,
    hidden: ResidentBuffer,
    normalized: ResidentBuffer,
    qkv: ResidentBuffer,
    rotated: ResidentBuffer,
    context: ResidentBuffer,
    projected: ResidentBuffer,
    up: ResidentBuffer,
    gated: ResidentBuffer,
    logits: ResidentBuffer,
}
impl State {
    fn load(weights: Weights) -> Result<Self> {
        let backend = ResidentMetal::new().map_err(error)?;
        let descriptor = serde_json::json!({"device_name":backend.device_name(),"backend":"metal","precision":"f32"});
        let h = weights.config.hidden;
        let f = weights.config.intermediate;
        let vocab = weights.embedding.len() / h;
        let embedding = backend.bytes(&weights.embedding).map_err(error)?;
        let embedding_norm = backend.f32s(&weights.embedding_norm).map_err(error)?;
        let final_norm = backend.f32s(&weights.final_norm).map_err(error)?;
        let mut layers = Vec::new();
        for (l, c) in weights.layers.into_iter().zip(&weights.config.layers) {
            layers.push(DeviceLayer {
                attn_norm: l
                    .attn_norm
                    .map(|v| backend.f32s(&v).map_err(error))
                    .transpose()?,
                mlp_norm: backend.f32s(&l.mlp_norm).map_err(error)?,
                qkv: backend.f32s(&l.qkv).map_err(error)?,
                attn_out: backend.f32s(&l.attn_out).map_err(error)?,
                up: backend.f32s(&l.up).map_err(error)?,
                down: backend.f32s(&l.down).map_err(error)?,
                frequencies: backend.f32s(&c.inverse_frequencies).map_err(error)?,
            });
        }
        let head_weights = backend.f32s(&weights.head_weights).map_err(error)?;
        let head_bias = backend.f32s(&weights.head_bias).map_err(error)?;
        let ids = backend.buffer(512 * 4);
        let mask = backend.buffer(512 * 4);
        let hidden = backend.buffer(512 * h * 4);
        let normalized = backend.buffer(512 * h * 4);
        let qkv = backend.buffer(512 * 3 * h * 4);
        let rotated = backend.buffer(512 * 3 * h * 4);
        let context = backend.buffer(512 * h * 4);
        let projected = backend.buffer(512 * h * 4);
        let up = backend.buffer(512 * 2 * f * 4);
        let gated = backend.buffer(512 * f * 4);
        let logits = backend.buffer(22 * 4);
        Ok(Self {
            backend,
            descriptor,
            config: weights.config,
            vocab,
            embedding,
            embedding_norm,
            final_norm,
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
            projected,
            up,
            gated,
            logits,
        })
    }
    fn run(&mut self, ids: &[i32], mask: &[i32]) -> Result<Tensors> {
        self.ids.write_i32(ids).map_err(error)?;
        self.mask.write_i32(mask).map_err(error)?;
        let n = ids.len();
        let h = self.config.hidden;
        let f = self.config.intermediate;
        let eps = self.config.epsilon;
        let s = self.backend.batch();
        s.embed(
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
        s.norm(&self.hidden, &self.embedding_norm, &self.hidden, n, h, eps)
            .map_err(error)?;
        for (i, layer) in self.layers.iter().enumerate() {
            if let Some(norm) = &layer.attn_norm {
                s.norm(&self.hidden, norm, &self.normalized, n, h, eps)
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
            s.rope(
                &self.qkv,
                &layer.frequencies,
                &self.rotated,
                n,
                self.config.heads,
                h / self.config.heads,
            )
            .map_err(error)?;
            let rows = if i + 1 == self.layers.len() { 1 } else { n };
            s.attention(
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
            s.matmul(&self.context, &layer.attn_out, &self.projected, rows, h, h)
                .map_err(error)?;
            s.add(&self.projected, &self.hidden, rows * h)
                .map_err(error)?;
            s.norm(&self.hidden, &layer.mlp_norm, &self.context, rows, h, eps)
                .map_err(error)?;
            s.matmul(&self.context, &layer.up, &self.up, rows, h, 2 * f)
                .map_err(error)?;
            s.geglu(&self.up, &self.gated, rows, f).map_err(error)?;
            s.matmul(&self.gated, &layer.down, &self.projected, rows, f, h)
                .map_err(error)?;
            s.add(&self.projected, &self.hidden, rows * h)
                .map_err(error)?;
        }
        s.norm(&self.hidden, &self.final_norm, &self.normalized, 1, h, eps)
            .map_err(error)?;
        s.matmul(&self.normalized, &self.head_weights, &self.logits, 1, h, 22)
            .map_err(error)?;
        s.bias(&self.logits, &self.head_bias, 22).map_err(error)?;
        s.commit_wait().map_err(error)?;
        let logits = self.logits.read_f32(22).map_err(error)?;
        let mut output = Tensors::new();
        let mut offset = 0;
        for (name, width) in &self.heads {
            output.insert(
                name.clone(),
                Tensor {
                    shape: vec![1, *width],
                    data: TensorData::F32(logits[offset..offset + width].to_vec()),
                },
            );
            offset += width;
        }
        Ok(output)
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires an Apple GPU"]
    fn device_attention_matches_cpu_with_padding_windows_and_empty_masks() {
        let backend = ResidentMetal::new().unwrap();
        for n in [1, 4, 65, 129, 512] {
            let h = 64;
            let heads = 1;
            let qkv: Vec<_> = (0..n * 3 * h)
                .map(|i| ((i * 17 % 101) as f32 - 50.0) / 50.0)
                .collect();
            let input = backend.f32s(&qkv).unwrap();
            let rotated = backend.buffer(3 * n * h * 4);
            let out = backend.buffer(n * h * 4);
            let frequencies: Vec<_> = (0..32).map(|i| 1.0 / (i + 1) as f32).collect();
            let freq = backend.f32s(&frequencies).unwrap();
            for window in [None, Some(1), Some(64)] {
                for empty in [false, true] {
                    let mask: Vec<i64> = (0..n).map(|i| i64::from(!empty && i % 3 != 2)).collect();
                    let device_mask = backend.buffer(n * 4);
                    device_mask
                        .write_i32(&mask.iter().map(|v| *v as i32).collect::<Vec<_>>())
                        .unwrap();
                    let config = LayerConfig {
                        window,
                        inverse_frequencies: frequencies.clone(),
                    };
                    let expected = super::super::attention(&qkv, n, h, heads, &mask, &config);
                    for _ in 0..3 {
                        let s = backend.batch();
                        s.rope(&input, &freq, &rotated, n, heads, h).unwrap();
                        s.attention(&rotated, &device_mask, &out, n, heads, h, window, n)
                            .unwrap();
                        s.commit_wait().unwrap();
                        let actual = out.read_f32(n * h).unwrap();
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
