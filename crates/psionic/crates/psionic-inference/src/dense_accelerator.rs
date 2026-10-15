//! Mixed execution using Psionic's existing device GEMMs and explicit host boundaries.
use crate::{DeviceChoice, Error, Result};
#[cfg(any(feature = "native-cuda", feature = "native-metal"))]
use psionic_core::Shape;
#[cfg(any(feature = "native-cuda", feature = "native-metal"))]
use std::collections::BTreeMap;

#[allow(dead_code)]
enum Backend {
    Unavailable,
    #[cfg(feature = "native-cuda")]
    Cuda {
        backend: psionic_backend_cuda::CudaBackend,
        weights: BTreeMap<usize, psionic_backend_cuda::CudaBuffer>,
    },
    #[cfg(feature = "native-metal")]
    Metal {
        backend: psionic_backend_metal::MetalBackend,
        weights: BTreeMap<usize, psionic_backend_metal::MetalBuffer>,
    },
}

struct Run {
    key: usize,
    input: Vec<f32>,
    m: usize,
    k: usize,
    n: usize,
    reply: std::sync::mpsc::SyncSender<Result<Vec<f32>>>,
}

pub(crate) struct DenseAccelerator {
    sender: Option<std::sync::mpsc::SyncSender<Run>>,
    worker: Option<std::thread::JoinHandle<()>>,
    pub device: String,
    pub provider: String,
    pub descriptor: serde_json::Value,
    uploaded_weight_bytes: u64,
    pub calls: u64,
    host_to_device_bytes: u64,
    device_to_host_bytes: u64,
}
impl Drop for DenseAccelerator {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn error(e: impl std::fmt::Display) -> Error {
    Error(format!("Native accelerator: {e}"))
}

#[allow(unused_variables, unreachable_code)]
impl DenseAccelerator {
    pub fn load(device: DeviceChoice, weights: &[(usize, &[f32], usize, usize)]) -> Result<Self> {
        let owned: Vec<_> = weights
            .iter()
            .map(|(key, v, k, n)| (*key, v.to_vec(), *k, *n))
            .collect();
        let uploaded_weight_bytes = weights.iter().map(|(_, v, _, _)| v.len() as u64 * 4).sum();
        let (sender, receiver) = std::sync::mpsc::sync_channel::<Run>(1);
        let (ready, initialization) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("psionic-device".into())
            .spawn(move || {
                // Device contexts and non-Send buffers are created, used, and dropped here.
                let weights: Vec<_> = owned
                    .iter()
                    .map(|(key, v, k, n)| (*key, v.as_slice(), *k, *n))
                    .collect();
                let (mut backend, descriptor) = match Self::backend(device, &weights) {
                    Ok(value) => value,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                drop(weights);
                drop(owned);
                if ready.send(Ok(descriptor)).is_err() {
                    return;
                }
                while let Ok(run) = receiver.recv() {
                    let _ = run.reply.send(Self::multiply(
                        &mut backend,
                        run.key,
                        &run.input,
                        run.m,
                        run.k,
                        run.n,
                    ));
                }
            })?;
        let descriptor = initialization.recv().map_err(error)??;
        Ok(Self {
            sender: Some(sender),
            worker: Some(worker),
            device: descriptor["device_name"]
                .as_str()
                .unwrap_or("device 0")
                .into(),
            provider: match device {
                DeviceChoice::Cuda => "psionic-cuda-cublas",
                _ => "psionic-metal",
            }
            .into(),
            descriptor,
            uploaded_weight_bytes,
            calls: 0,
            host_to_device_bytes: 0,
            device_to_host_bytes: 0,
        })
    }

    fn backend(
        device: DeviceChoice,
        weights: &[(usize, &[f32], usize, usize)],
    ) -> Result<(Backend, serde_json::Value)> {
        let (backend, descriptor): (Backend, serde_json::Value) = match device {
            #[cfg(feature = "native-cuda")]
            DeviceChoice::Cuda => {
                let mut backend = psionic_backend_cuda::CudaBackend::new();
                let descriptor = serde_json::to_value(
                    backend
                        .selected_device()
                        .ok_or_else(|| error("CUDA is unavailable"))?,
                )?;
                let mut buffers = BTreeMap::new();
                for (key, values, k, n) in weights {
                    buffers.insert(
                        *key,
                        backend
                            .input_buffer(Shape::new(vec![*k, *n]), values.to_vec())
                            .map_err(error)?,
                    );
                }
                (
                    Backend::Cuda {
                        backend,
                        weights: buffers,
                    },
                    descriptor,
                )
            }
            #[cfg(feature = "native-metal")]
            DeviceChoice::Metal => {
                let mut backend = psionic_backend_metal::MetalBackend::new();
                let descriptor = serde_json::to_value(
                    backend
                        .selected_device()
                        .ok_or_else(|| error("Metal is unavailable"))?,
                )?;
                let mut buffers = BTreeMap::new();
                for (key, values, k, n) in weights {
                    buffers.insert(
                        *key,
                        backend
                            .input_buffer(Shape::new(vec![*k, *n]), values.to_vec())
                            .map_err(error)?,
                    );
                }
                (
                    Backend::Metal {
                        backend,
                        weights: buffers,
                    },
                    descriptor,
                )
            }
            _ => {
                return Err(error(
                    "This native accelerator is not enabled in this build",
                ));
            }
        };
        Ok((backend, descriptor))
    }

    pub fn matmul(
        &mut self,
        key: usize,
        input: &[f32],
        m: usize,
        k: usize,
        n: usize,
    ) -> Result<Vec<f32>> {
        if input.len() != m * k {
            return Err(error("Unexpected GEMM input dimensions"));
        }
        let (reply, result) = std::sync::mpsc::sync_channel(1);
        self.sender
            .as_ref()
            .ok_or_else(|| error("Native worker stopped"))?
            .send(Run {
                key,
                input: input.to_vec(),
                m,
                k,
                n,
                reply,
            })
            .map_err(error)?;
        let output = result.recv().map_err(error)??;
        self.calls += 1;
        self.host_to_device_bytes += input.len() as u64 * 4;
        self.device_to_host_bytes += m as u64 * n as u64 * 4;
        Ok(output)
    }
    fn multiply(
        backend: &mut Backend,
        key: usize,
        input: &[f32],
        m: usize,
        k: usize,
        n: usize,
    ) -> Result<Vec<f32>> {
        match backend {
            Backend::Unavailable => Err(error("Native accelerator is unavailable")),
            #[cfg(feature = "native-cuda")]
            Backend::Cuda { backend, weights } => {
                use psionic_backend_cuda::CudaCommandWait;
                let left = backend
                    .input_buffer(Shape::new(vec![m, k]), input.to_vec())
                    .map_err(error)?;
                let right = weights
                    .get(&key)
                    .ok_or_else(|| error("Missing device weight"))?;
                let output = backend.f32_buffer(m * n).map_err(error)?;
                let mut submission = backend.begin_submission().map_err(error)?;
                submission
                    .matmul(&left, right, &output, m, k, n)
                    .map_err(error)?;
                submission
                    .commit(CudaCommandWait::Completed)
                    .map_err(error)?;
                output.read_f32().map_err(error)
            }
            #[cfg(feature = "native-metal")]
            Backend::Metal { backend, weights } => {
                use psionic_backend_metal::MetalCommandWait;
                let left = backend
                    .input_buffer(Shape::new(vec![m, k]), input.to_vec())
                    .map_err(error)?;
                let right = weights
                    .get(&key)
                    .ok_or_else(|| error("Missing device weight"))?;
                let output = backend
                    .zeros_buffer(Shape::new(vec![m, n]))
                    .map_err(error)?;
                let mut submission = backend
                    .begin_submission("small-model GEMM")
                    .map_err(error)?;
                backend
                    .encode_matmul_submission(&mut submission, &left, right, &output, m, k, n)
                    .map_err(error)?;
                submission
                    .commit(MetalCommandWait::Completed)
                    .map_err(error)?;
                output.read_f32().map_err(error)
            }
        }
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({"device":self.descriptor,"placement":"mixed CPU and GPU","gpu_operations":"dense GEMMs",
            "gemm_calls":self.calls,"nvidia_tf32_override":std::env::var("NVIDIA_TF32_OVERRIDE").ok(),"host_control_threads":1,"uploaded_weight_bytes":self.uploaded_weight_bytes,
            "host_to_device_logical_bytes":self.host_to_device_bytes,"device_to_host_logical_bytes":self.device_to_host_bytes,
            "transfer_note":"Logical buffer writes/reads; unified-memory Metal does not imply PCIe copies. Each GEMM waits for completion."})
    }
}
