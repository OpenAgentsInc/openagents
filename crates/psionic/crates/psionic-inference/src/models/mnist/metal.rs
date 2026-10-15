//! Device-resident float32 digit inference in one Metal command buffer.
use super::*;
use psionic_backend_metal::small_models::{ResidentBuffer, ResidentMetal};
use std::sync::mpsc::{SyncSender, sync_channel};

fn error(e: impl std::fmt::Display) -> Error {
    Error(format!("Native Metal digit model: {e}"))
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
            .name("psionic-digit-metal".into())
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
                .unwrap_or("Metal device 0")
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
        serde_json::json!({"device":self.descriptor,"placement":"GPU convolution, activation, pooling, and dense head","cpu_operations":"image validation and request dispatch","calls":self.calls,"host_to_device_logical_bytes":self.calls*784*4,"device_to_host_logical_bytes":self.calls*10*4,"request_completion_waits":self.calls,"activation_allocations_per_call":0,"command_buffers_per_call":1,"precision":"float32; fast math disabled","qualification":"Apple device correctness and Core ML comparison required"})
    }
}
struct State {
    backend: ResidentMetal,
    descriptor: serde_json::Value,
    conv1: ResidentBuffer,
    bias1: ResidentBuffer,
    conv2: ResidentBuffer,
    bias2: ResidentBuffer,
    fc: ResidentBuffer,
    bias_fc: ResidentBuffer,
    image: ResidentBuffer,
    first: ResidentBuffer,
    second: ResidentBuffer,
    logits: ResidentBuffer,
}
impl State {
    fn load(weights: Weights) -> Result<Self> {
        let backend = ResidentMetal::new().map_err(error)?;
        let descriptor = serde_json::json!({"device_name":backend.device_name(),"backend":"metal","precision":"f32"});
        let conv1 = backend.f32s(&weights.conv1).map_err(error)?;
        let bias1 = backend.f32s(&weights.bias1).map_err(error)?;
        let conv2 = backend.f32s(&weights.conv2).map_err(error)?;
        let bias2 = backend.f32s(&weights.bias2).map_err(error)?;
        let fc = backend.f32s(&weights.fc).map_err(error)?;
        let bias_fc = backend.f32s(&weights.bias_fc).map_err(error)?;
        let image = backend.buffer(784 * 4);
        let first = backend.buffer(8 * 14 * 14 * 4);
        let second = backend.buffer(256 * 4);
        let logits = backend.buffer(10 * 4);
        Ok(Self {
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
        let s = self.backend.batch();
        s.digit_conv_pool(
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
        s.digit_conv_pool(
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
        s.digit_head(&self.second, &self.fc, &self.bias_fc, &self.logits)
            .map_err(error)?;
        s.commit_wait().map_err(error)?;
        Ok(BTreeMap::from([(
            "Plus214_Output_0".into(),
            Tensor {
                shape: vec![1, 10],
                data: TensorData::F32(self.logits.read_f32(10).map_err(error)?),
            },
        )]))
    }
}
