//! Device-resident digit inference with three captured float32 kernels.
use super::*;
use psionic_backend_cuda::{CudaBackend, CudaBuffer, CudaCommandWait, CudaGraphExec};
use psionic_core::Shape;
use std::sync::mpsc::{SyncSender, sync_channel};

fn error(e: impl std::fmt::Display) -> Error {
    Error(format!("Native CUDA digit model: {e}"))
}
struct Weights {
    conv1: Vec<f32>,
    bias1: Vec<f32>,
    conv2: Vec<f32>,
    bias2: Vec<f32>,
    fc: Vec<f32>,
    bias_fc: Vec<f32>,
}
struct Request {
    pixels: Vec<f32>,
    reply: SyncSender<Result<Tensors>>,
}
pub(super) struct Digit {
    sender: Option<SyncSender<Request>>,
    worker: Option<std::thread::JoinHandle<()>>,
    pub descriptor: serde_json::Value,
    pub device: String,
    calls: u64,
}
impl Drop for Digit {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Digit {
    pub(super) fn load(model: &mut MnistEngine) -> Result<Self> {
        let weights = Weights {
            conv1: std::mem::take(&mut model.conv1),
            bias1: std::mem::take(&mut model.bias1),
            conv2: std::mem::take(&mut model.conv2),
            bias2: std::mem::take(&mut model.bias2),
            fc: std::mem::take(&mut model.fc),
            bias_fc: std::mem::take(&mut model.bias_fc),
        };
        let (sender, requests) = sync_channel::<Request>(1);
        let (ready, initialization) = sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("psionic-digit-cuda".into())
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
                    let _ = request.reply.send(state.run(&request.pixels));
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
        })
    }
    pub(super) fn run(&mut self, pixels: &[f32]) -> Result<Tensors> {
        let (reply, result) = sync_channel(1);
        self.sender
            .as_ref()
            .ok_or_else(|| error("Worker stopped"))?
            .send(Request {
                pixels: pixels.to_vec(),
                reply,
            })
            .map_err(error)?;
        let output = result.recv().map_err(error)??;
        self.calls += 1;
        Ok(output)
    }
    pub(super) fn report(&self) -> serde_json::Value {
        serde_json::json!({"device":self.descriptor,"placement":"GPU convolution, activation, pooling, and dense head","cpu_operations":"image tensor validation and request dispatch","gpu_operations":"two fused convolution/ReLU/pooling kernels and one dense kernel","calls":self.calls,"host_to_device_logical_bytes":self.calls*784*4,"device_to_host_logical_bytes":self.calls*10*4,"request_completion_waits":self.calls,"activation_allocations_per_call":0,"cuda_graphs":1,"host_control_threads":1,"precision":"f32; no TF32 or reduced-precision operations"})
    }
}
fn upload(backend: &mut CudaBackend, values: Vec<f32>) -> Result<CudaBuffer> {
    backend
        .input_buffer(Shape::new(vec![values.len()]), values)
        .map_err(error)
}
struct State {
    graph: Option<CudaGraphExec>,
    backend: CudaBackend,
    descriptor: serde_json::Value,
    conv1: CudaBuffer,
    bias1: CudaBuffer,
    conv2: CudaBuffer,
    bias2: CudaBuffer,
    fc: CudaBuffer,
    bias_fc: CudaBuffer,
    image: CudaBuffer,
    first: CudaBuffer,
    second: CudaBuffer,
    logits: CudaBuffer,
}
impl State {
    fn load(weights: Weights) -> Result<Self> {
        let mut backend = CudaBackend::new();
        let descriptor = serde_json::to_value(
            backend
                .selected_device()
                .ok_or_else(|| error("CUDA is unavailable"))?,
        )?;
        let conv1 = upload(&mut backend, weights.conv1)?;
        let bias1 = upload(&mut backend, weights.bias1)?;
        let conv2 = upload(&mut backend, weights.conv2)?;
        let bias2 = upload(&mut backend, weights.bias2)?;
        let fc = upload(&mut backend, weights.fc)?;
        let bias_fc = upload(&mut backend, weights.bias_fc)?;
        let image = backend.f32_buffer(784).map_err(error)?;
        let first = backend.f32_buffer(8 * 14 * 14).map_err(error)?;
        let second = backend.f32_buffer(256).map_err(error)?;
        let logits = backend.f32_buffer(10).map_err(error)?;
        Ok(Self {
            graph: None,
            backend,
            descriptor,
            conv1,
            bias1,
            conv2,
            bias2,
            fc,
            bias_fc,
            image,
            first,
            second,
            logits,
        })
    }
    fn run(&mut self, pixels: &[f32]) -> Result<Tensors> {
        self.image.write_f32(pixels).map_err(error)?;
        if let Some(graph) = &self.graph {
            graph.launch(CudaCommandWait::Completed).map_err(error)?;
        } else {
            let mut submission = self.backend.begin_captured_submission().map_err(error)?;
            submission
                .digit_conv_pool(
                    &self.image,
                    &self.conv1,
                    &self.bias1,
                    &self.first,
                    1,
                    28,
                    8,
                    2,
                )
                .map_err(error)?;
            submission
                .digit_conv_pool(
                    &self.first,
                    &self.conv2,
                    &self.bias2,
                    &self.second,
                    8,
                    14,
                    16,
                    3,
                )
                .map_err(error)?;
            submission
                .digit_head(&self.second, &self.fc, &self.bias_fc, &self.logits)
                .map_err(error)?;
            self.graph = Some(
                submission
                    .commit_captured(CudaCommandWait::Completed)
                    .map_err(error)?
                    .1,
            );
        }
        Ok(BTreeMap::from([(
            "Plus214_Output_0".into(),
            Tensor {
                shape: vec![1, 10],
                data: TensorData::F32(self.logits.read_f32().map_err(error)?),
            },
        )]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires an NVIDIA GPU and compiled CUDA kernels"]
    fn fused_convolution_pooling_matches_scalar_padding_and_floor_edges() {
        let mut backend = CudaBackend::new();
        for (width, channels, pool) in [(3, 1, 2), (7, 2, 3), (14, 8, 3), (28, 1, 2)] {
            let pixels: Vec<_> = (0..channels * width * width)
                .map(|i| ((i * 17 % 47) as f32 - 23.0) / 29.0)
                .collect();
            let weights: Vec<_> = (0..3 * channels * 25)
                .map(|i| ((i * 7 % 31) as f32 - 15.0) / 37.0)
                .collect();
            let bias = vec![0.5, -0.2, 0.1];
            let expected = super::super::max_pool(
                &super::super::conv_relu_scalar(&pixels, channels, width, &weights, &bias),
                3,
                width,
                pool,
            );
            let x = upload(&mut backend, pixels).unwrap();
            let w = upload(&mut backend, weights).unwrap();
            let b = upload(&mut backend, bias).unwrap();
            let out = backend.f32_buffer(expected.len()).unwrap();
            let mut submission = backend.begin_submission().unwrap();
            submission
                .digit_conv_pool(&x, &w, &b, &out, channels, width, 3, pool)
                .unwrap();
            submission.commit(CudaCommandWait::Completed).unwrap();
            let actual = out.read_f32().unwrap();
            assert!(
                actual
                    .iter()
                    .zip(&expected)
                    .all(|(a, b)| (a - b).abs() < 1e-4)
            );
        }
    }
}
