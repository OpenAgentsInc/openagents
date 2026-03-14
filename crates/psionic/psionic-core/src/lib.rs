//! Core tensor, shape, dtype, device, and layout types for Psionic.
//!
//! This crate intentionally stays small and product-agnostic. It owns public
//! engine-facing metadata, not backend execution logic.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "tensor facade and foundational engine types";

/// Stable tensor identifier used across the Psionic crates.
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
    /// 16-bit IEEE 754 half-precision floating point values.
    F16,
    /// 16-bit bfloat values.
    BF16,
    /// 8-bit signed integer values.
    I8,
}

impl DType {
    /// Returns the size of a single element in bytes.
    #[must_use]
    pub const fn element_size_bytes(self) -> usize {
        match self {
            Self::F32 => 4,
            Self::F16 | Self::BF16 => 2,
            Self::I8 => 1,
        }
    }
}

/// Quantization mode for stored model weights.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationMode {
    /// Unquantized weights.
    None,
    /// Symmetric int8 quantization with explicit scale tensors.
    Int8Symmetric,
    /// GGML/GGUF MXFP4 block quantization.
    GgmlMxfp4,
    /// GGML/GGUF Q4_0 block quantization.
    GgmlQ4_0,
    /// GGML/GGUF Q4_1 block quantization.
    GgmlQ4_1,
    /// GGML/GGUF Q8_0 block quantization.
    GgmlQ8_0,
}

impl QuantizationMode {
    /// Returns the GGML block shape for the quantization mode when one exists.
    #[must_use]
    pub const fn ggml_block_spec(self) -> Option<(usize, usize)> {
        match self {
            Self::GgmlMxfp4 => Some((32, 17)),
            Self::GgmlQ4_0 => Some((32, 18)),
            Self::GgmlQ4_1 => Some((32, 20)),
            Self::GgmlQ8_0 => Some((32, 34)),
            Self::None | Self::Int8Symmetric => None,
        }
    }

    /// Returns the block layout for a tensor with the provided logical shape.
    #[must_use]
    pub fn ggml_block_layout(self, shape: &Shape) -> Option<QuantizedBlockLayout> {
        let (elements_per_block, bytes_per_block) = self.ggml_block_spec()?;
        let dims = shape.dims();
        let last_dim = *dims.last()?;
        if last_dim == 0 || last_dim % elements_per_block != 0 {
            return None;
        }
        let element_count = shape.element_count();
        if element_count == 0 || !element_count.is_multiple_of(elements_per_block) {
            return None;
        }
        Some(QuantizedBlockLayout::new(
            elements_per_block,
            bytes_per_block,
            element_count / elements_per_block,
        ))
    }
}

/// Stable floating-point parameter encoded via raw `f32` bit representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StableF32(pub u32);

impl StableF32 {
    /// Creates a stable floating-point parameter from an `f32`.
    #[must_use]
    pub const fn from_f32(value: f32) -> Self {
        Self(value.to_bits())
    }

    /// Decodes the stored value as an `f32`.
    #[must_use]
    pub const fn to_f32(self) -> f32 {
        f32::from_bits(self.0)
    }
}

/// Backend-extension family kept separate from the small visible primitive surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendExtensionKind {
    /// Root-mean-square normalization over the last dimension.
    RmsNorm,
    /// Layer normalization over the last dimension.
    LayerNorm,
    /// Rotary position embedding application.
    RotaryEmbedding,
    /// Scaled dot-product attention over query/key/value tensors.
    ScaledDotProductAttention,
    /// Matmul that is eligible for a quantized-GEMM specialization.
    QuantizedMatmul,
}

impl BackendExtensionKind {
    /// Returns a stable extension label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::RmsNorm => "rms_norm",
            Self::LayerNorm => "layer_norm",
            Self::RotaryEmbedding => "rotary_embedding",
            Self::ScaledDotProductAttention => "scaled_dot_product_attention",
            Self::QuantizedMatmul => "quantized_matmul",
        }
    }
}

/// Typed backend-extension operation carried through graph and plan surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackendExtensionOp {
    /// Root-mean-square normalization over the last dimension.
    RmsNorm {
        /// Epsilon added before square root for numeric stability.
        epsilon: StableF32,
    },
    /// Layer normalization over the last dimension.
    LayerNorm {
        /// Epsilon added before square root for numeric stability.
        epsilon: StableF32,
    },
    /// Rotary position embedding application.
    RotaryEmbedding {
        /// Whether pairs are interleaved on the last dimension.
        interleaved: bool,
    },
    /// Scaled dot-product attention over query/key/value tensors.
    ScaledDotProductAttention {
        /// Multiplicative scale applied to query-key dot products.
        scale: StableF32,
        /// Whether causal masking is applied.
        causal: bool,
    },
    /// Matmul that is eligible for a quantized-GEMM specialization.
    QuantizedMatmul {
        /// Quantized family of the right-hand-side weights.
        rhs_mode: QuantizationMode,
    },
}

