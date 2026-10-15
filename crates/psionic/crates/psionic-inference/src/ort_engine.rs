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

pub struct OrtEngine {
    session: Session,
    identity: ExecutionIdentity,
}

fn ort_error(error: impl std::fmt::Display) -> Error {
    Error(format!("ONNX Runtime: {error}"))
}

impl OrtEngine {
    pub fn load(model: &Path, options: &OrtOptions) -> Result<Self> {
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
        let session = Session::builder()
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
            .map_err(ort_error)?
            .commit_from_file(model)
            .map_err(ort_error)?;
        Ok(Self {
            session,
            identity: ExecutionIdentity {
                engine: EngineKind::Ort,
                provider: "CPUExecutionProvider".into(),
                device: "cpu".into(),
                precision: "model-defined".into(),
                threads: options.threads,
                runtime_version: ort::info().into(),
                runtime_sha256: Some(hash),
                cpu_fallback: Some(false),
            },
        })
    }
}

impl ModelEngine for OrtEngine {
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
        Ok(result)
    }
}
