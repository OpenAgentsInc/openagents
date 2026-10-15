//! Shared model execution for native Psionic and optional external engines.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub mod modernbert;
#[cfg(feature = "ort")]
pub mod ort_engine;
pub mod router;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self(value.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self(value.to_string())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineKind {
    Ort,
    Native,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "dtype", content = "values", rename_all = "snake_case")]
pub enum TensorData {
    F32(Vec<f32>),
    I64(Vec<i64>),
    U8(Vec<u8>),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Tensor {
    pub shape: Vec<usize>,
    pub data: TensorData,
}

impl Tensor {
    pub fn validate(&self) -> Result<()> {
        let count = self.shape.iter().try_fold(1_usize, |size, dim| {
            size.checked_mul(*dim)
                .ok_or_else(|| Error("Tensor dimensions are too large".into()))
        })?;
        let len = match &self.data {
            TensorData::F32(v) => {
                if v.iter().any(|x| !x.is_finite()) {
                    return Err(Error("Tensor contains a non-finite value".into()));
                }
                v.len()
            }
            TensorData::I64(v) => v.len(),
            TensorData::U8(v) => v.len(),
        };
        if count != len {
            return Err(Error("Tensor dimensions do not match its values".into()));
        }
        Ok(())
    }

    pub fn floats(&self) -> Result<&[f32]> {
        self.validate()?;
        match &self.data {
            TensorData::F32(v) => Ok(v),
            _ => Err(Error("Expected a float32 tensor".into())),
        }
    }

    pub fn integers(&self) -> Result<&[i64]> {
        self.validate()?;
        match &self.data {
            TensorData::I64(v) => Ok(v),
            _ => Err(Error("Expected an int64 tensor".into())),
        }
    }
}

pub type Tensors = BTreeMap<String, Tensor>;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ExecutionIdentity {
    pub engine: EngineKind,
    pub provider: String,
    pub device: String,
    pub precision: String,
    pub threads: usize,
    pub runtime_version: String,
    pub runtime_sha256: Option<String>,
    /// Actual CPU placement when known; never imply an entire graph ran on GPU.
    pub cpu_fallback: Option<bool>,
}

pub trait ModelEngine: Send {
    fn identity(&self) -> &ExecutionIdentity;
    fn run(&mut self, inputs: &Tensors) -> Result<Tensors>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_shapes_overflow_and_nonfinite_data() {
        for tensor in [
            Tensor {
                shape: vec![2],
                data: TensorData::I64(vec![1]),
            },
            Tensor {
                shape: vec![usize::MAX, 2],
                data: TensorData::U8(vec![]),
            },
            Tensor {
                shape: vec![1],
                data: TensorData::F32(vec![f32::NAN]),
            },
        ] {
            assert!(tensor.validate().is_err());
        }
    }
}