impl BackendExtensionOp {
    /// Returns the extension family.
    #[must_use]
    pub const fn kind(&self) -> BackendExtensionKind {
        match self {
            Self::RmsNorm { .. } => BackendExtensionKind::RmsNorm,
            Self::LayerNorm { .. } => BackendExtensionKind::LayerNorm,
            Self::RotaryEmbedding { .. } => BackendExtensionKind::RotaryEmbedding,
            Self::ScaledDotProductAttention { .. } => {
                BackendExtensionKind::ScaledDotProductAttention
            }
            Self::QuantizedMatmul { .. } => BackendExtensionKind::QuantizedMatmul,
        }
    }

    /// Returns a stable extension label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        self.kind().label()
    }
}

/// Stable block layout for GGML/GGUF quantized tensor storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QuantizedBlockLayout {
    /// Logical scalar elements covered by a single quantized block.
    pub elements_per_block: usize,
    /// Serialized byte width of a single quantized block.
    pub bytes_per_block: usize,
    /// Number of quantized blocks in the tensor.
    pub block_count: usize,
}

impl QuantizedBlockLayout {
    /// Creates an explicit block layout.
    #[must_use]
    pub const fn new(
        elements_per_block: usize,
        bytes_per_block: usize,
        block_count: usize,
    ) -> Self {
        Self {
            elements_per_block,
            bytes_per_block,
            block_count,
        }
    }

    /// Returns the logical scalar element count represented by the layout.
    #[must_use]
    pub const fn element_count(self) -> usize {
        self.elements_per_block * self.block_count
    }

    /// Returns the serialized byte length represented by the layout.
    #[must_use]
    pub const fn byte_len(self) -> usize {
        self.bytes_per_block * self.block_count
    }
}

/// Runtime backend family for a device.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeviceKind {
    /// Host CPU execution.
    Cpu,
    /// NVIDIA CUDA execution.
    Cuda,
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
            Self::Cuda => "cuda",
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

    /// Returns the dimension at the provided axis.
    #[must_use]
    pub fn dim(&self, axis: usize) -> Option<usize> {
        self.dims.get(axis).copied()
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

    /// Returns a new shape with axes permuted according to `order`.
    #[must_use]
    pub fn permuted(&self, order: &[usize]) -> Option<Self> {
        if order.len() != self.rank() || !is_permutation(order) {
            return None;
        }
        let dims = order
            .iter()
            .map(|&axis| self.dims[axis])
            .collect::<Vec<_>>();
        Some(Self::new(dims))
    }

    /// Returns a new shape with the given axis removed.
    #[must_use]
    pub fn without_axis(&self, axis: usize) -> Option<Self> {
        if axis >= self.rank() {
            return None;
        }
        let mut dims = self.dims.clone();
        dims.remove(axis);
        Some(Self::new(dims))
    }
}

impl fmt::Display for Shape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.dims)
    }
}

/// Layout metadata for a logical tensor view.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Layout {
    shape: Shape,
    strides: Vec<usize>,
    offset: usize,
}

impl Layout {
    /// Creates a layout from explicit fields.
    #[must_use]
    pub fn new(shape: Shape, strides: Vec<usize>, offset: usize) -> Self {
        Self {
            shape,
            strides,
            offset,
        }
    }

    /// Creates a standard row-major contiguous layout.
    #[must_use]
    pub fn contiguous(shape: Shape) -> Self {
        let mut strides = vec![0; shape.rank()];
        let mut running = 1;
        for (index, dim) in shape.dims().iter().enumerate().rev() {
            strides[index] = running;
            running *= *dim;
        }
        Self::new(shape, strides, 0)
    }

    /// Returns the layout shape.
    #[must_use]
    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    /// Returns the logical strides.
    #[must_use]
    pub fn strides(&self) -> &[usize] {
        &self.strides
    }

    /// Returns the storage offset.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the minimum backing storage length required by the layout.
    #[must_use]
    pub fn storage_size(&self) -> usize {
        if self.shape.rank() == 0 {
            return self.offset + 1;
        }

        let span = self
            .shape
            .dims()
            .iter()
            .zip(self.strides.iter())
            .map(|(&dim, &stride)| dim.saturating_sub(1) * stride)
            .sum::<usize>();
        self.offset + span + 1
    }

