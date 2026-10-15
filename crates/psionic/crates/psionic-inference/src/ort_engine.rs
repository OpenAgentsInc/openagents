//! Optional in-process ONNX Runtime execution with explicit library selection.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::{DynValue, Tensor as OrtTensor},
};

use crate::{
    EngineKind, Error, ExecutionIdentity, ModelEngine, Result, Tensor, TensorData, Tensors,
    router::file_sha256,
};

static LIBRARY: Mutex<Option<(PathBuf, String)>> = Mutex::new(None);

#[derive(Clone, Debug)]
pub struct OrtOptions {
    pub library: PathBuf,
    /// Required SHA-256 of the explicitly provisioned native library.
    pub library_sha256: String,
    pub threads: usize,
}

#[derive(Clone, Copy, Debug, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreMlUnits {
    All,
    #[default]
    CpuAndGpu,
    CpuAndNeuralEngine,
    CpuOnly,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct OrtExecution {
    pub device: crate::DeviceChoice,
    pub device_id: i32,
    pub coreml_units: CoreMlUnits,
    /// CPU graph partitions require explicit opt-in for an accelerator.
    pub allow_cpu_fallback: bool,
    pub profile: Option<PathBuf>,
}

pub struct OrtEngine {
    session: Session,
    identity: ExecutionIdentity,
    execution: OrtExecution,
    profile_finished: bool,
    calls: u64,
    host_input_bytes: u64,
    host_output_bytes: u64,
}

fn ort_error(error: impl std::fmt::Display) -> Error {
    Error(format!("ONNX Runtime: {error}"))
}

impl OrtEngine {
    pub fn load(model: &Path, options: &OrtOptions) -> Result<Self> {
        Self::load_with_execution(model, options, &OrtExecution::default())
    }

    pub fn load_with_execution(
        model: &Path,
        options: &OrtOptions,
        execution: &OrtExecution,
    ) -> Result<Self> {
        use crate::DeviceChoice;
        if execution.device == DeviceChoice::Cuda {
            validate_cuda_loading(
                std::env::var("CUDA_MODULE_LOADING").ok().as_deref(),
                std::env::var("CUDA_MODULE_DATA_LOADING").ok().as_deref(),
            )?;
        }
        if execution.device == DeviceChoice::Metal || execution.device_id < 0 {
            return Err(Error(
                "Choose CPU, CUDA, or Core ML for ONNX Runtime".into(),
            ));
        }
        if execution.device == DeviceChoice::CoreMl
            && !cfg!(any(target_os = "macos", target_os = "ios"))
        {
            return Err(Error("Core ML requires an Apple platform".into()));
        }
        if !(1..=256).contains(&options.threads) {
            return Err(Error("Choose between 1 and 256 CPU threads".into()));
        }
        let library = options.library.canonicalize()?;
        let hash = file_sha256(&library)?;
        if hash != options.library_sha256 {
            return Err(Error("ONNX Runtime library checksum does not match".into()));
        }
        {
            let mut loaded = LIBRARY
                .lock()
                .map_err(|_| Error("Cannot initialize ONNX Runtime".into()))?;
            match loaded.as_ref() {
                Some((path, loaded_hash)) if path != &library || loaded_hash != &hash => {
                    return Err(Error(
                        "This process already loaded a different ONNX Runtime library".into(),
                    ));
                }
                Some(_) => {}
                None => {
                    ort::init_from(&library).map_err(ort_error)?.commit();
                    *loaded = Some((library, hash.clone()));
                }
            }
        }
        let mut builder = Session::builder()
            .map_err(ort_error)?
            .with_no_environment_execution_providers()
            .map_err(ort_error)?
            .with_intra_threads(options.threads)
            .map_err(ort_error)?
            .with_inter_threads(1)
            .map_err(ort_error)?
            .with_parallel_execution(false)
            .map_err(ort_error)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(ort_error)?;
        let (provider, device) = match execution.device {
            DeviceChoice::Cpu => ("CPUExecutionProvider", "cpu".to_owned()),
            DeviceChoice::Cuda => {
                builder = builder
                    .with_execution_providers([ort::ep::CUDA::default()
                        .with_device_id(execution.device_id)
                        .with_tf32(false)
                        .with_conv_algorithm_search(ort::ep::cuda::ConvAlgorithmSearch::Default)
                        .build()
                        .error_on_failure()])
                    .map_err(ort_error)?;
                (
                    "CUDAExecutionProvider",
                    format!("cuda:{}", execution.device_id),
                )
            }
            DeviceChoice::CoreMl => {
                use ort::ep::coreml::{ComputeUnits, ModelFormat};
                let units = match execution.coreml_units {
                    CoreMlUnits::All => ComputeUnits::All,
                    CoreMlUnits::CpuAndGpu => ComputeUnits::CPUAndGPU,
                    CoreMlUnits::CpuAndNeuralEngine => ComputeUnits::CPUAndNeuralEngine,
                    CoreMlUnits::CpuOnly => ComputeUnits::CPUOnly,
                };
                builder = builder
                    .with_execution_providers([ort::ep::CoreML::default()
                        .with_compute_units(units)
                        .with_model_format(ModelFormat::MLProgram)
                        .build()
                        .error_on_failure()])
                    .map_err(ort_error)?;
                (
                    "CoreMLExecutionProvider",
                    format!(
                        "coreml:{:?}; physical placement unknown",
                        execution.coreml_units
                    ),
                )
            }
            DeviceChoice::Metal => unreachable!(),
        };
        if execution.device != DeviceChoice::Cpu && !execution.allow_cpu_fallback {
            builder = builder
                .with_config_entry("session.disable_cpu_ep_fallback", "1")
                .map_err(ort_error)?;
        }
        if let Some(path) = &execution.profile {
            builder = builder.with_profiling(path).map_err(ort_error)?;
        }
        let session = builder.commit_from_file(model).map_err(ort_error)?;
        Ok(Self {
            session,
            execution: execution.clone(),
            profile_finished: false,
            calls: 0,
            host_input_bytes: 0,
            host_output_bytes: 0,
            identity: ExecutionIdentity {
                engine: EngineKind::Ort,
                provider: provider.into(),
                device,
                precision: "model-defined".into(),
                threads: options.threads,
                runtime_version: ort::info().into(),
                runtime_sha256: Some(hash),
                cpu_fallback: if execution.device == DeviceChoice::Cpu {
                    Some(false)
                } else {
                    None
                },
            },
        })
    }
}

