//! Core tensor, shape, dtype, and device types for Rustygrad.
//!
//! This crate intentionally stays small and product-agnostic. It owns public
//! engine-facing metadata, not backend execution logic.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "tensor facade and foundational engine types";

/// Stable tensor identifier used across the Rustygrad crates.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct TensorId(pub u32);

impl fmt::Display for TensorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0)
    }
}

/// Supported scalar data types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DType {
    /// 32-bit floating point values.
    F32,
}

impl DType {
    /// Returns the size of a single element in bytes.
    #[must_use]
    pub const fn element_size_bytes(self) -> usize {
        match self {
            Self::F32 => 4,
        }
    }
}

/// Runtime backend family for a device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceKind {
    /// Host CPU execution.
    Cpu,
    /// Apple Metal execution.
    Metal,
    /// AMD KFD execution.
    AmdKfd,
    /// AMD userspace execution.
    AmdUserspace,
}

impl fmt::Display for DeviceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Cpu => "cpu",
            Self::Metal => "metal",
            Self::AmdKfd => "amd_kfd",
            Self::AmdUserspace => "amd_userspace",
        };
        f.write_str(label)
    }
}

/// Logical device descriptor used by graph, runtime, and provider layers.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Device {
    kind: DeviceKind,
    ordinal: u16,
    label: Option<String>,
}

impl Device {
    /// Creates a new device descriptor.
    #[must_use]
    pub fn new(kind: DeviceKind, ordinal: u16, label: Option<String>) -> Self {
        Self {
            kind,
            ordinal,
            label,
        }
    }

    /// Returns a default CPU device.
    #[must_use]
    pub fn cpu() -> Self {
        Self::new(DeviceKind::Cpu, 0, Some(String::from("cpu:0")))
    }

    /// Returns the device kind.
    #[must_use]
    pub const fn kind(&self) -> DeviceKind {
        self.kind
    }

    /// Returns the device ordinal.
    #[must_use]
    pub const fn ordinal(&self) -> u16 {
        self.ordinal
    }

    /// Returns an optional friendly label.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(label) = self.label() {
            write!(f, "{label}")
        } else {
            write!(f, "{}:{}", self.kind, self.ordinal)
        }
    }
}

/// Tensor shape descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Shape {
    dims: Vec<usize>,
}

impl Shape {
    /// Creates a new shape.
    #[must_use]
    pub fn new(dims: impl Into<Vec<usize>>) -> Self {
        Self { dims: dims.into() }
    }

    /// Creates a scalar shape.
    #[must_use]
    pub fn scalar() -> Self {
        Self { dims: Vec::new() }
    }

    /// Returns the shape dimensions.
    #[must_use]
    pub fn dims(&self) -> &[usize] {
        &self.dims
    }

    /// Returns the rank.
    #[must_use]
    pub fn rank(&self) -> usize {
        self.dims.len()
    }

    /// Returns the number of addressable elements.
    #[must_use]
    pub fn element_count(&self) -> usize {
        if self.dims.is_empty() {
            1
        } else {
            self.dims.iter().product()
        }
    }
}

impl fmt::Display for Shape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.dims)
    }
}

/// Static tensor metadata.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TensorSpec {
    shape: Shape,
    dtype: DType,
    device: Device,
}

impl TensorSpec {
    /// Creates a new tensor specification.
    #[must_use]
    pub fn new(shape: Shape, dtype: DType, device: Device) -> Self {
        Self {
            shape,
            dtype,
            device,
        }
    }

    /// Returns the tensor shape.
    #[must_use]
    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    /// Returns the tensor dtype.
    #[must_use]
    pub const fn dtype(&self) -> DType {
        self.dtype
    }

    /// Returns the target device.
    #[must_use]
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Returns a copy with a different shape.
    #[must_use]
    pub fn with_shape(&self, shape: Shape) -> Self {
        Self::new(shape, self.dtype, self.device.clone())
    }

    /// Returns the number of addressable elements.
    #[must_use]
    pub fn element_count(&self) -> usize {
        self.shape.element_count()
    }
}

/// Small data container used for constants and host-visible results.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TensorData {
    /// 32-bit floating point tensor payload.
    F32(Vec<f32>),
}

impl TensorData {
    /// Returns the element count of the payload.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::F32(values) => values.len(),
        }
    }

    /// Returns whether the payload is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the payload as an `f32` slice.
    #[must_use]
    pub fn as_f32_slice(&self) -> &[f32] {
        match self {
            Self::F32(values) => values.as_slice(),
        }
    }
}

/// High-level operation provenance for a lazy tensor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LazyOp {
    /// Graph input.
    Input { name: String },
    /// Constant tensor.
    Constant,
    /// Binary add.
    Add,
    /// Binary multiply.
    Mul,
    /// Matrix multiplication.
    Matmul,
    /// Tensor reshape.
    Reshape,
    /// Full reduction to a scalar.
    ReduceSum,
}

/// Public tensor handle produced by graph construction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tensor {
    id: TensorId,
    spec: TensorSpec,
    op: LazyOp,
}

impl Tensor {
    /// Creates a new tensor handle.
    #[must_use]
    pub fn new(id: TensorId, spec: TensorSpec, op: LazyOp) -> Self {
        Self { id, spec, op }
    }

    /// Returns the tensor identifier.
    #[must_use]
    pub const fn id(&self) -> TensorId {
        self.id
    }

    /// Returns the tensor specification.
    #[must_use]
    pub fn spec(&self) -> &TensorSpec {
        &self.spec
    }

    /// Returns the lazy operation provenance.
    #[must_use]
    pub fn op(&self) -> &LazyOp {
        &self.op
    }
}

#[cfg(test)]
mod tests {
    use super::{DType, Device, Shape, TensorSpec};

    #[test]
    fn scalar_shape_counts_as_one_element() {
        assert_eq!(Shape::scalar().element_count(), 1);
    }

    #[test]
    fn dense_shape_reports_element_count() {
        assert_eq!(Shape::new(vec![2, 3, 4]).element_count(), 24);
    }

    #[test]
    fn tensor_spec_retains_device_and_dtype() {
        let spec = TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu());

        assert_eq!(spec.dtype(), DType::F32);
        assert_eq!(spec.device().kind(), super::DeviceKind::Cpu);
    }
}