    /// Returns whether the layout is row-major contiguous.
    #[must_use]
    pub fn is_contiguous(&self) -> bool {
        *self == Self::contiguous(self.shape.clone())
    }

    /// Returns a permuted layout if `order` is valid.
    #[must_use]
    pub fn permuted(&self, order: &[usize]) -> Option<Self> {
        let shape = self.shape.permuted(order)?;
        let strides = order.iter().map(|&axis| self.strides[axis]).collect();
        Some(Self::new(shape, strides, self.offset))
    }

    /// Returns a sliced layout if the requested bounds are valid.
    #[must_use]
    pub fn sliced(&self, axis: usize, start: usize, end: usize) -> Option<Self> {
        let dim = self.shape.dim(axis)?;
        if start > end || end > dim {
            return None;
        }
        let mut dims = self.shape.dims.clone();
        dims[axis] = end - start;
        let offset = self.offset + (start * self.strides[axis]);
        Some(Self::new(Shape::new(dims), self.strides.clone(), offset))
    }

    /// Returns a selected layout if the requested index is valid.
    #[must_use]
    pub fn selected(&self, axis: usize, index: usize) -> Option<Self> {
        let dim = self.shape.dim(axis)?;
        if index >= dim {
            return None;
        }
        let shape = self.shape.without_axis(axis)?;
        let mut strides = self.strides.clone();
        strides.remove(axis);
        let offset = self.offset + (index * self.strides[axis]);
        Some(Self::new(shape, strides, offset))
    }

    /// Returns an expanded layout if the requested target shape is valid.
    #[must_use]
    pub fn expanded(&self, target_shape: &Shape) -> Option<Self> {
        if target_shape.rank() < self.shape.rank() {
            return None;
        }

        let rank_padding = target_shape.rank() - self.shape.rank();
        let storage_stride = self.storage_size();
        let mut current_dims = vec![1; rank_padding];
        current_dims.extend_from_slice(self.shape.dims());

        let mut strides = vec![storage_stride; rank_padding];
        strides.extend_from_slice(&self.strides);

        for (axis, (&current, &target)) in current_dims.iter().zip(target_shape.dims()).enumerate()
        {
            if current == target {
                continue;
            }
            if current != 1 {
                return None;
            }
            strides[axis] = 0;
        }

        Some(Self::new(target_shape.clone(), strides, self.offset))
    }
}

/// Static tensor metadata.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TensorSpec {
    layout: Layout,
    dtype: DType,
    device: Device,
}

impl TensorSpec {
    /// Creates a new contiguous tensor specification.
    #[must_use]
    pub fn new(shape: Shape, dtype: DType, device: Device) -> Self {
        Self {
            layout: Layout::contiguous(shape),
            dtype,
            device,
        }
    }

    /// Creates a tensor specification from an explicit layout.
    #[must_use]
    pub fn from_layout(layout: Layout, dtype: DType, device: Device) -> Self {
        Self {
            layout,
            dtype,
            device,
        }
    }

    /// Returns the tensor layout.
    #[must_use]
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Returns the tensor shape.
    #[must_use]
    pub fn shape(&self) -> &Shape {
        self.layout.shape()
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

    /// Returns a copy with a different contiguous shape.
    #[must_use]
    pub fn with_shape(&self, shape: Shape) -> Self {
        Self::new(shape, self.dtype, self.device.clone())
    }

    /// Returns a copy with a different layout.
    #[must_use]
    pub fn with_layout(&self, layout: Layout) -> Self {
        Self::from_layout(layout, self.dtype, self.device.clone())
    }

    /// Returns the number of addressable elements.
    #[must_use]
    pub fn element_count(&self) -> usize {
        self.shape().element_count()
    }

    /// Returns the minimum backing storage length required by the tensor
    /// layout.
    #[must_use]
    pub fn storage_size(&self) -> usize {
        self.layout.storage_size()
    }
}

/// Small data container used for constants and host-visible results.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TensorData {
    /// 32-bit floating point tensor payload.
    F32(Vec<f32>),
    /// Quantized GGML/GGUF block payload.
    QuantizedBlocks(QuantizedTensorData),
}

/// Quantized GGML/GGUF block payload kept in graph/runtime constants.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizedTensorData {
    /// Quantization family for the blocks.
    pub mode: QuantizationMode,
    /// Stable GGML block layout for the logical tensor.
    pub layout: QuantizedBlockLayout,
    /// Serialized quantized block bytes.
    pub bytes: Vec<u8>,
}