impl ModelEngine for OrtEngine {
    fn execution_report(&self) -> serde_json::Value {
        serde_json::json!({"configuration":self.execution,"cuda_convolution_search":(self.execution.device==crate::DeviceChoice::Cuda).then_some("default (fallback heuristic)"),"cuda_module_loading":std::env::var("CUDA_MODULE_LOADING").ok(),"cuda_module_data_loading":std::env::var("CUDA_MODULE_DATA_LOADING").ok(),"calls":self.calls,"host_input_bytes":self.host_input_bytes,"host_output_bytes":self.host_output_bytes,
            "placement":"Use the separate profiling report; provider selection alone does not establish placement.",
            "transfer_note":"Host tensor boundary bytes are logical I/O, not measured bus traffic. ORT can copy host I/O outside profiled graph nodes. Core ML internal transfers are unknown."})
    }
    fn finish_profiling(&mut self) -> Result<Option<serde_json::Value>> {
        if self.execution.profile.is_none() || self.profile_finished {
            return Ok(None);
        }
        let path = self.session.end_profiling().map_err(ort_error)?;
        self.profile_finished = true;
        let events: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(&path)?)?;
        let report = summarize_profile(&events);
        if self.execution.device != crate::DeviceChoice::Cpu {
            self.identity.cpu_fallback = if report["nodes_by_provider"]
                .as_object()
                .is_none_or(|v| v.is_empty())
            {
                None
            } else {
                Some(
                    report["nodes_by_provider"]["CPUExecutionProvider"]
                        .as_u64()
                        .unwrap_or(0)
                        > 0,
                )
            };
        }
        Ok(Some(
            serde_json::json!({"profile_sha256":file_sha256(Path::new(&path))?,"summary":report,
            "note":"Counts cover the profiled reference/warm-up calls. Core ML provider placement does not reveal CPU/GPU/ANE placement inside Core ML."}),
        ))
    }

    fn identity(&self) -> &ExecutionIdentity {
        &self.identity
    }

    fn run(&mut self, inputs: &Tensors) -> Result<Tensors> {
        if inputs.len() != self.session.inputs().len()
            || self
                .session
                .inputs()
                .iter()
                .any(|x| !inputs.contains_key(x.name()))
        {
            return Err(Error("Input names do not match this ONNX model".into()));
        }
        let values = inputs
            .iter()
            .map(|(name, tensor)| -> Result<(String, DynValue)> {
                tensor.validate()?;
                let shape = tensor.shape.clone();
                let value = match &tensor.data {
                    TensorData::F32(v) => OrtTensor::from_array((shape, v.clone()))
                        .map_err(ort_error)?
                        .into_dyn(),
                    TensorData::I64(v) => OrtTensor::from_array((shape, v.clone()))
                        .map_err(ort_error)?
                        .into_dyn(),
                    TensorData::U8(v) => OrtTensor::from_array((shape, v.clone()))
                        .map_err(ort_error)?
                        .into_dyn(),
                };
                Ok((name.clone(), value))
            })
            .collect::<Result<Vec<_>>>()?;
        let outputs = self.session.run(values).map_err(ort_error)?;
        let mut result = Tensors::new();
        for (name, output) in outputs.iter() {
            let (shape, data) = if let Ok((shape, values)) = output.try_extract_tensor::<f32>() {
                (shape, TensorData::F32(values.to_vec()))
            } else if let Ok((shape, values)) = output.try_extract_tensor::<i64>() {
                (shape, TensorData::I64(values.to_vec()))
            } else if let Ok((shape, values)) = output.try_extract_tensor::<u8>() {
                (shape, TensorData::U8(values.to_vec()))
            } else {
                return Err(Error(format!("Unsupported output type for {name}")));
            };
            let shape = shape
                .iter()
                .map(|n| usize::try_from(*n).map_err(|_| Error("Invalid output dimensions".into())))
                .collect::<Result<_>>()?;
            let tensor = Tensor { shape, data };
            tensor.validate()?;
            result.insert(name.into(), tensor);
        }
        self.calls += 1;
        let bytes = |t: &Tensor| match &t.data {
            TensorData::F32(v) => v.len() as u64 * 4,
            TensorData::I64(v) => v.len() as u64 * 8,
            TensorData::U8(v) => v.len() as u64,
        };
        self.host_input_bytes += inputs.values().map(bytes).sum::<u64>();
        self.host_output_bytes += result.values().map(bytes).sum::<u64>();
        Ok(result)
    }
}