impl QuantizedTensorData {
    /// Creates a quantized block payload.
    #[must_use]
    pub fn new(
        mode: QuantizationMode,
        layout: QuantizedBlockLayout,
        bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            mode,
            layout,
            bytes: bytes.into(),
        }
    }
}

impl TensorData {
    /// Returns the element count of the payload.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::F32(values) => values.len(),
            Self::QuantizedBlocks(data) => data.layout.element_count(),
        }
    }

    /// Returns whether the payload is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the payload as an `f32` slice when the storage is dense.
    #[must_use]
    pub fn as_f32_slice(&self) -> Option<&[f32]> {
        match self {
            Self::F32(values) => Some(values.as_slice()),
            Self::QuantizedBlocks(_) => None,
        }
    }

    /// Returns the quantized payload when the storage is GGML/GGUF blocks.
    #[must_use]
    pub fn as_quantized_blocks(&self) -> Option<&QuantizedTensorData> {
        match self {
            Self::F32(_) => None,
            Self::QuantizedBlocks(data) => Some(data),
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
    /// Gradient-stopping identity.
    Detach,
    /// Binary add.
    Add,
    /// Binary multiply.
    Mul,
    /// Matrix multiplication.
    Matmul,
    /// Tensor reshape.
    Reshape,
    /// Tensor permute.
    Permute { axes: Vec<usize> },
    /// Tensor slice.
    Slice {
        axis: usize,
        start: usize,
        end: usize,
    },
    /// Tensor select.
    Select { axis: usize, index: usize },
    /// Tensor concat.
    Concat { axis: usize },
    /// Tensor expand/broadcast.
    Expand { shape: Shape },
    /// Full or axis-specific reduction.
    ReduceSum { axis: Option<usize> },
    /// Typed backend-extension operation kept separate from primitive ops.
    BackendExtension { op: BackendExtensionOp },
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

fn is_permutation(order: &[usize]) -> bool {
    let mut seen = vec![false; order.len()];
    for &axis in order {
        if axis >= order.len() || seen[axis] {
            return false;
        }
        seen[axis] = true;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{DType, Device, DeviceKind, Layout, Shape, TensorSpec};

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
        assert!(spec.layout().is_contiguous());
    }

    #[test]
    fn cuda_device_kind_formats_stably() {
        let device = Device::new(DeviceKind::Cuda, 0, None);
        assert_eq!(device.kind(), DeviceKind::Cuda);
        assert_eq!(device.to_string(), "cuda:0");
    }

    #[test]
    fn layout_permute_updates_shape_and_strides() {
        let layout = Layout::contiguous(Shape::new(vec![2, 3, 4]));
        let permuted = layout.permuted(&[1, 0, 2]);
        assert!(permuted.is_some());
        let Some(permuted) = permuted else {
            return;
        };

        assert_eq!(permuted.shape().dims(), &[3, 2, 4]);
        assert_eq!(permuted.strides(), &[4, 12, 1]);
        assert!(!permuted.is_contiguous());
    }

    #[test]
    fn layout_expand_uses_zero_strides() {
        let layout = Layout::contiguous(Shape::new(vec![1, 3]));
        let expanded = layout.expanded(&Shape::new(vec![4, 3]));
        assert!(expanded.is_some());
        let Some(expanded) = expanded else {
            return;
        };

        assert_eq!(expanded.shape().dims(), &[4, 3]);
        assert_eq!(expanded.strides(), &[0, 1]);
    }

    #[test]
    fn layout_expand_can_increase_rank() {
        let layout = Layout::contiguous(Shape::new(vec![2]));
        let expanded = layout.expanded(&Shape::new(vec![3, 2]));
        assert!(expanded.is_some());
        let Some(expanded) = expanded else {
            return;
        };

        assert_eq!(expanded.shape().dims(), &[3, 2]);
        assert_eq!(expanded.strides(), &[0, 1]);
    }

    #[test]
    fn expanded_layout_storage_size_matches_source_span() {
        let expanded = Layout::contiguous(Shape::new(vec![1, 3])).expanded(&Shape::new(vec![4, 3]));
        assert!(expanded.is_some());
        let Some(expanded) = expanded else {
            return;
        };
        let spec = TensorSpec::from_layout(expanded, DType::F32, Device::cpu());

        assert_eq!(spec.element_count(), 12);
        assert_eq!(spec.storage_size(), 3);
    }
}