fn validate_cuda_loading(modules: Option<&str>, data: Option<&str>) -> Result<()> {
    if modules != Some("EAGER") || data.is_some_and(|v| v != "EAGER") {
        return Err(Error("Set CUDA_MODULE_LOADING=EAGER and CUDA_MODULE_DATA_LOADING=EAGER before starting this process; lazy loading failed the CUDA shutdown checks".into()));
    }
    Ok(())
}

fn summarize_profile(events: &[serde_json::Value]) -> serde_json::Value {
    let mut counts = std::collections::BTreeMap::<String, u64>::new();
    let mut operators =
        std::collections::BTreeMap::<String, std::collections::BTreeMap<String, u64>>::new();
    let mut transfers = std::collections::BTreeMap::<String, serde_json::Value>::new();
    let mut transfer_count = 0_u64;
    for event in events {
        if event["cat"] != "Node" {
            continue;
        }
        let Some(provider) = event["args"]["provider"].as_str() else {
            continue;
        };
        *counts.entry(provider.to_owned()).or_default() += 1;
        let op = event["args"]["op_name"].as_str().unwrap_or("unknown");
        *operators
            .entry(provider.into())
            .or_default()
            .entry(op.into())
            .or_default() += 1;
        if op.contains("Memcpy") {
            transfer_count += 1;
            let entry=transfers.entry(op.into()).or_insert_with(||serde_json::json!({"operator":op,"provider":provider,"count":0,"duration_us":0,"output_bytes":0}));
            entry["count"] = serde_json::json!(entry["count"].as_u64().unwrap_or(0) + 1);
            entry["duration_us"] = serde_json::json!(
                entry["duration_us"].as_u64().unwrap_or(0) + event["dur"].as_u64().unwrap_or(0)
            );
            let bytes = event["args"]["output_size"]
                .as_str()
                .and_then(|s| s.parse::<u64>().ok())
                .or_else(|| event["args"]["output_size"].as_u64());
            entry["output_bytes"] = match (entry["output_bytes"].as_u64(), bytes) {
                (Some(a), Some(b)) => serde_json::json!(a + b),
                _ => serde_json::Value::Null,
            };
        }
    }
    serde_json::json!({"nodes_by_provider":counts,"operators_by_provider":operators,"transfer_events":transfers.values().collect::<Vec<_>>(),"transfer_count":transfer_count})
}
#[cfg(test)]
mod tests {
    #[test]
    fn cuda_requires_the_qualified_module_loading_settings() {
        for settings in [
            (None, None),
            (Some("LAZY"), None),
            (Some("EAGER"), Some("LAZY")),
        ] {
            assert!(super::validate_cuda_loading(settings.0, settings.1).is_err());
        }
        assert!(super::validate_cuda_loading(Some("EAGER"), None).is_ok());
        assert!(super::validate_cuda_loading(Some("EAGER"), Some("EAGER")).is_ok());
    }

    #[test]
    fn profile_reports_cpu_partitions_and_copies_without_guessing() {
        let report = super::summarize_profile(&[
            serde_json::json!({"cat":"Node","args":{"provider":"CPUExecutionProvider","op_name":"Shape"}}),
            serde_json::json!({"cat":"Node","dur":5,"args":{"provider":"CUDAExecutionProvider","op_name":"MemcpyFromHost","output_size":"12"}}),
            serde_json::json!({"cat":"Session","args":{}}),
        ]);
        assert_eq!(report["nodes_by_provider"]["CPUExecutionProvider"], 1);
        assert_eq!(report["transfer_count"], 1);
        assert_eq!(report["transfer_events"][0]["output_bytes"], 12);
    }
}
