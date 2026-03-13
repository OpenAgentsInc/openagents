//! CUDA backend discovery, allocation, submission, and dense execution surface
//! for Psionic.

#![allow(
    clippy::manual_is_multiple_of,
    clippy::result_large_err,
    clippy::too_many_arguments
)]
#![cfg_attr(
    test,
    allow(
        clippy::bool_to_int_with_if,
        clippy::expect_used,
        clippy::manual_slice_size_calculation,
        clippy::panic,
        clippy::panic_in_result_fn
    )
)]

use std::{collections::BTreeMap, fmt, io::ErrorKind, process::Command};

use psionic_compiler::compile_graph;
use psionic_core::{
    DType, Device, DeviceKind, QuantizationMode, Shape, TensorData, TensorId, TensorSpec,
};
use psionic_ir::{ExecutionOp, ExecutionPlan, ExecutionStep, Graph};
use psionic_runtime::{
    Allocator, AllocatorPoolPolicy, AllocatorPoolReport, AllocatorPoolState, BackendDegradedPolicy,
    BackendName, BackendRuntimeResources, BackendSelection, BufferHandle, CacheAction, CacheKind,
    CacheObservation, CompilePathEvidence, CompilePathTemperature, DeviceDescriptor,
    DeviceDiscovery, DeviceMemoryBudget, ExecutionBackend, ExecutionMetrics,
    ExecutionPlanCachePolicy, ExecutionPlanCacheReport, ExecutionPlanCacheState, ExecutionResult,
    HealthStatus, KernelCachePolicy, KernelCacheReport, KernelCacheState, NvidiaBackendReport,
    NvidiaDeviceMetadata, NvidiaRecoveryAction, NvidiaRecoveryProfile, NvidiaRiskLevel,
    NvidiaRiskProfile, NvidiaTopologyInfo, RuntimeError, RuntimeHealth, ServedProductBackendPolicy,
};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "CUDA backend discovery, allocation, and submission";

const NVIDIA_SMI_BINARY: &str = "nvidia-smi";
const INVENTORY_QUERY: &str = concat!(
    "index,name,pci.bus_id,memory.total,compute_cap,display_attached,",
    "mig.mode.current,persistence_mode,addressing_mode"
);
const OFFLINE_NO_DRIVER_MESSAGE: &str =
    "cuda backend unavailable: nvidia-smi is not installed or the NVIDIA driver is not reachable";
const CUDA_POOL_MAX_CACHED_BUFFERS: usize = 128;
const CUDA_POOL_MAX_CACHED_BYTES: u64 = 64 * 1024 * 1024;
const CUDA_EXECUTION_PLAN_CACHE_MAX_ENTRIES: usize = 64;
const CUDA_EXECUTION_PLAN_CACHE_MAX_CACHED_BYTES: u64 = 1024 * 1024;

/// Exact plan surface currently covered by the first CUDA-backed served-product
/// milestone.
pub const SUPPORTED_OPS: &[&str] = &["input", "constant", "matmul", "add"];

/// Dense op surface currently covered for the first CUDA-backed embeddings
/// product path.
pub const EMBEDDINGS_SUPPORTED_OPS: &[&str] = SUPPORTED_OPS;

/// CUDA-side quantized primitive surface required by the GPT-OSS text-generation
/// path.
pub const TEXT_GENERATION_SUPPORTED_OPS: &[&str] =
    &["quantized_matvec_q8_0", "quantized_matvec_mxfp4"];
pub const GGML_Q8_1_BLOCK_ELEMENTS: usize = 32;
pub const GGML_Q8_1_BLOCK_BYTES: usize = 36;

/// Returns the device scratch-buffer size for contiguous GGML `Q8_1` rows.
pub fn ggml_q8_1_storage_bytes(rows: usize, cols: usize) -> Result<usize, RuntimeError> {
    if cols == 0 || cols % GGML_Q8_1_BLOCK_ELEMENTS != 0 {
        return Err(RuntimeError::Backend(format!(
            "ggml q8_1 scratch requires block-aligned width {cols}",
        )));
    }
    Ok(rows
        .saturating_mul(cols / GGML_Q8_1_BLOCK_ELEMENTS)
        .saturating_mul(GGML_Q8_1_BLOCK_BYTES))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NvidiaInventoryRow {
    ordinal: u16,
    name: String,
    pci_bdf: Option<String>,
    memory_bytes: Option<u64>,
    compute_capability: Option<String>,
    display_attached: Option<bool>,
    mig_profile: Option<String>,
    persistence_mode_enabled: Option<bool>,
    addressing_mode: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NvidiaQueryErrorKind {
    NotInstalled,
    Failed,
    InvalidUtf8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NvidiaQueryError {
    kind: NvidiaQueryErrorKind,
    message: String,
}

/// CUDA-visible backing memory class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CudaMemorySpace {
    /// Ordinary device-only memory allocated by the CUDA runtime.
    Device,
}

/// How long Psionic should wait after a CUDA submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CudaCommandWait {
    /// Wait until the CUDA stream completes.
    Completed,
}

/// Stable submission lifecycle state exposed by Psionic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CudaCommandStatus {
    /// The submission was recorded onto a CUDA stream.
    Submitted,
    /// The CUDA stream completed successfully.
    Completed,
    /// The CUDA stream failed.
    Error,
}

/// Submission metadata returned after a CUDA stream is synchronized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CudaSubmissionReport {
    /// Final stream status observed by Psionic.
    pub status: CudaCommandStatus,
    /// Number of explicit operations recorded in the submission.
    pub encoded_operations: usize,
}

/// Reusable captured CUDA graph executable for one fixed submission shape.
pub struct CudaGraphExec {
    encoded_operations: usize,
    platform: platform::PlatformGraphExec,
}

impl fmt::Debug for CudaGraphExec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CudaGraphExec")
            .field("encoded_operations", &self.encoded_operations)
            .field("platform", &"<cuda graph exec>")
            .finish()
    }
}

impl CudaGraphExec {
    /// Launches the captured CUDA graph and optionally waits for completion.
    pub fn launch(&self, wait: CudaCommandWait) -> Result<CudaSubmissionReport, RuntimeError> {
        let status = self.platform.launch(wait)?;
        Ok(CudaSubmissionReport {
            status,
            encoded_operations: self.encoded_operations,
        })
    }
}

/// Explicit per-call counters for one quantized CUDA matvec request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CudaQuantizedMatvecStats {
    /// Bytes uploaded from host to device for the activation input.
    pub host_to_device_bytes: u64,
    /// Bytes read back from device to host for the output vector.
    pub device_to_host_bytes: u64,
    /// Number of CUDA submissions used by the call.
    pub submission_count: usize,
    /// Number of CUDA stream synchronizations used by the call.
    pub sync_count: usize,
    /// Number of quantized CUDA kernels launched by the call.
    pub kernel_launches: usize,
}

impl CudaQuantizedMatvecStats {
    /// Accumulates another stats snapshot into this one.
    pub fn accumulate(&mut self, other: &Self) {
        self.host_to_device_bytes = self
            .host_to_device_bytes
            .saturating_add(other.host_to_device_bytes);
        self.device_to_host_bytes = self
            .device_to_host_bytes
            .saturating_add(other.device_to_host_bytes);
        self.submission_count = self.submission_count.saturating_add(other.submission_count);
        self.sync_count = self.sync_count.saturating_add(other.sync_count);
        self.kernel_launches = self.kernel_launches.saturating_add(other.kernel_launches);
    }
}

/// Result payload for one profiled quantized CUDA matvec call.
#[derive(Clone, Debug, PartialEq)]
pub struct CudaQuantizedMatvecResult {
    /// Returned dense output values.
    pub values: Vec<f32>,
    /// Explicit transfer and submission counters for the call.
    pub stats: CudaQuantizedMatvecStats,
}

/// CUDA-backed tensor buffer.
#[derive(Clone)]
pub struct CudaBuffer {
    spec: TensorSpec,
    byte_len: usize,
    memory_space: CudaMemorySpace,
    host_visible: bool,
    platform: platform::PlatformBuffer,
}

impl fmt::Debug for CudaBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CudaBuffer")
            .field("spec", &self.spec)
            .field("byte_len", &self.byte_len)
            .field("memory_space", &self.memory_space)
            .field("host_visible", &self.host_visible)
            .field("platform", &"<cuda platform buffer>")
            .finish()
    }
}

impl CudaBuffer {
    /// Returns the backing allocation size in bytes.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// Returns the CUDA memory space backing the buffer.
    #[must_use]
    pub const fn memory_space(&self) -> CudaMemorySpace {
        self.memory_space
    }

    /// Returns whether the CPU can directly map the backing storage.
    #[must_use]
    pub const fn host_visible(&self) -> bool {
        self.host_visible
    }

    /// Returns a stable identity for the current device allocation.
    #[must_use]
    pub fn allocation_identity(&self) -> usize {
        self.platform.allocation_identity()
    }

    /// Writes raw bytes into the CUDA buffer via an explicit host-to-device transfer.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        if bytes.len() != self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer write length mismatch: expected {}, actual {}",
                self.byte_len,
                bytes.len()
            )));
        }
        self.platform.write_bytes(bytes)
    }

    /// Writes raw bytes into a region of the CUDA buffer via an explicit host-to-device transfer.
    pub fn write_bytes_at_offset(
        &mut self,
        byte_offset: usize,
        bytes: &[u8],
    ) -> Result<(), RuntimeError> {
        if byte_offset.saturating_add(bytes.len()) > self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer region write exceeds allocation: offset={} len={} allocation={}",
                byte_offset,
                bytes.len(),
                self.byte_len
            )));
        }
        self.platform.write_bytes_at_offset(byte_offset, bytes)
    }

    /// Reads raw bytes from the CUDA buffer via an explicit device-to-host transfer.
    pub fn read_bytes(&self) -> Result<Vec<u8>, RuntimeError> {
        self.platform.read_bytes(self.byte_len)
    }

    /// Reads raw bytes from a region of the CUDA buffer via an explicit device-to-host transfer.
    pub fn read_bytes_at_offset(
        &self,
        byte_offset: usize,
        byte_len: usize,
    ) -> Result<Vec<u8>, RuntimeError> {
        if byte_offset.saturating_add(byte_len) > self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer region read exceeds allocation: offset={} len={} allocation={}",
                byte_offset, byte_len, self.byte_len
            )));
        }
        self.platform.read_bytes_at_offset(byte_offset, byte_len)
    }

    /// Writes contiguous `f32` values into an `f32` buffer.
    pub fn write_f32(&mut self, values: &[f32]) -> Result<(), RuntimeError> {
        if self.spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "write_f32 requires F32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        if values.len() != self.spec.storage_size() {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer write length mismatch: expected {} values, actual {}",
                self.spec.storage_size(),
                values.len()
            )));
        }
        let mut bytes = Vec::with_capacity(self.byte_len);
        for value in values {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        self.write_bytes(&bytes)
    }

    /// Writes contiguous `f32` values into a region of an `f32` buffer.
    pub fn write_f32_at_offset(
        &mut self,
        element_offset: usize,
        values: &[f32],
    ) -> Result<(), RuntimeError> {
        if self.spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "write_f32_at_offset requires F32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        let expected_end = element_offset.saturating_add(values.len());
        if expected_end > self.spec.storage_size() {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer f32 region write exceeds allocation: end={} allocation={}",
                expected_end,
                self.spec.storage_size()
            )));
        }
        let mut bytes = Vec::with_capacity(values.len().saturating_mul(size_of_dtype(DType::F32)));
        for value in values {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        self.write_bytes_at_offset(
            element_offset.saturating_mul(size_of_dtype(DType::F32)),
            bytes.as_slice(),
        )
    }

    /// Reads contiguous `f32` values from an `f32` buffer.
    pub fn read_f32(&self) -> Result<Vec<f32>, RuntimeError> {
        if self.spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "read_f32 requires F32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        let bytes = self.read_bytes()?;
        let mut values = Vec::with_capacity(bytes.len() / size_of_dtype(self.spec.dtype()));
        for chunk in bytes.chunks_exact(size_of_dtype(self.spec.dtype())) {
            values.push(f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        Ok(values)
    }

    /// Reads contiguous `f32` values from a region of an `f32` buffer.
    pub fn read_f32_at_offset(
        &self,
        element_offset: usize,
        element_count: usize,
    ) -> Result<Vec<f32>, RuntimeError> {
        if self.spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "read_f32_at_offset requires F32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        let expected_end = element_offset.saturating_add(element_count);
        if expected_end > self.spec.storage_size() {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer f32 region read exceeds allocation: end={} allocation={}",
                expected_end,
                self.spec.storage_size()
            )));
        }
        let bytes = self.read_bytes_at_offset(
            element_offset.saturating_mul(size_of_dtype(DType::F32)),
            element_count.saturating_mul(size_of_dtype(DType::F32)),
        )?;
        let mut values = Vec::with_capacity(element_count);
        for chunk in bytes.chunks_exact(size_of_dtype(DType::F32)) {
            values.push(f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }
        Ok(values)
    }
}

/// Page-locked host buffer for stream-owned CUDA staging transfers.
#[derive(Clone)]
pub struct CudaHostBuffer {
    byte_len: usize,
    platform: platform::PlatformHostBuffer,
}

impl fmt::Debug for CudaHostBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CudaHostBuffer")
            .field("byte_len", &self.byte_len)
            .field("platform", &"<cuda pinned host buffer>")
            .finish()
    }
}

impl CudaHostBuffer {
    /// Returns the allocation size in bytes.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// Writes raw bytes into the pinned host buffer.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        if bytes.len() != self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda host buffer write length mismatch: expected {}, actual {}",
                self.byte_len,
                bytes.len()
            )));
        }
        self.platform.write_bytes(bytes)
    }

    /// Reads raw bytes from the pinned host buffer.
    pub fn read_bytes(&self) -> Result<Vec<u8>, RuntimeError> {
        self.platform.read_bytes(self.byte_len)
    }

    /// Writes contiguous `f32` values into the pinned host buffer.
    pub fn write_f32(&mut self, values: &[f32]) -> Result<(), RuntimeError> {
        if values.len().saturating_mul(size_of_dtype(DType::F32)) != self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda host buffer f32 write length mismatch: expected {} bytes, actual {}",
                self.byte_len,
                values.len().saturating_mul(size_of_dtype(DType::F32))
            )));
        }
        let mut bytes = Vec::with_capacity(self.byte_len);
        for value in values {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        self.write_bytes(bytes.as_slice())
    }

    /// Writes contiguous `i32` values into the pinned host buffer.
    pub fn write_i32(&mut self, values: &[i32]) -> Result<(), RuntimeError> {
        if values.len().saturating_mul(size_of::<i32>()) != self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda host buffer i32 write length mismatch: expected {} bytes, actual {}",
                self.byte_len,
                values.len().saturating_mul(size_of::<i32>())
            )));
        }
        let mut bytes = Vec::with_capacity(self.byte_len);
        for value in values {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        self.write_bytes(bytes.as_slice())
    }

    /// Reads one `i32` from the pinned host buffer.
    pub fn read_i32(&self) -> Result<i32, RuntimeError> {
        if self.byte_len != size_of::<i32>() {
            return Err(RuntimeError::Backend(format!(
                "cuda host buffer i32 read requires {} bytes, actual {}",
                size_of::<i32>(),
                self.byte_len
            )));
        }
        let bytes = self.read_bytes()?;
        Ok(i32::from_ne_bytes(
            bytes[..size_of::<i32>()].try_into().map_err(|_| {
                RuntimeError::Backend(String::from("cuda host buffer returned invalid i32 bytes"))
            })?,
        ))
    }
}

impl BufferHandle for CudaBuffer {
    fn spec(&self) -> &TensorSpec {
        &self.spec
    }
}

/// CUDA stream submission that keeps fill/copy operations explicit.
pub struct CudaSubmission {
    encoded_operations: usize,
    capturing: bool,
    platform: platform::PlatformSubmission,
}

impl CudaSubmission {
    /// Fills a buffer with a constant byte value using an async CUDA memset.
    pub fn fill_buffer(&mut self, buffer: &CudaBuffer, value: u8) -> Result<(), RuntimeError> {
        self.platform
            .fill_buffer(&buffer.platform, buffer.byte_len, value)?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Copies one CUDA buffer into another with explicit size checking.
    pub fn copy_buffer(
        &mut self,
        source: &CudaBuffer,
        destination: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        if source.byte_len != destination.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda buffer copy length mismatch: source {}, destination {}",
                source.byte_len, destination.byte_len
            )));
        }
        self.platform
            .copy_buffer(&source.platform, &destination.platform, source.byte_len)?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Copies a pinned host staging buffer into device memory on this stream.
    pub fn copy_host_to_device(
        &mut self,
        source: &CudaHostBuffer,
        destination: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        if source.byte_len != destination.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda host-to-device copy length mismatch: source {}, destination {}",
                source.byte_len, destination.byte_len
            )));
        }
        self.platform.copy_host_to_device(
            &source.platform,
            &destination.platform,
            destination.byte_len,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Copies device memory back into a pinned host staging buffer on this stream.
    pub fn copy_device_to_host(
        &mut self,
        source: &CudaBuffer,
        destination: &CudaHostBuffer,
    ) -> Result<(), RuntimeError> {
        if source.byte_len != destination.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda device-to-host copy length mismatch: source {}, destination {}",
                source.byte_len, destination.byte_len
            )));
        }
        self.platform.copy_device_to_host(
            &source.platform,
            &destination.platform,
            source.byte_len,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Copies a byte range between CUDA buffers.
    pub fn copy_buffer_region(
        &mut self,
        source: &CudaBuffer,
        source_byte_offset: usize,
        destination: &CudaBuffer,
        destination_byte_offset: usize,
        byte_len: usize,
    ) -> Result<(), RuntimeError> {
        if source_byte_offset.saturating_add(byte_len) > source.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda source region copy exceeds allocation: offset={} len={} allocation={}",
                source_byte_offset, byte_len, source.byte_len
            )));
        }
        if destination_byte_offset.saturating_add(byte_len) > destination.byte_len {
            return Err(RuntimeError::Backend(format!(
                "cuda destination region copy exceeds allocation: offset={} len={} allocation={}",
                destination_byte_offset, byte_len, destination.byte_len
            )));
        }
        self.platform.copy_buffer_region(
            &source.platform,
            source_byte_offset,
            &destination.platform,
            destination_byte_offset,
            byte_len,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one quantized row-wise matrix-vector product.
    pub fn quantized_matvec(
        &mut self,
        weights: &CudaBuffer,
        byte_offset: usize,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input: &CudaBuffer,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_quantized_matvec(
            &weights.platform,
            byte_offset,
            mode,
            rows,
            cols,
            &input.platform,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Quantizes contiguous `f32` rows into GGML `Q8_1` scratch storage.
    pub fn quantize_f32_to_q8_1(
        &mut self,
        input: &CudaBuffer,
        rows: usize,
        cols: usize,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let required_bytes = ggml_q8_1_storage_bytes(rows, cols)?;
        if output.byte_len() < required_bytes {
            return Err(RuntimeError::Backend(format!(
                "cuda q8_1 scratch buffer too small: need {required_bytes} bytes for {rows}x{cols}, have {}",
                output.byte_len()
            )));
        }
        self.platform
            .encode_quantize_f32_to_q8_1(&input.platform, rows, cols, &output.platform)?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Dequantizes one GGML row directly into a device `f32` vector.
    pub fn dequantize_row_to_f32(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        rows: usize,
        row_stride: usize,
        cols: usize,
        decode_params: &CudaBuffer,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_dequantize_row_to_f32(
            &weights.platform,
            mode,
            rows,
            row_stride,
            cols,
            &decode_params.platform,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Gathers one dense row-major `f16` row directly into a device `f32`
    /// vector using the token id stored in the decode params buffer.
    pub fn gather_f16_row_to_f32(
        &mut self,
        input: &CudaBuffer,
        rows: usize,
        cols: usize,
        decode_params: &CudaBuffer,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_gather_f16_row_to_f32(
            &input.platform,
            rows,
            cols,
            &decode_params.platform,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one quantized row-wise matrix-vector product using a device
    /// `Q8_1` activation buffer.
    pub fn quantized_matvec_q8_1(
        &mut self,
        weights: &CudaBuffer,
        byte_offset: usize,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input_q8_1: &CudaBuffer,
        bias: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_quantized_matvec_q8_1(
            &weights.platform,
            byte_offset,
            mode,
            rows,
            cols,
            &input_q8_1.platform,
            bias.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one quantized row-wise matrix-vector product using a device
    /// `Q8_1` activation buffer and writes the greedy argmax directly.
    pub fn quantized_matvec_q8_1_argmax(
        &mut self,
        weights: &CudaBuffer,
        byte_offset: usize,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input_q8_1: &CudaBuffer,
        bias: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_quantized_matvec_q8_1_argmax(
            &weights.platform,
            byte_offset,
            mode,
            rows,
            cols,
            &input_q8_1.platform,
            bias.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one dense row-major matrix multiply using cuBLAS.
    pub fn matmul(
        &mut self,
        left: &CudaBuffer,
        right: &CudaBuffer,
        output: &CudaBuffer,
        rows: usize,
        inner: usize,
        cols: usize,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_matmul(
            &left.platform,
            &right.platform,
            &output.platform,
            rows,
            inner,
            cols,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one dense row-major matrix multiply with an `f16` rhs and
    /// `f32` accumulate/output.
    pub fn matmul_f16_to_f32(
        &mut self,
        left: &CudaBuffer,
        right: &CudaBuffer,
        output: &CudaBuffer,
        rows: usize,
        inner: usize,
        cols: usize,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_matmul_f16_to_f32(
            &left.platform,
            &right.platform,
            &output.platform,
            rows,
            inner,
            cols,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Casts contiguous `f32` values to contiguous `f16` values on CUDA.
    pub fn cast_f32_to_f16(
        &mut self,
        input: &CudaBuffer,
        output: &CudaBuffer,
        element_count: usize,
    ) -> Result<(), RuntimeError> {
        self.platform
            .encode_cast_f32_to_f16(&input.platform, &output.platform, element_count)?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Reduces each contiguous `f32` row to its argmax index on CUDA.
    pub fn argmax_f32(
        &mut self,
        input: &CudaBuffer,
        row_count: usize,
        column_count: usize,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_argmax_f32(
            &input.platform,
            row_count,
            column_count,
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Launches one RMSNorm kernel.
    pub fn rms_norm(
        &mut self,
        input: &CudaBuffer,
        weight: &CudaBuffer,
        output: &CudaBuffer,
        element_count: usize,
        epsilon: f32,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_rms_norm(
            &input.platform,
            &weight.platform,
            &output.platform,
            element_count,
            epsilon,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies RMSNorm and quantizes the normalized output into GGML `Q8_1`.
    pub fn rms_norm_q8_1(
        &mut self,
        input: &CudaBuffer,
        weight: &CudaBuffer,
        output_q8_1: &CudaBuffer,
        element_count: usize,
        epsilon: f32,
    ) -> Result<(), RuntimeError> {
        let required_bytes = ggml_q8_1_storage_bytes(1, element_count)?;
        if output_q8_1.byte_len() < required_bytes {
            return Err(RuntimeError::Backend(format!(
                "cuda q8_1 norm buffer too small: need {required_bytes} bytes for 1x{element_count}, have {}",
                output_q8_1.byte_len()
            )));
        }
        self.platform.encode_rms_norm_q8_1(
            &input.platform,
            &weight.platform,
            &output_q8_1.platform,
            element_count,
            epsilon,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Adds a residual vector and applies RMSNorm in one CUDA kernel.
    pub fn add_residual_rms_norm(
        &mut self,
        input: &CudaBuffer,
        residual: &CudaBuffer,
        input_bias: Option<&CudaBuffer>,
        weight: &CudaBuffer,
        summed_output: &CudaBuffer,
        normalized_output: &CudaBuffer,
        element_count: usize,
        epsilon: f32,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_add_residual_rms_norm(
            &input.platform,
            &residual.platform,
            input_bias.map(|buffer| &buffer.platform),
            &weight.platform,
            &summed_output.platform,
            &normalized_output.platform,
            element_count,
            epsilon,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Adds a residual vector, applies RMSNorm, and quantizes the normalized
    /// output into GGML `Q8_1` in one CUDA kernel.
    pub fn add_residual_rms_norm_q8_1(
        &mut self,
        input: &CudaBuffer,
        residual: &CudaBuffer,
        input_bias: Option<&CudaBuffer>,
        weight: &CudaBuffer,
        summed_output: &CudaBuffer,
        normalized_output: &CudaBuffer,
        quantized_output: &CudaBuffer,
        element_count: usize,
        epsilon: f32,
    ) -> Result<(), RuntimeError> {
        let required_bytes = ggml_q8_1_storage_bytes(1, element_count)?;
        if quantized_output.byte_len() < required_bytes {
            return Err(RuntimeError::Backend(format!(
                "cuda q8_1 norm buffer too small: need {required_bytes} bytes for 1x{element_count}, have {}",
                quantized_output.byte_len()
            )));
        }
        self.platform.encode_add_residual_rms_norm_q8_1(
            &input.platform,
            &residual.platform,
            input_bias.map(|buffer| &buffer.platform),
            &weight.platform,
            &summed_output.platform,
            &normalized_output.platform,
            &quantized_output.platform,
            element_count,
            epsilon,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Adds a residual vector, applies RMSNorm, quantizes the normalized
    /// output into GGML `Q8_1`, and computes router top-k on the normalized
    /// vector in one CUDA kernel.
    #[allow(clippy::too_many_arguments)]
    pub fn add_residual_rms_norm_q8_1_router_topk(
        &mut self,
        input: &CudaBuffer,
        residual: &CudaBuffer,
        input_bias: Option<&CudaBuffer>,
        weight: &CudaBuffer,
        summed_output: &CudaBuffer,
        normalized_output: &CudaBuffer,
        quantized_output: &CudaBuffer,
        router_weights: &CudaBuffer,
        router_bias: Option<&CudaBuffer>,
        expert_count: usize,
        top_k: usize,
        selected_ids: &CudaBuffer,
        selected_weights: &CudaBuffer,
        element_count: usize,
        epsilon: f32,
    ) -> Result<(), RuntimeError> {
        let required_bytes = ggml_q8_1_storage_bytes(1, element_count)?;
        if quantized_output.byte_len() < required_bytes {
            return Err(RuntimeError::Backend(format!(
                "cuda q8_1 norm buffer too small: need {required_bytes} bytes for 1x{element_count}, have {}",
                quantized_output.byte_len()
            )));
        }
        self.platform
            .encode_add_residual_rms_norm_q8_1_router_topk(
                &input.platform,
                &residual.platform,
                input_bias.map(|buffer| &buffer.platform),
                &weight.platform,
                &summed_output.platform,
                &normalized_output.platform,
                &quantized_output.platform,
                &router_weights.platform,
                router_bias.map(|buffer| &buffer.platform),
                expert_count,
                top_k,
                &selected_ids.platform,
                &selected_weights.platform,
                element_count,
                epsilon,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Adds one dense vector into a region of a destination buffer in place.
    pub fn add_f32_in_place(
        &mut self,
        destination: &CudaBuffer,
        element_offset: usize,
        rhs: &CudaBuffer,
        element_count: usize,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_add_f32_in_place(
            &destination.platform,
            element_offset,
            &rhs.platform,
            element_count,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies GPT-OSS NEOX-style RoPE in place to one buffer region.
    pub fn rope_neox_in_place(
        &mut self,
        values: &CudaBuffer,
        element_offset: usize,
        head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        position: usize,
        freq_scale: f32,
        ext_factor: f32,
        corr_dims: [f32; 2],
        theta_scale: f32,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_rope_neox_in_place(
            &values.platform,
            element_offset,
            head_count,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Applies GPT-OSS NEOX-style RoPE, writes the current KV entry, and runs
    /// decode attention in one CUDA kernel.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_decode_rope_cache(
        &mut self,
        qkv: &CudaBuffer,
        query_offset: usize,
        key_offset: usize,
        value_offset: usize,
        cache_keys: &CudaBuffer,
        cache_values: &CudaBuffer,
        cache_width: usize,
        layer_offset: usize,
        past_tokens: usize,
        sliding_window: usize,
        head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        position: usize,
        freq_scale: f32,
        ext_factor: f32,
        corr_dims: [f32; 2],
        theta_scale: f32,
        attention_sinks: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_attention_decode_rope_cache(
            &qkv.platform,
            query_offset,
            key_offset,
            value_offset,
            &cache_keys.platform,
            &cache_values.platform,
            cache_width,
            layer_offset,
            past_tokens,
            sliding_window,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            attention_sinks.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Variant of fused GPT-OSS decode attention that keeps the device KV
    /// mirror in `f16`, closer to the llama.cpp CUDA path.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_decode_rope_cache_f16_kv(
        &mut self,
        qkv: &CudaBuffer,
        query_offset: usize,
        key_offset: usize,
        value_offset: usize,
        cache_keys: &CudaBuffer,
        cache_values: &CudaBuffer,
        cache_width: usize,
        layer_offset: usize,
        past_tokens: usize,
        sliding_window: usize,
        head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        position: usize,
        freq_scale: f32,
        ext_factor: f32,
        corr_dims: [f32; 2],
        theta_scale: f32,
        attention_sinks: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_attention_decode_rope_cache_f16_kv(
            &qkv.platform,
            query_offset,
            key_offset,
            value_offset,
            &cache_keys.platform,
            &cache_values.platform,
            cache_width,
            layer_offset,
            past_tokens,
            sliding_window,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            position,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            attention_sinks.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Variant of fused GPT-OSS decode attention that writes the attention
    /// output directly into contiguous GGML `Q8_1` blocks.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_decode_rope_cache_f16_kv_q8_1(
        &mut self,
        qkv: &CudaBuffer,
        query_offset: usize,
        key_offset: usize,
        value_offset: usize,
        cache_keys: &CudaBuffer,
        cache_values: &CudaBuffer,
        cache_width: usize,
        layer_offset: usize,
        past_tokens: usize,
        sliding_window: usize,
        head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        position: usize,
        freq_scale: f32,
        ext_factor: f32,
        corr_dims: [f32; 2],
        theta_scale: f32,
        attention_sinks: Option<&CudaBuffer>,
        output_q8_1: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let element_count = head_count.checked_mul(head_dim).ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda q8_1 attention output element count overflow",
            ))
        })?;
        let required_bytes = ggml_q8_1_storage_bytes(1, element_count)?;
        if output_q8_1.byte_len() < required_bytes {
            return Err(RuntimeError::Backend(format!(
                "cuda q8_1 attention output buffer too small: need {required_bytes} bytes for 1x{element_count}, have {}",
                output_q8_1.byte_len()
            )));
        }
        self.platform
            .encode_attention_decode_rope_cache_f16_kv_q8_1(
                &qkv.platform,
                query_offset,
                key_offset,
                value_offset,
                &cache_keys.platform,
                &cache_values.platform,
                cache_width,
                layer_offset,
                past_tokens,
                sliding_window,
                head_count,
                kv_head_count,
                head_dim,
                rotary_dim,
                position,
                freq_scale,
                ext_factor,
                corr_dims,
                theta_scale,
                attention_sinks.map(|buffer| &buffer.platform),
                &output_q8_1.platform,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Graph-capture-friendly variant of fused GPT-OSS decode attention that
    /// reads the dynamic `past_tokens` and `position` values from device memory.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_decode_rope_cache_f16_kv_graph(
        &mut self,
        qkv: &CudaBuffer,
        query_offset: usize,
        key_offset: usize,
        value_offset: usize,
        cache_keys: &CudaBuffer,
        cache_values: &CudaBuffer,
        cache_width: usize,
        layer_offset: usize,
        decode_params: &CudaBuffer,
        sliding_window: usize,
        head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        freq_scale: f32,
        ext_factor: f32,
        corr_dims: [f32; 2],
        theta_scale: f32,
        attention_sinks: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform
            .encode_attention_decode_rope_cache_f16_kv_graph(
                &qkv.platform,
                query_offset,
                key_offset,
                value_offset,
                &cache_keys.platform,
                &cache_values.platform,
                cache_width,
                layer_offset,
                &decode_params.platform,
                sliding_window,
                head_count,
                kv_head_count,
                head_dim,
                rotary_dim,
                freq_scale,
                ext_factor,
                corr_dims,
                theta_scale,
                attention_sinks.map(|buffer| &buffer.platform),
                &output.platform,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Graph-capture-friendly variant of fused GPT-OSS decode attention that
    /// writes the attention output directly into contiguous GGML `Q8_1` blocks.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_decode_rope_cache_f16_kv_graph_q8_1(
        &mut self,
        qkv: &CudaBuffer,
        query_offset: usize,
        key_offset: usize,
        value_offset: usize,
        cache_keys: &CudaBuffer,
        cache_values: &CudaBuffer,
        cache_width: usize,
        layer_offset: usize,
        decode_params: &CudaBuffer,
        sliding_window: usize,
        head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        rotary_dim: usize,
        freq_scale: f32,
        ext_factor: f32,
        corr_dims: [f32; 2],
        theta_scale: f32,
        attention_sinks: Option<&CudaBuffer>,
        output_q8_1: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let element_count = head_count.checked_mul(head_dim).ok_or_else(|| {
            RuntimeError::Backend(String::from(
                "cuda q8_1 attention output element count overflow",
            ))
        })?;
        let required_bytes = ggml_q8_1_storage_bytes(1, element_count)?;
        if output_q8_1.byte_len() < required_bytes {
            return Err(RuntimeError::Backend(format!(
                "cuda q8_1 attention output buffer too small: need {required_bytes} bytes for 1x{element_count}, have {}",
                output_q8_1.byte_len()
            )));
        }
        self.platform
            .encode_attention_decode_rope_cache_f16_kv_graph_q8_1(
                &qkv.platform,
                query_offset,
                key_offset,
                value_offset,
                &cache_keys.platform,
                &cache_values.platform,
                cache_width,
                layer_offset,
                &decode_params.platform,
                sliding_window,
                head_count,
                kv_head_count,
                head_dim,
                rotary_dim,
                freq_scale,
                ext_factor,
                corr_dims,
                theta_scale,
                attention_sinks.map(|buffer| &buffer.platform),
                &output_q8_1.platform,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes single-token GPT-OSS attention against a device-resident KV cache.
    #[allow(clippy::too_many_arguments)]
    pub fn attention_decode(
        &mut self,
        query: &CudaBuffer,
        query_offset: usize,
        current_key: &CudaBuffer,
        key_offset: usize,
        current_value: &CudaBuffer,
        value_offset: usize,
        cache_keys: &CudaBuffer,
        cache_values: &CudaBuffer,
        cache_width: usize,
        layer_offset: usize,
        past_tokens: usize,
        sliding_window: usize,
        head_count: usize,
        kv_head_count: usize,
        head_dim: usize,
        attention_sinks: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_attention_decode(
            &query.platform,
            query_offset,
            &current_key.platform,
            key_offset,
            &current_value.platform,
            value_offset,
            &cache_keys.platform,
            &cache_values.platform,
            cache_width,
            layer_offset,
            past_tokens,
            sliding_window,
            head_count,
            kv_head_count,
            head_dim,
            attention_sinks.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Computes router logits on device, selects the top experts, and writes
    /// normalized routing weights for the selected set.
    pub fn router_topk_softmax(
        &mut self,
        weights: &CudaBuffer,
        bias: Option<&CudaBuffer>,
        input: &CudaBuffer,
        expert_count: usize,
        input_size: usize,
        top_k: usize,
        selected_ids: &CudaBuffer,
        selected_weights: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_router_topk_softmax(
            &weights.platform,
            bias.map(|buffer| &buffer.platform),
            &input.platform,
            expert_count,
            input_size,
            top_k,
            &selected_ids.platform,
            &selected_weights.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Selects the top experts from precomputed router logits and applies
    /// delayed softmax normalization across only the selected set.
    pub fn router_topk_delayed_softmax(
        &mut self,
        logits: &CudaBuffer,
        expert_count: usize,
        top_k: usize,
        selected_ids: &CudaBuffer,
        selected_weights: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_router_topk_delayed_softmax(
            &logits.platform,
            expert_count,
            top_k,
            &selected_ids.platform,
            &selected_weights.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes the packed GPT-OSS gate/up expert projection on device and
    /// writes the post-SwiGLU activations for the selected experts.
    #[allow(clippy::too_many_arguments)]
    pub fn moe_gate_up_swiglu(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        columns: usize,
        gate_rows: usize,
        up_rows: usize,
        selected_ids: &CudaBuffer,
        selected_count: usize,
        input: &CudaBuffer,
        gate_bias: Option<&CudaBuffer>,
        up_bias: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_moe_gate_up_swiglu(
            &weights.platform,
            mode,
            row_stride,
            rows_per_expert,
            columns,
            gate_rows,
            up_rows,
            &selected_ids.platform,
            selected_count,
            &input.platform,
            gate_bias.map(|buffer| &buffer.platform),
            up_bias.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes the packed GPT-OSS gate/up expert projection on device using a
    /// `Q8_1` activation buffer.
    #[allow(clippy::too_many_arguments)]
    pub fn moe_gate_up_swiglu_q8_1(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        columns: usize,
        gate_rows: usize,
        up_rows: usize,
        selected_ids: &CudaBuffer,
        selected_count: usize,
        input_q8_1: &CudaBuffer,
        gate_bias: Option<&CudaBuffer>,
        up_bias: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_moe_gate_up_swiglu_q8_1(
            &weights.platform,
            mode,
            row_stride,
            rows_per_expert,
            columns,
            gate_rows,
            up_rows,
            &selected_ids.platform,
            selected_count,
            &input_q8_1.platform,
            gate_bias.map(|buffer| &buffer.platform),
            up_bias.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes the selected4 GPT-OSS gate/up expert projection and writes the
    /// activated expert outputs directly as `Q8_1` blocks.
    #[allow(clippy::too_many_arguments)]
    pub fn moe_gate_up_swiglu_q8_1_selected4_quantized(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        columns: usize,
        gate_rows: usize,
        up_rows: usize,
        selected_ids: &CudaBuffer,
        selected_count: usize,
        input_q8_1: &CudaBuffer,
        gate_bias: Option<&CudaBuffer>,
        up_bias: Option<&CudaBuffer>,
        output_q8_1: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform
            .encode_moe_gate_up_swiglu_q8_1_selected4_quantized(
                &weights.platform,
                mode,
                row_stride,
                rows_per_expert,
                columns,
                gate_rows,
                up_rows,
                &selected_ids.platform,
                selected_count,
                &input_q8_1.platform,
                gate_bias.map(|buffer| &buffer.platform),
                up_bias.map(|buffer| &buffer.platform),
                &output_q8_1.platform,
            )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes the ids-driven GPT-OSS gate/up expert path and writes the
    /// activated expert outputs directly as `Q8_1` blocks.
    #[allow(clippy::too_many_arguments)]
    pub fn expert_gate_up_swiglu_q8_1_ids(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows_per_expert: usize,
        columns: usize,
        gate_rows: usize,
        up_rows: usize,
        selected_ids: &CudaBuffer,
        selected_count: usize,
        input_q8_1: &CudaBuffer,
        gate_bias: Option<&CudaBuffer>,
        up_bias: Option<&CudaBuffer>,
        output_q8_1: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.moe_gate_up_swiglu_q8_1_selected4_quantized(
            weights,
            mode,
            row_stride,
            rows_per_expert,
            columns,
            gate_rows,
            up_rows,
            selected_ids,
            selected_count,
            input_q8_1,
            gate_bias,
            up_bias,
            output_q8_1,
        )
    }

    /// Executes the GPT-OSS down projection and route-weighted expert
    /// aggregation on device for the selected experts.
    #[allow(clippy::too_many_arguments)]
    pub fn moe_down_aggregate(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows: usize,
        columns: usize,
        selected_ids: &CudaBuffer,
        selected_weights: &CudaBuffer,
        selected_count: usize,
        activated: &CudaBuffer,
        bias: Option<&CudaBuffer>,
        residual: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_moe_down_aggregate(
            &weights.platform,
            mode,
            row_stride,
            rows,
            columns,
            &selected_ids.platform,
            &selected_weights.platform,
            selected_count,
            &activated.platform,
            bias.map(|buffer| &buffer.platform),
            residual.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes the GPT-OSS down projection and route-weighted expert
    /// aggregation on device using row-wise `Q8_1` activations.
    #[allow(clippy::too_many_arguments)]
    pub fn moe_down_aggregate_q8_1(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows: usize,
        columns: usize,
        selected_ids: &CudaBuffer,
        selected_weights: &CudaBuffer,
        selected_count: usize,
        activated_q8_1: &CudaBuffer,
        bias: Option<&CudaBuffer>,
        residual: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_moe_down_aggregate_q8_1(
            &weights.platform,
            mode,
            row_stride,
            rows,
            columns,
            &selected_ids.platform,
            &selected_weights.platform,
            selected_count,
            &activated_q8_1.platform,
            bias.map(|buffer| &buffer.platform),
            residual.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes the GPT-OSS down projection and route-weighted expert
    /// aggregation on device by quantizing selected expert activations from
    /// `f32` into shared `Q8_1` blocks inside the selected-4 fast path.
    #[allow(clippy::too_many_arguments)]
    pub fn moe_down_aggregate_q8_1_f32(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows: usize,
        columns: usize,
        selected_ids: &CudaBuffer,
        selected_weights: &CudaBuffer,
        selected_count: usize,
        activated: &CudaBuffer,
        bias: Option<&CudaBuffer>,
        residual: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_moe_down_aggregate_q8_1_f32(
            &weights.platform,
            mode,
            row_stride,
            rows,
            columns,
            &selected_ids.platform,
            &selected_weights.platform,
            selected_count,
            &activated.platform,
            bias.map(|buffer| &buffer.platform),
            residual.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes the GPT-OSS selected4 down projection into expert-specific
    /// output rows, leaving route-weighted accumulation as a separate step.
    #[allow(clippy::too_many_arguments)]
    pub fn moe_down_project_q8_1_selected4(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows: usize,
        columns: usize,
        selected_ids: &CudaBuffer,
        selected_count: usize,
        activated_q8_1: &CudaBuffer,
        bias: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_moe_down_project_q8_1_selected4(
            &weights.platform,
            mode,
            row_stride,
            rows,
            columns,
            &selected_ids.platform,
            selected_count,
            &activated_q8_1.platform,
            bias.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Executes one ids-driven grouped expert projection from row-wise `Q8_1`
    /// activations into expert-specific `f32` output rows.
    #[allow(clippy::too_many_arguments)]
    pub fn expert_matvec_q8_1_ids(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        row_stride: usize,
        rows: usize,
        columns: usize,
        selected_ids: &CudaBuffer,
        selected_count: usize,
        input_q8_1: &CudaBuffer,
        bias: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.moe_down_project_q8_1_selected4(
            weights,
            mode,
            row_stride,
            rows,
            columns,
            selected_ids,
            selected_count,
            input_q8_1,
            bias,
            output,
        )
    }

    /// Accumulates up to four expert-specific output rows using route weights
    /// and an optional residual vector.
    pub fn accumulate_selected4(
        &mut self,
        input: &CudaBuffer,
        selected_weights: &CudaBuffer,
        selected_count: usize,
        rows: usize,
        residual: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.platform.encode_accumulate_selected4(
            &input.platform,
            &selected_weights.platform,
            selected_count,
            rows,
            residual.map(|buffer| &buffer.platform),
            &output.platform,
        )?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Accumulates grouped expert output rows using route weights and an
    /// optional residual vector.
    pub fn accumulate_expert_outputs(
        &mut self,
        input: &CudaBuffer,
        selected_weights: &CudaBuffer,
        selected_count: usize,
        rows: usize,
        residual: Option<&CudaBuffer>,
        output: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        self.accumulate_selected4(
            input,
            selected_weights,
            selected_count,
            rows,
            residual,
            output,
        )
    }

    /// Synchronizes the CUDA stream and returns explicit submission metadata.
    pub fn commit(self, wait: CudaCommandWait) -> Result<CudaSubmissionReport, RuntimeError> {
        if self.capturing {
            return Err(RuntimeError::Backend(String::from(
                "captured cuda submissions must use commit_captured",
            )));
        }
        let status = self.platform.commit(wait)?;
        Ok(CudaSubmissionReport {
            status,
            encoded_operations: self.encoded_operations,
        })
    }

    /// Finalizes stream capture, instantiates a reusable CUDA graph, launches it
    /// once for the current step, and returns the executable for later replays.
    pub fn commit_captured(
        self,
        wait: CudaCommandWait,
    ) -> Result<(CudaSubmissionReport, CudaGraphExec), RuntimeError> {
        if !self.capturing {
            return Err(RuntimeError::Backend(String::from(
                "ordinary cuda submissions must use commit",
            )));
        }
        let (status, graph_exec) = self.platform.commit_captured(wait)?;
        Ok((
            CudaSubmissionReport {
                status,
                encoded_operations: self.encoded_operations,
            },
            CudaGraphExec {
                encoded_operations: self.encoded_operations,
                platform: graph_exec,
            },
        ))
    }
}

enum CudaBackendState {
    Available(Box<AvailableCudaBackend>),
    Unavailable(RuntimeHealth),
}

struct AvailableCudaBackend {
    descriptor: DeviceDescriptor,
    platform: platform::ConfiguredBackend,
    allocator_pool: AllocatorPoolReport,
    execution_plan_cache: CudaExecutionPlanCache,
    kernel_cache: KernelCacheReport,
}

/// CUDA backend probe backed by `nvidia-smi` discovery plus `libcudart` buffer
/// and stream substrate.
pub struct CudaBackend {
    state: CudaBackendState,
}

impl Default for CudaBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl CudaBackend {
    /// Creates a CUDA backend and selects the first discovered device when the
    /// CUDA runtime substrate is available.
    #[must_use]
    pub fn new() -> Self {
        match discovery_report_internal() {
            Ok(report) => {
                let Some(descriptor) = report.devices.first().cloned() else {
                    return Self {
                        state: CudaBackendState::Unavailable(report.health),
                    };
                };
                let allocator_pool = AllocatorPoolReport {
                    policy: cuda_allocator_pool_policy(),
                    state: AllocatorPoolState::default(),
                };
                let kernel_cache = KernelCacheReport {
                    policy: KernelCachePolicy::disabled(),
                    state: KernelCacheState::default(),
                };
                match platform::configure_backend(descriptor.clone()) {
                    Ok(platform) => Self {
                        state: CudaBackendState::Available(Box::new(AvailableCudaBackend {
                            descriptor,
                            platform,
                            allocator_pool,
                            execution_plan_cache: CudaExecutionPlanCache::new(
                                cuda_execution_plan_cache_policy(),
                            ),
                            kernel_cache,
                        })),
                    },
                    Err(error) => Self {
                        state: CudaBackendState::Unavailable(RuntimeHealth {
                            status: report.health.status,
                            message: format!("cuda runtime substrate unavailable: {error}"),
                        }),
                    },
                }
            }
            Err(error) => Self {
                state: CudaBackendState::Unavailable(RuntimeHealth {
                    status: HealthStatus::Offline,
                    message: error.to_string(),
                }),
            },
        }
    }

    /// Returns the backend-local NVIDIA discovery report.
    pub fn discovery_report(&self) -> Result<NvidiaBackendReport, RuntimeError> {
        discovery_report_internal()
    }

    /// Returns the device selected for allocation/submission, when available.
    #[must_use]
    pub fn selected_device(&self) -> Option<&DeviceDescriptor> {
        match &self.state {
            CudaBackendState::Available(backend) => Some(&backend.descriptor),
            CudaBackendState::Unavailable(_) => None,
        }
    }

    /// Returns explicit runtime resource posture for the selected CUDA device.
    #[must_use]
    pub fn runtime_resources(&self) -> Option<BackendRuntimeResources> {
        match &self.state {
            CudaBackendState::Available(backend) => Some(BackendRuntimeResources {
                execution_plan_cache: backend.execution_plan_cache.report(),
                allocator_pool: backend.allocator_pool.clone(),
                kernel_cache: backend.kernel_cache.clone(),
                device_memory_budget: Some(DeviceMemoryBudget::new(
                    backend.descriptor.memory_capacity_bytes,
                    backend.allocator_pool.policy.max_cached_bytes,
                    backend
                        .kernel_cache
                        .policy
                        .max_cached_bytes
                        .unwrap_or(backend.kernel_cache.state.cached_bytes),
                )),
            }),
            CudaBackendState::Unavailable(_) => None,
        }
    }

    /// Creates a dense `f32` input buffer on the selected CUDA device.
    pub fn input_buffer(
        &mut self,
        shape: Shape,
        values: impl Into<Vec<f32>>,
    ) -> Result<CudaBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "cuda backend unavailable: no selected execution device",
            )));
        };
        let mut buffer = self.allocate(&TensorSpec::new(shape, DType::F32, device))?;
        buffer.write_f32(values.into().as_slice())?;
        Ok(buffer)
    }

    /// Allocates an uninitialized dense `f32` buffer on the selected CUDA device.
    pub fn f32_buffer(&mut self, len: usize) -> Result<CudaBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "cuda backend unavailable: no selected execution device",
            )));
        };
        self.allocate_buffer(&TensorSpec::new(Shape::new(vec![len]), DType::F32, device))
    }

    /// Allocates an uninitialized dense `f16` buffer on the selected CUDA device.
    pub fn f16_buffer(&mut self, len: usize) -> Result<CudaBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "cuda backend unavailable: no selected execution device",
            )));
        };
        self.allocate_buffer(&TensorSpec::new(Shape::new(vec![len]), DType::F16, device))
    }

    /// Allocates an uninitialized dense `i32` buffer on the selected CUDA device.
    pub fn i32_buffer(&mut self, len: usize) -> Result<CudaBuffer, RuntimeError> {
        self.byte_buffer(&vec![0_u8; len.saturating_mul(size_of::<i32>())])
    }

    /// Allocates a page-locked host buffer for stream-owned staging transfers.
    pub fn host_buffer(&mut self, byte_len: usize) -> Result<CudaHostBuffer, RuntimeError> {
        let Some(backend) = self.selected_backend() else {
            return Err(RuntimeError::Backend(self.health().message));
        };
        Ok(CudaHostBuffer {
            byte_len,
            platform: backend.platform.allocate_host(byte_len)?,
        })
    }

    /// Returns whether the Psionic-owned CUDA quantized text-generation kernels are built.
    #[must_use]
    pub fn quantized_kernels_available(&self) -> bool {
        platform::quantized_kernels_compiled()
    }

    /// Uploads raw bytes into device memory for backend-owned quantized storage.
    pub fn byte_buffer(&mut self, bytes: &[u8]) -> Result<CudaBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "cuda backend unavailable: no selected execution device",
            )));
        };
        let spec = TensorSpec::new(Shape::new(vec![bytes.len()]), DType::I8, device);
        let Some(backend) = self.selected_backend() else {
            return Err(RuntimeError::Backend(self.health().message));
        };
        let mut buffer = backend.allocate(&spec)?;
        buffer.write_bytes(bytes)?;
        Ok(buffer)
    }

    /// Executes one quantized row-wise matrix-vector product on CUDA.
    pub fn quantized_matvec(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input: &[f32],
    ) -> Result<Vec<f32>, RuntimeError> {
        Ok(self
            .quantized_matvec_profiled(weights, mode, rows, cols, input)?
            .values)
    }

    /// Executes one profiled quantized row-wise matrix-vector product on CUDA.
    pub fn quantized_matvec_profiled(
        &mut self,
        weights: &CudaBuffer,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input: &[f32],
    ) -> Result<CudaQuantizedMatvecResult, RuntimeError> {
        self.quantized_matvec_with_offset_profiled(weights, 0, mode, rows, cols, input)
    }

    /// Executes one quantized row-wise matrix-vector product from a byte offset.
    pub fn quantized_matvec_with_offset(
        &mut self,
        weights: &CudaBuffer,
        byte_offset: usize,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input: &[f32],
    ) -> Result<Vec<f32>, RuntimeError> {
        Ok(self
            .quantized_matvec_with_offset_profiled(weights, byte_offset, mode, rows, cols, input)?
            .values)
    }

    /// Executes one profiled quantized row-wise matrix-vector product from a byte offset.
    pub fn quantized_matvec_with_offset_profiled(
        &mut self,
        weights: &CudaBuffer,
        byte_offset: usize,
        mode: QuantizationMode,
        rows: usize,
        cols: usize,
        input: &[f32],
    ) -> Result<CudaQuantizedMatvecResult, RuntimeError> {
        if !self.quantized_kernels_available() {
            return Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels are not available in this build",
            )));
        }
        if weights.spec().dtype() != DType::I8 {
            return Err(RuntimeError::Backend(format!(
                "cuda quantized matvec requires raw byte weights, actual {:?}",
                weights.spec().dtype()
            )));
        }
        let Some((elements_per_block, bytes_per_block)) = mode.ggml_block_spec() else {
            return Err(RuntimeError::Backend(format!(
                "cuda quantized matvec does not support mode {mode:?}"
            )));
        };
        if cols == 0 || cols % elements_per_block != 0 {
            return Err(RuntimeError::Backend(format!(
                "cuda quantized matvec requires block-aligned width {cols} for {mode:?}"
            )));
        }
        if input.len() != cols {
            return Err(RuntimeError::Backend(format!(
                "cuda quantized matvec input width mismatch: expected {cols}, actual {}",
                input.len()
            )));
        }
        let row_stride = (cols / elements_per_block) * bytes_per_block;
        let required_bytes = rows.saturating_mul(row_stride);
        let expected_end = byte_offset.saturating_add(required_bytes);
        if weights.byte_len() < expected_end {
            return Err(RuntimeError::Backend(format!(
                "cuda quantized matvec byte length mismatch: required {expected_end}, actual {}",
                weights.byte_len(),
            )));
        }

        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "cuda backend unavailable: no selected execution device",
            )));
        };
        let input_buffer = self.input_buffer(Shape::new(vec![cols]), input.to_vec())?;
        let output =
            self.allocate_buffer(&TensorSpec::new(Shape::new(vec![rows]), DType::F32, device))?;
        let mut submission = self.begin_submission()?;
        submission.quantized_matvec(
            weights,
            byte_offset,
            mode,
            rows,
            cols,
            &input_buffer,
            &output,
        )?;
        let _ = submission.commit(CudaCommandWait::Completed)?;
        let values = output.read_f32()?;
        Ok(CudaQuantizedMatvecResult {
            values,
            stats: CudaQuantizedMatvecStats {
                host_to_device_bytes: (input.len())
                    .saturating_mul(size_of_dtype(DType::F32))
                    .try_into()
                    .unwrap_or(u64::MAX),
                device_to_host_bytes: (rows)
                    .saturating_mul(size_of_dtype(DType::F32))
                    .try_into()
                    .unwrap_or(u64::MAX),
                submission_count: 1,
                sync_count: 1,
                kernel_launches: 1,
            },
        })
    }

    /// Compiles and executes a graph on the supported dense CUDA surface.
    pub fn compile_and_execute(
        &mut self,
        graph: &Graph,
        inputs: &BTreeMap<TensorId, CudaBuffer>,
    ) -> Result<ExecutionResult<CudaBuffer>, RuntimeError> {
        let Some(backend) = self.selected_backend_mut() else {
            return Err(RuntimeError::Backend(self.health().message));
        };
        let (plan, plan_digest, compile_path) = backend.lookup_or_compile(graph)?;
        let mut result = backend.execute(&plan, inputs)?;
        result.metrics.execution_plan_digest = Some(plan_digest);
        result.metrics.compile_path = Some(compile_path);
        result.metrics.plan_cache_hits = usize::from(matches!(
            result
                .metrics
                .compile_path
                .as_ref()
                .map(|value| value.temperature),
            Some(CompilePathTemperature::WarmReuse)
        ));
        result.metrics.plan_cache_misses = usize::from(matches!(
            result
                .metrics
                .compile_path
                .as_ref()
                .map(|value| value.temperature),
            Some(CompilePathTemperature::ColdCompile)
        ));
        Ok(result)
    }

    /// Returns truthful backend-selection data for a supported CUDA product path.
    pub fn backend_selection(
        &self,
        supported_ops: &[&str],
    ) -> Result<BackendSelection, RuntimeError> {
        let policy = ServedProductBackendPolicy::fallback_to_compatible_backend(
            BackendDegradedPolicy::AllowSameBackend,
        );
        match &self.state {
            CudaBackendState::Available(backend) => {
                let supported_ops = supported_ops
                    .iter()
                    .map(|label| String::from(*label))
                    .collect();
                let health = self.health();
                match health.status {
                    HealthStatus::Ready => Ok(BackendSelection::direct_with_policy(
                        self.backend_name(),
                        Some(backend.descriptor.clone()),
                        supported_ops,
                        policy,
                    )
                    .with_runtime_resources(self.runtime_resources())
                    .with_backend_extensions(self.extension_support())),
                    HealthStatus::Degraded => Ok(BackendSelection::degraded(
                        self.backend_name(),
                        Some(backend.descriptor.clone()),
                        supported_ops,
                        policy,
                        health.message,
                    )
                    .with_runtime_resources(self.runtime_resources())
                    .with_backend_extensions(self.extension_support())),
                    HealthStatus::Offline => Err(RuntimeError::Backend(format!(
                        "cuda backend unavailable: {}",
                        health.message
                    ))),
                }
            }
            CudaBackendState::Unavailable(health) => Err(RuntimeError::Backend(format!(
                "cuda backend unavailable: {}",
                health.message
            ))),
        }
    }

    /// Returns an explicit fallback selection when CUDA cannot execute the
    /// requested product path on the local machine.
    pub fn fallback_selection<B>(
        &self,
        fallback_backend: &B,
        supported_ops: &[&str],
    ) -> Result<BackendSelection, RuntimeError>
    where
        B: DeviceDiscovery + ?Sized,
    {
        let policy = ServedProductBackendPolicy::fallback_to_compatible_backend(
            BackendDegradedPolicy::AllowSameBackend,
        );
        match &self.state {
            CudaBackendState::Available(_) => self.backend_selection(supported_ops),
            CudaBackendState::Unavailable(health) => Ok(BackendSelection::fallback_with_policy(
                self.backend_name(),
                fallback_backend.backend_name(),
                fallback_backend.discover_devices()?.into_iter().next(),
                supported_ops
                    .iter()
                    .map(|label| String::from(*label))
                    .collect(),
                policy,
                format!("cuda backend unavailable: {}", health.message),
            )
            .with_runtime_resources(fallback_backend.runtime_resources())
            .with_backend_extensions(fallback_backend.extension_support())),
        }
    }

    /// Allocates a CUDA buffer for the provided tensor specification.
    pub fn allocate_buffer(&mut self, spec: &TensorSpec) -> Result<CudaBuffer, RuntimeError> {
        let Some(backend) = self.selected_backend() else {
            let message = match &self.state {
                CudaBackendState::Unavailable(health) => health.message.clone(),
                CudaBackendState::Available(_) => String::from("cuda backend unavailable"),
            };
            return Err(RuntimeError::Backend(message));
        };
        if spec.dtype() != DType::F32 && spec.dtype() != DType::F16 {
            return Err(RuntimeError::Backend(format!(
                "cuda dense surface only supports F32/F16 buffers, actual {:?}",
                spec.dtype()
            )));
        }
        if spec.device().kind() != DeviceKind::Cuda {
            return Err(RuntimeError::Backend(format!(
                "cuda allocator requires a CUDA tensor spec, actual device kind {}",
                spec.device().kind()
            )));
        }
        if spec.device().ordinal() != backend.descriptor.device.ordinal() {
            return Err(RuntimeError::Backend(format!(
                "cuda allocator requires device ordinal {}, actual {}",
                backend.descriptor.device.ordinal(),
                spec.device().ordinal()
            )));
        }
        let byte_len = spec
            .storage_size()
            .checked_mul(size_of_dtype(spec.dtype()))
            .ok_or_else(|| {
                RuntimeError::Backend(format!(
                    "cuda buffer size overflow for tensor storage size {}",
                    spec.storage_size()
                ))
            })?;
        let platform_buffer = backend.platform.allocate(byte_len)?;
        Ok(CudaBuffer {
            spec: spec.clone(),
            byte_len,
            memory_space: CudaMemorySpace::Device,
            host_visible: false,
            platform: platform_buffer,
        })
    }

    /// Begins a CUDA submission on a fresh stream.
    pub fn begin_submission(&self) -> Result<CudaSubmission, RuntimeError> {
        let Some(backend) = self.selected_backend() else {
            return Err(RuntimeError::Backend(self.health().message));
        };
        Ok(CudaSubmission {
            encoded_operations: 0,
            capturing: false,
            platform: backend.platform.begin_submission()?,
        })
    }

    /// Begins recording a CUDA submission into a reusable captured graph.
    pub fn begin_captured_submission(&self) -> Result<CudaSubmission, RuntimeError> {
        let Some(backend) = self.selected_backend() else {
            return Err(RuntimeError::Backend(self.health().message));
        };
        Ok(CudaSubmission {
            encoded_operations: 0,
            capturing: true,
            platform: backend.platform.begin_capture_submission()?,
        })
    }

    fn selected_backend(&self) -> Option<&AvailableCudaBackend> {
        match &self.state {
            CudaBackendState::Available(backend) => Some(backend),
            CudaBackendState::Unavailable(_) => None,
        }
    }

    fn selected_backend_mut(&mut self) -> Option<&mut AvailableCudaBackend> {
        match &mut self.state {
            CudaBackendState::Available(backend) => Some(backend),
            CudaBackendState::Unavailable(_) => None,
        }
    }
}

impl DeviceDiscovery for CudaBackend {
    fn backend_name(&self) -> BackendName {
        "cuda"
    }

    fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError> {
        self.discovery_report().map(|report| report.devices)
    }

    fn health(&self) -> RuntimeHealth {
        match &self.state {
            CudaBackendState::Available(_) => match self.discovery_report() {
                Ok(report) => report.health,
                Err(error) => RuntimeHealth {
                    status: HealthStatus::Degraded,
                    message: format!("cuda discovery failed: {error}"),
                },
            },
            CudaBackendState::Unavailable(health) => health.clone(),
        }
    }

    fn runtime_resources(&self) -> Option<BackendRuntimeResources> {
        CudaBackend::runtime_resources(self)
    }
}

impl Allocator for CudaBackend {
    type Buffer = CudaBuffer;

    fn allocate(&mut self, spec: &TensorSpec) -> Result<Self::Buffer, RuntimeError> {
        self.allocate_buffer(spec)
    }
}

impl ExecutionBackend for CudaBackend {
    type Buffer = CudaBuffer;

    fn execute(
        &mut self,
        plan: &ExecutionPlan,
        inputs: &BTreeMap<TensorId, Self::Buffer>,
    ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError> {
        validate_supported_plan(plan)?;
        match &self.state {
            CudaBackendState::Available(backend) => backend.execute(plan, inputs),
            CudaBackendState::Unavailable(health) => Err(RuntimeError::Backend(format!(
                "cuda backend unavailable: {}",
                health.message
            ))),
        }
    }
}

impl AvailableCudaBackend {
    fn lookup_or_compile(
        &mut self,
        graph: &Graph,
    ) -> Result<(ExecutionPlan, String, CompilePathEvidence), RuntimeError> {
        let (plan, plan_digest, plan_cache_hit) =
            self.execution_plan_cache.lookup_or_compile(graph)?;
        let kernel_cache = if self.kernel_cache.policy.enabled {
            CacheObservation::new(
                CacheKind::KernelCache,
                CacheAction::Reuse,
                "reused the configured cuda kernel cache",
            )
        } else {
            CacheObservation::new(
                CacheKind::KernelCache,
                CacheAction::Bypass,
                "cuda kernel cache is disabled for this backend path",
            )
        };
        Ok((
            plan,
            plan_digest,
            CompilePathEvidence {
                temperature: if plan_cache_hit {
                    CompilePathTemperature::WarmReuse
                } else {
                    CompilePathTemperature::ColdCompile
                },
                execution_plan_cache: if plan_cache_hit {
                    CacheObservation::new(
                        CacheKind::ExecutionPlan,
                        CacheAction::Reuse,
                        "reused a cached cuda execution plan",
                    )
                } else {
                    CacheObservation::new(
                        CacheKind::ExecutionPlan,
                        CacheAction::Rebuild,
                        "compiled a new cuda execution plan",
                    )
                },
                kernel_cache,
            },
        ))
    }

    fn allocate(&self, spec: &TensorSpec) -> Result<CudaBuffer, RuntimeError> {
        let byte_len = spec
            .storage_size()
            .checked_mul(size_of_dtype(spec.dtype()))
            .ok_or_else(|| {
                RuntimeError::Backend(format!(
                    "cuda buffer size overflow for tensor storage size {}",
                    spec.storage_size()
                ))
            })?;
        let platform_buffer = self.platform.allocate(byte_len)?;
        Ok(CudaBuffer {
            spec: spec.clone(),
            byte_len,
            memory_space: CudaMemorySpace::Device,
            host_visible: false,
            platform: platform_buffer,
        })
    }

    fn buffer_from_tensor_data(
        &self,
        spec: &TensorSpec,
        data: &TensorData,
    ) -> Result<CudaBuffer, RuntimeError> {
        let Some(values) = data.as_f32_slice() else {
            return Err(RuntimeError::Backend(String::from(
                "cuda constant storage must use dense f32 payloads",
            )));
        };
        if values.len() != spec.storage_size() {
            return Err(RuntimeError::Backend(String::from(
                "cuda constant payload length mismatch",
            )));
        }
        let mut buffer = self.allocate(spec)?;
        buffer.write_f32(values)?;
        Ok(buffer)
    }

    fn execute(
        &self,
        plan: &ExecutionPlan,
        inputs: &BTreeMap<TensorId, CudaBuffer>,
    ) -> Result<ExecutionResult<CudaBuffer>, RuntimeError> {
        let mut submission = CudaSubmission {
            encoded_operations: 0,
            capturing: false,
            platform: self.platform.begin_submission()?,
        };
        let mut values = BTreeMap::new();

        for step in &plan.steps {
            match &step.op {
                ExecutionOp::Input { .. } => {
                    let input = inputs
                        .get(&step.output)
                        .ok_or(RuntimeError::MissingInput(step.output))?;
                    if input.spec() != &step.spec {
                        return Err(RuntimeError::InvalidBuffer {
                            tensor: step.output,
                            expected: step.spec.clone(),
                            actual: input.spec().clone(),
                        });
                    }
                    values.insert(step.output, input.clone());
                }
                ExecutionOp::Constant { data } => {
                    values.insert(step.output, self.buffer_from_tensor_data(&step.spec, data)?);
                }
                ExecutionOp::Add => {
                    let (left, right) = binary_inputs(step, &values)?;
                    let output = self.allocate(&step.spec)?;
                    submission.platform.encode_add(
                        &left.platform,
                        &right.platform,
                        &output.platform,
                        step.spec.element_count(),
                    )?;
                    submission.encoded_operations += 1;
                    values.insert(step.output, output);
                }
                ExecutionOp::Matmul => {
                    let (left, right) = binary_inputs(step, &values)?;
                    let left_dims = left.spec().shape().dims();
                    let right_dims = right.spec().shape().dims();
                    if left_dims.len() != 2
                        || right_dims.len() != 2
                        || left_dims[1] != right_dims[0]
                    {
                        return Err(RuntimeError::Backend(String::from(
                            "invalid matmul shapes at runtime",
                        )));
                    }
                    let output = self.allocate(&step.spec)?;
                    submission.platform.encode_matmul(
                        &left.platform,
                        &right.platform,
                        &output.platform,
                        left_dims[0],
                        left_dims[1],
                        right_dims[1],
                    )?;
                    submission.encoded_operations += 1;
                    values.insert(step.output, output);
                }
                _ => {
                    return Err(RuntimeError::UnsupportedStep(step.op.label().to_string()));
                }
            }
        }

        let _report = submission.commit(CudaCommandWait::Completed)?;
        let mut outputs = BTreeMap::new();
        for output_id in &plan.outputs {
            let Some(buffer) = values.remove(output_id) else {
                return Err(RuntimeError::MissingInput(*output_id));
            };
            outputs.insert(*output_id, buffer);
        }
        Ok(ExecutionResult {
            outputs,
            metrics: ExecutionMetrics {
                steps_executed: plan.steps.len(),
                kernel_count: plan.steps.len(),
                bytes_moved: plan_output_bytes(plan),
                plan_cache_hits: 0,
                plan_cache_misses: 0,
                execution_plan_digest: None,
                compile_path: None,
            },
        })
    }
}

fn cuda_execution_plan_cache_policy() -> ExecutionPlanCachePolicy {
    ExecutionPlanCachePolicy::bounded(
        CUDA_EXECUTION_PLAN_CACHE_MAX_ENTRIES,
        Some(CUDA_EXECUTION_PLAN_CACHE_MAX_CACHED_BYTES),
    )
}

#[derive(Clone, Debug)]
struct CachedCudaExecutionPlan {
    plan: ExecutionPlan,
    plan_digest: String,
}

#[derive(Clone, Debug)]
struct CudaExecutionPlanCache {
    policy: ExecutionPlanCachePolicy,
    cached: std::collections::HashMap<String, CachedCudaExecutionPlan>,
    state: ExecutionPlanCacheState,
}

impl CudaExecutionPlanCache {
    fn new(policy: ExecutionPlanCachePolicy) -> Self {
        Self {
            policy,
            cached: std::collections::HashMap::new(),
            state: ExecutionPlanCacheState::default(),
        }
    }

    fn report(&self) -> ExecutionPlanCacheReport {
        ExecutionPlanCacheReport {
            policy: self.policy.clone(),
            state: self.state.clone(),
        }
    }

    fn lookup_or_compile(
        &mut self,
        graph: &Graph,
    ) -> Result<(ExecutionPlan, String, bool), RuntimeError> {
        let cache_key = graph.stable_digest();
        if let Some(cached) = self.cached.get(&cache_key) {
            return Ok((cached.plan.clone(), cached.plan_digest.clone(), true));
        }

        let plan =
            compile_graph(graph).map_err(|error| RuntimeError::Backend(error.to_string()))?;
        let plan_digest = plan.stable_digest();
        let estimated_bytes = estimate_execution_plan_bytes(&plan, &plan_digest);
        if self.policy.enabled
            && self.cached.len() < self.policy.max_cached_entries
            && self
                .policy
                .max_cached_bytes
                .map(|limit| self.state.cached_bytes.saturating_add(estimated_bytes) <= limit)
                .unwrap_or(true)
        {
            self.cached.insert(
                cache_key,
                CachedCudaExecutionPlan {
                    plan: plan.clone(),
                    plan_digest: plan_digest.clone(),
                },
            );
            self.state.cached_entries = self.cached.len();
            self.state.cached_bytes = self.state.cached_bytes.saturating_add(estimated_bytes);
        }
        Ok((plan, plan_digest, false))
    }
}

fn estimate_execution_plan_bytes(plan: &ExecutionPlan, plan_digest: &str) -> u64 {
    plan.stable_debug()
        .len()
        .saturating_add(plan_digest.len())
        .try_into()
        .unwrap_or(u64::MAX)
}

fn plan_output_bytes(plan: &ExecutionPlan) -> u64 {
    plan.steps
        .iter()
        .map(|step| {
            step.spec
                .storage_size()
                .saturating_mul(step.spec.dtype().element_size_bytes())
                .try_into()
                .unwrap_or(u64::MAX)
        })
        .sum()
}

impl NvidiaInventoryRow {
    fn into_device_descriptor(self) -> DeviceDescriptor {
        let architecture = architecture_from_compute_capability(self.compute_capability.as_deref());
        let risk = risk_profile(
            self.display_attached,
            self.mig_profile.as_deref(),
            self.persistence_mode_enabled,
        );
        let recovery = recovery_profile(query_recovery_action(self.ordinal).ok().as_deref());

        let mut feature_flags = vec![String::from("cuda_architecture_surface")];
        if self.display_attached == Some(true) {
            feature_flags.push(String::from("display_attached"));
        }
        if self.mig_profile.is_some() {
            feature_flags.push(String::from("mig_enabled"));
        }
        if let Some(enabled) = self.persistence_mode_enabled {
            feature_flags.push(if enabled {
                String::from("persistence_mode_enabled")
            } else {
                String::from("persistence_mode_disabled")
            });
        }
        if let Some(addressing_mode) = self.addressing_mode.as_deref() {
            feature_flags.push(format!(
                "addressing_mode_{}",
                addressing_mode.to_ascii_lowercase()
            ));
        }

        DeviceDescriptor {
            backend: String::from("cuda"),
            device: Device::new(
                DeviceKind::Cuda,
                self.ordinal,
                Some(format!("cuda:{}", self.ordinal)),
            ),
            device_name: Some(self.name),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: self.memory_bytes,
            unified_memory: Some(false),
            feature_flags,
            amd_metadata: None,
            nvidia_metadata: Some(NvidiaDeviceMetadata {
                topology: NvidiaTopologyInfo {
                    architecture,
                    compute_capability: self.compute_capability,
                    pci_bdf: self.pci_bdf,
                    sm_count: None,
                    vram_bytes: self.memory_bytes,
                    mig_profile: self.mig_profile,
                },
                risk,
                recovery,
            }),
        }
    }
}

fn cuda_allocator_pool_policy() -> AllocatorPoolPolicy {
    AllocatorPoolPolicy::exact_tensor_spec(CUDA_POOL_MAX_CACHED_BUFFERS, CUDA_POOL_MAX_CACHED_BYTES)
}

fn size_of_dtype(dtype: DType) -> usize {
    dtype.element_size_bytes()
}

fn validate_supported_plan(plan: &ExecutionPlan) -> Result<(), RuntimeError> {
    for step in &plan.steps {
        validate_supported_step(step)?;
    }
    Ok(())
}

fn validate_supported_step(step: &ExecutionStep) -> Result<(), RuntimeError> {
    ensure_supported_spec(&step.spec)?;
    match &step.op {
        ExecutionOp::Input { .. } => {
            if !step.inputs.is_empty() {
                return Err(RuntimeError::Backend(format!(
                    "cuda input step {} unexpectedly has inputs",
                    step.output
                )));
            }
        }
        ExecutionOp::Constant { data } => {
            let Some(values) = data.as_f32_slice() else {
                return Err(RuntimeError::Backend(format!(
                    "cuda constant {} must use dense f32 storage",
                    step.output
                )));
            };
            if values.len() != step.spec.storage_size() {
                return Err(RuntimeError::Backend(format!(
                    "cuda constant {} payload length mismatch",
                    step.output
                )));
            }
        }
        ExecutionOp::Add => {
            if step.inputs.len() != 2 {
                return Err(RuntimeError::Backend(format!(
                    "cuda add step {} requires two inputs",
                    step.output
                )));
            }
        }
        ExecutionOp::Matmul => {
            if step.inputs.len() != 2 {
                return Err(RuntimeError::Backend(format!(
                    "cuda matmul step {} requires two inputs",
                    step.output
                )));
            }
            let dims = step.spec.shape().dims();
            if dims.len() != 2 {
                return Err(RuntimeError::Backend(format!(
                    "cuda matmul step {} requires a rank-2 output, actual rank {}",
                    step.output,
                    dims.len()
                )));
            }
        }
        _ => {
            return Err(RuntimeError::UnsupportedStep(step.op.label().to_string()));
        }
    }
    Ok(())
}

fn ensure_supported_spec(spec: &TensorSpec) -> Result<(), RuntimeError> {
    if spec.dtype() != DType::F32 {
        return Err(RuntimeError::Backend(format!(
            "cuda dense surface only supports F32 tensors, actual {:?}",
            spec.dtype()
        )));
    }
    if spec.device().kind() != DeviceKind::Cuda {
        return Err(RuntimeError::Backend(format!(
            "cuda dense surface requires CUDA tensor specs, actual device kind {}",
            spec.device().kind()
        )));
    }
    if !spec.layout().is_contiguous() || spec.layout().offset() != 0 {
        return Err(RuntimeError::Backend(String::from(
            "cuda dense surface requires contiguous zero-offset tensors",
        )));
    }
    Ok(())
}

fn binary_inputs<'a>(
    step: &ExecutionStep,
    values: &'a BTreeMap<TensorId, CudaBuffer>,
) -> Result<(&'a CudaBuffer, &'a CudaBuffer), RuntimeError> {
    let Some(left_id) = step.inputs.first().copied() else {
        return Err(RuntimeError::Backend(format!(
            "missing left input for step {}",
            step.output
        )));
    };
    let Some(right_id) = step.inputs.get(1).copied() else {
        return Err(RuntimeError::Backend(format!(
            "missing right input for step {}",
            step.output
        )));
    };
    let left = values
        .get(&left_id)
        .ok_or(RuntimeError::MissingInput(left_id))?;
    let right = values
        .get(&right_id)
        .ok_or(RuntimeError::MissingInput(right_id))?;
    if left.spec() != right.spec() && !matches!(step.op, ExecutionOp::Matmul) {
        return Err(RuntimeError::Backend(format!(
            "cuda {} requires matching input specs",
            step.op.label()
        )));
    }
    Ok((left, right))
}

fn discovery_report_internal() -> Result<NvidiaBackendReport, RuntimeError> {
    match query_inventory() {
        Ok(rows) => {
            let devices = rows
                .into_iter()
                .map(NvidiaInventoryRow::into_device_descriptor)
                .collect::<Vec<_>>();
            let health = cuda_health(&devices);
            Ok(NvidiaBackendReport { devices, health })
        }
        Err(error) if error.kind == NvidiaQueryErrorKind::NotInstalled => Ok(NvidiaBackendReport {
            devices: Vec::new(),
            health: RuntimeHealth {
                status: HealthStatus::Offline,
                message: String::from(OFFLINE_NO_DRIVER_MESSAGE),
            },
        }),
        Err(error) => Ok(NvidiaBackendReport {
            devices: Vec::new(),
            health: RuntimeHealth {
                status: HealthStatus::Offline,
                message: error.message,
            },
        }),
    }
}

fn architecture_from_compute_capability(compute_capability: Option<&str>) -> Option<String> {
    let architecture = match compute_capability?.trim() {
        "7.0" | "7.2" => "volta",
        "7.5" => "turing",
        "8.0" | "8.6" | "8.7" => "ampere",
        "8.9" => "ada",
        "9.0" => "hopper",
        _ => return None,
    };
    Some(String::from(architecture))
}

fn risk_profile(
    display_attached: Option<bool>,
    mig_profile: Option<&str>,
    persistence_mode_enabled: Option<bool>,
) -> NvidiaRiskProfile {
    let mut warnings = Vec::new();
    let mig_partitioned = mig_profile.is_some();
    if display_attached == Some(true) {
        warnings.push(String::from(
            "display-attached NVIDIA devices may show variable latency under local desktop load",
        ));
    }
    if mig_partitioned {
        warnings.push(String::from(
            "MIG-enabled NVIDIA devices expose only a partitioned slice of the physical GPU",
        ));
    }
    if persistence_mode_enabled == Some(false) {
        warnings.push(String::from(
            "persistence mode disabled; first request after idle may pay driver-load latency",
        ));
    }

    NvidiaRiskProfile {
        level: if display_attached == Some(true) || mig_partitioned {
            NvidiaRiskLevel::Elevated
        } else {
            NvidiaRiskLevel::Standard
        },
        display_attached,
        mig_partitioned,
        warnings,
    }
}

fn recovery_profile(recovery_action: Option<&str>) -> NvidiaRecoveryProfile {
    let normalized = normalize_value(recovery_action.unwrap_or_default())
        .map(|value| value.to_ascii_lowercase());
    match normalized.as_deref() {
        Some("none") => NvidiaRecoveryProfile {
            supports_gpu_reset: Some(true),
            expected_actions: vec![
                NvidiaRecoveryAction::ProcessRestart,
                NvidiaRecoveryAction::GpuReset,
                NvidiaRecoveryAction::RebootHost,
            ],
        },
        Some(value) if value.contains("reboot") => NvidiaRecoveryProfile {
            supports_gpu_reset: Some(false),
            expected_actions: vec![
                NvidiaRecoveryAction::ProcessRestart,
                NvidiaRecoveryAction::RebootHost,
            ],
        },
        Some(value) if value.contains("reset") => NvidiaRecoveryProfile {
            supports_gpu_reset: Some(true),
            expected_actions: vec![
                NvidiaRecoveryAction::ProcessRestart,
                NvidiaRecoveryAction::GpuReset,
                NvidiaRecoveryAction::RebootHost,
            ],
        },
        _ => NvidiaRecoveryProfile {
            supports_gpu_reset: None,
            expected_actions: vec![
                NvidiaRecoveryAction::ProcessRestart,
                NvidiaRecoveryAction::RebootHost,
            ],
        },
    }
}

fn cuda_health(devices: &[DeviceDescriptor]) -> RuntimeHealth {
    if devices.is_empty() {
        return RuntimeHealth {
            status: HealthStatus::Offline,
            message: String::from("cuda query succeeded but no NVIDIA GPUs were detected"),
        };
    }

    let elevated_devices = devices
        .iter()
        .filter(|device| {
            device
                .nvidia_metadata
                .as_ref()
                .map(|metadata| metadata.risk.level == NvidiaRiskLevel::Elevated)
                .unwrap_or(false)
        })
        .count();
    if elevated_devices > 0 {
        return RuntimeHealth {
            status: HealthStatus::Degraded,
            message: format!(
                "cuda discovered {} NVIDIA device(s); {} device(s) are display-attached or MIG-partitioned",
                devices.len(),
                elevated_devices
            ),
        };
    }

    RuntimeHealth {
        status: HealthStatus::Ready,
        message: format!("cuda ready on {} NVIDIA device(s)", devices.len()),
    }
}

fn query_inventory() -> Result<Vec<NvidiaInventoryRow>, NvidiaQueryError> {
    let stdout = run_nvidia_smi_query(INVENTORY_QUERY, None)?;
    let mut rows = Vec::new();
    for line in stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        rows.push(parse_inventory_row(line)?);
    }
    Ok(rows)
}

fn query_recovery_action(ordinal: u16) -> Result<String, NvidiaQueryError> {
    run_nvidia_smi_query("gpu_recovery_action", Some(ordinal))
        .map(|output| output.lines().next().unwrap_or_default().trim().to_string())
}

fn run_nvidia_smi_query(query: &str, ordinal: Option<u16>) -> Result<String, NvidiaQueryError> {
    let mut command = Command::new(NVIDIA_SMI_BINARY);
    command.arg(format!("--query-gpu={query}"));
    if let Some(ordinal) = ordinal {
        command.arg(format!("--id={ordinal}"));
    }
    command.args(["--format=csv,noheader,nounits"]);

    let output = command.output().map_err(|error| {
        let kind = if error.kind() == ErrorKind::NotFound {
            NvidiaQueryErrorKind::NotInstalled
        } else {
            NvidiaQueryErrorKind::Failed
        };
        NvidiaQueryError {
            kind,
            message: if kind == NvidiaQueryErrorKind::NotInstalled {
                String::from(OFFLINE_NO_DRIVER_MESSAGE)
            } else {
                format!("failed to execute {NVIDIA_SMI_BINARY}: {error}")
            },
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(NvidiaQueryError {
            kind: NvidiaQueryErrorKind::Failed,
            message: if stderr.is_empty() {
                format!("{NVIDIA_SMI_BINARY} exited with {}", output.status)
            } else {
                format!("{NVIDIA_SMI_BINARY} query failed: {stderr}")
            },
        });
    }

    String::from_utf8(output.stdout).map_err(|error| NvidiaQueryError {
        kind: NvidiaQueryErrorKind::InvalidUtf8,
        message: format!("{NVIDIA_SMI_BINARY} returned non-utf8 output: {error}"),
    })
}

fn parse_inventory_row(line: &str) -> Result<NvidiaInventoryRow, NvidiaQueryError> {
    let fields = line.split(',').map(str::trim).collect::<Vec<_>>();
    if fields.len() != 9 {
        return Err(NvidiaQueryError {
            kind: NvidiaQueryErrorKind::Failed,
            message: format!(
                "nvidia-smi returned {} inventory fields, expected 9: {line}",
                fields.len()
            ),
        });
    }

    let ordinal = fields[0].parse::<u16>().map_err(|error| NvidiaQueryError {
        kind: NvidiaQueryErrorKind::Failed,
        message: format!("invalid CUDA device ordinal {:?}: {error}", fields[0]),
    })?;

    Ok(NvidiaInventoryRow {
        ordinal,
        name: fields[1].to_string(),
        pci_bdf: normalize_value(fields[2]),
        memory_bytes: parse_memory_bytes(fields[3]),
        compute_capability: normalize_value(fields[4]),
        display_attached: parse_yes_no(fields[5]),
        mig_profile: parse_mig_profile(fields[6]),
        persistence_mode_enabled: parse_enabled_disabled(fields[7]),
        addressing_mode: normalize_value(fields[8]).map(|value| value.to_ascii_lowercase()),
    })
}

fn normalize_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case("[N/A]")
        || trimmed.eq_ignore_ascii_case("N/A")
    {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_mig_profile(value: &str) -> Option<String> {
    let normalized = normalize_value(value)?;
    if normalized.eq_ignore_ascii_case("disabled")
        || normalized.eq_ignore_ascii_case("not supported")
    {
        None
    } else {
        Some(normalized.to_ascii_lowercase())
    }
}

fn parse_yes_no(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "enabled" => Some(true),
        "no" | "disabled" => Some(false),
        _ => None,
    }
}

fn parse_enabled_disabled(value: &str) -> Option<bool> {
    parse_yes_no(value)
}

fn parse_memory_bytes(value: &str) -> Option<u64> {
    normalize_value(value)?
        .parse::<u64>()
        .ok()
        .map(|mebibytes| mebibytes * 1024 * 1024)
}

#[cfg(target_os = "linux")]
mod platform {
    use std::{
        ffi::{CStr, c_char, c_int, c_void},
        sync::Arc,
    };

    use libloading::Library;

    use super::{CudaCommandStatus, CudaCommandWait, QuantizationMode};
    use psionic_runtime::RuntimeError;

    type CudaError = i32;
    type CudaStream = *mut c_void;
    type CudaGraph = *mut c_void;
    type CudaGraphExec = *mut c_void;
    type CublasStatus = i32;
    type CublasHandle = *mut c_void;

    const CUDA_SUCCESS: CudaError = 0;
    const CUBLAS_STATUS_SUCCESS: CublasStatus = 0;
    const CUDA_MEMCPY_HOST_TO_DEVICE: c_int = 1;
    const CUDA_MEMCPY_DEVICE_TO_HOST: c_int = 2;
    const CUDA_MEMCPY_DEVICE_TO_DEVICE: c_int = 3;
    const CUBLAS_OP_N: c_int = 0;
    const CUDA_R_32F: c_int = 0;
    const CUDA_R_16F: c_int = 2;
    const CUDA_STREAM_CAPTURE_MODE_RELAXED: c_int = 2;
    const CUBLAS_COMPUTE_32F_FAST_16F: c_int = 74;
    const CUBLAS_GEMM_DEFAULT_TENSOR_OP: c_int = 99;

    type CudaGetErrorString = unsafe extern "C" fn(CudaError) -> *const c_char;
    type CudaSetDevice = unsafe extern "C" fn(c_int) -> CudaError;
    type CudaMalloc = unsafe extern "C" fn(*mut *mut c_void, usize) -> CudaError;
    type CudaMallocHost = unsafe extern "C" fn(*mut *mut c_void, usize) -> CudaError;
    type CudaFree = unsafe extern "C" fn(*mut c_void) -> CudaError;
    type CudaFreeHost = unsafe extern "C" fn(*mut c_void) -> CudaError;
    type CudaMemcpy = unsafe extern "C" fn(*mut c_void, *const c_void, usize, c_int) -> CudaError;
    type CudaMemcpyAsync =
        unsafe extern "C" fn(*mut c_void, *const c_void, usize, c_int, CudaStream) -> CudaError;
    type CudaMemsetAsync = unsafe extern "C" fn(*mut c_void, c_int, usize, CudaStream) -> CudaError;
    type CudaStreamCreate = unsafe extern "C" fn(*mut CudaStream) -> CudaError;
    type CudaStreamDestroy = unsafe extern "C" fn(CudaStream) -> CudaError;
    type CudaStreamSynchronize = unsafe extern "C" fn(CudaStream) -> CudaError;
    type CudaStreamBeginCapture = unsafe extern "C" fn(CudaStream, c_int) -> CudaError;
    type CudaStreamEndCapture = unsafe extern "C" fn(CudaStream, *mut CudaGraph) -> CudaError;
    type CudaGraphInstantiate =
        unsafe extern "C" fn(*mut CudaGraphExec, CudaGraph, u64) -> CudaError;
    type CudaGraphLaunch = unsafe extern "C" fn(CudaGraphExec, CudaStream) -> CudaError;
    type CudaGraphExecDestroy = unsafe extern "C" fn(CudaGraphExec) -> CudaError;
    type CudaGraphDestroy = unsafe extern "C" fn(CudaGraph) -> CudaError;
    type CublasCreate = unsafe extern "C" fn(*mut CublasHandle) -> CublasStatus;
    type CublasDestroy = unsafe extern "C" fn(CublasHandle) -> CublasStatus;
    type CublasSetStream = unsafe extern "C" fn(CublasHandle, CudaStream) -> CublasStatus;
    type CublasSgemm = unsafe extern "C" fn(
        CublasHandle,
        c_int,
        c_int,
        c_int,
        c_int,
        c_int,
        *const f32,
        *const f32,
        c_int,
        *const f32,
        c_int,
        *const f32,
        *mut f32,
        c_int,
    ) -> CublasStatus;
    type CublasSgeam = unsafe extern "C" fn(
        CublasHandle,
        c_int,
        c_int,
        c_int,
        c_int,
        *const f32,
        *const f32,
        c_int,
        *const f32,
        *const f32,
        c_int,
        *mut f32,
        c_int,
    ) -> CublasStatus;
    type CublasGemmEx = unsafe extern "C" fn(
        CublasHandle,
        c_int,
        c_int,
        c_int,
        c_int,
        c_int,
        *const c_void,
        *const c_void,
        c_int,
        c_int,
        *const c_void,
        c_int,
        c_int,
        *const c_void,
        *mut c_void,
        c_int,
        c_int,
        c_int,
        c_int,
    ) -> CublasStatus;

    type QuantizedMatvecKernel = unsafe extern "C" fn(
        *const c_void,
        c_int,
        c_int,
        c_int,
        *const c_void,
        *mut c_void,
        CudaStream,
    ) -> CudaError;
    type QuantizedMatvecQ81Kernel = unsafe extern "C" fn(
        *const c_void,
        c_int,
        c_int,
        c_int,
        *const c_void,
        *const c_void,
        *mut c_void,
        CudaStream,
    ) -> CudaError;
    type QuantizedMatvecQ81ArgmaxKernel = unsafe extern "C" fn(
        *const c_void,
        c_int,
        c_int,
        c_int,
        *const c_void,
        *const c_void,
        *mut c_void,
        CudaStream,
    ) -> CudaError;
    type QuantizeQ81Kernel =
        unsafe extern "C" fn(*const c_void, c_int, c_int, *mut c_void, CudaStream) -> CudaError;
    type DequantizeRowToF32Kernel = unsafe extern "C" fn(
        *const c_void,
        c_int,
        c_int,
        c_int,
        *const c_void,
        *mut c_void,
        CudaStream,
    ) -> CudaError;
    type GatherF16RowToF32Kernel = unsafe extern "C" fn(
        *const c_void,
        c_int,
        c_int,
        *const c_void,
        *mut c_void,
        CudaStream,
    ) -> CudaError;

    unsafe extern "C" {
        fn psionic_cuda_quantized_kernels_compiled() -> c_int;
        fn psionic_cuda_q8_0_matvec(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_mxfp4_matvec(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_quantize_q8_1(
            input: *const c_void,
            rows: c_int,
            cols: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q8_0_dequantize_row_to_f32(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            decode_params: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_mxfp4_dequantize_row_to_f32(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            decode_params: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_cast_f32_to_f16(
            input: *const c_void,
            element_count: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_gather_f16_row_to_f32(
            input: *const c_void,
            rows: c_int,
            cols: c_int,
            decode_params: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q8_0_matvec_q8_1(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_mxfp4_matvec_q8_1(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_q8_0_matvec_q8_1_argmax(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_mxfp4_matvec_q8_1_argmax(
            weights: *const c_void,
            rows: c_int,
            cols: c_int,
            row_stride: c_int,
            input_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_argmax_f32(
            input: *const c_void,
            rows: c_int,
            cols: c_int,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_rms_norm(
            input: *const c_void,
            weight: *const c_void,
            element_count: c_int,
            epsilon: f32,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_rms_norm_q8_1(
            input: *const c_void,
            weight: *const c_void,
            element_count: c_int,
            epsilon: f32,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_add_residual_rms_norm(
            input: *const c_void,
            residual: *const c_void,
            input_bias: *const c_void,
            weight: *const c_void,
            element_count: c_int,
            epsilon: f32,
            summed_output: *mut c_void,
            normalized_output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_add_residual_rms_norm_q8_1(
            input: *const c_void,
            residual: *const c_void,
            input_bias: *const c_void,
            weight: *const c_void,
            element_count: c_int,
            epsilon: f32,
            summed_output: *mut c_void,
            normalized_output: *mut c_void,
            quantized_output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_add_residual_rms_norm_q8_1_router_topk(
            input: *const c_void,
            residual: *const c_void,
            input_bias: *const c_void,
            weight: *const c_void,
            element_count: c_int,
            epsilon: f32,
            summed_output: *mut c_void,
            normalized_output: *mut c_void,
            quantized_output: *mut c_void,
            router_weights: *const c_void,
            router_bias: *const c_void,
            expert_count: c_int,
            top_k: c_int,
            selected_ids: *mut c_void,
            selected_weights: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_add_f32_offset_in_place(
            destination: *mut c_void,
            element_offset: c_int,
            rhs: *const c_void,
            element_count: c_int,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_rope_neox_in_place(
            values: *mut c_void,
            element_offset: c_int,
            head_count: c_int,
            head_dim: c_int,
            rotary_dim: c_int,
            position: c_int,
            freq_scale: f32,
            ext_factor: f32,
            corr_low: f32,
            corr_high: f32,
            theta_scale: f32,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_decode(
            query: *const c_void,
            query_offset: c_int,
            current_key: *const c_void,
            key_offset: c_int,
            current_value: *const c_void,
            value_offset: c_int,
            cache_keys: *const c_void,
            cache_values: *const c_void,
            cache_width: c_int,
            layer_offset: c_int,
            past_tokens: c_int,
            sliding_window: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            attention_sinks: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_decode_rope_cache(
            qkv: *const c_void,
            query_offset: c_int,
            key_offset: c_int,
            value_offset: c_int,
            cache_keys: *mut c_void,
            cache_values: *mut c_void,
            cache_width: c_int,
            layer_offset: c_int,
            past_tokens: c_int,
            sliding_window: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            rotary_dim: c_int,
            position: c_int,
            freq_scale: f32,
            ext_factor: f32,
            corr_low: f32,
            corr_high: f32,
            theta_scale: f32,
            attention_sinks: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_decode_rope_cache_f16_kv(
            qkv: *const c_void,
            query_offset: c_int,
            key_offset: c_int,
            value_offset: c_int,
            cache_keys: *mut c_void,
            cache_values: *mut c_void,
            cache_width: c_int,
            layer_offset: c_int,
            past_tokens: c_int,
            sliding_window: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            rotary_dim: c_int,
            position: c_int,
            freq_scale: f32,
            ext_factor: f32,
            corr_low: f32,
            corr_high: f32,
            theta_scale: f32,
            attention_sinks: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_decode_rope_cache_f16_kv_q8_1(
            qkv: *const c_void,
            query_offset: c_int,
            key_offset: c_int,
            value_offset: c_int,
            cache_keys: *mut c_void,
            cache_values: *mut c_void,
            cache_width: c_int,
            layer_offset: c_int,
            past_tokens: c_int,
            sliding_window: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            rotary_dim: c_int,
            position: c_int,
            freq_scale: f32,
            ext_factor: f32,
            corr_low: f32,
            corr_high: f32,
            theta_scale: f32,
            attention_sinks: *const c_void,
            output_q8_1: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_decode_rope_cache_f16_kv_graph(
            qkv: *const c_void,
            query_offset: c_int,
            key_offset: c_int,
            value_offset: c_int,
            cache_keys: *mut c_void,
            cache_values: *mut c_void,
            cache_width: c_int,
            layer_offset: c_int,
            decode_params: *const c_void,
            sliding_window: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            rotary_dim: c_int,
            freq_scale: f32,
            ext_factor: f32,
            corr_low: f32,
            corr_high: f32,
            theta_scale: f32,
            attention_sinks: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_attention_decode_rope_cache_f16_kv_graph_q8_1(
            qkv: *const c_void,
            query_offset: c_int,
            key_offset: c_int,
            value_offset: c_int,
            cache_keys: *mut c_void,
            cache_values: *mut c_void,
            cache_width: c_int,
            layer_offset: c_int,
            decode_params: *const c_void,
            sliding_window: c_int,
            head_count: c_int,
            kv_head_count: c_int,
            head_dim: c_int,
            rotary_dim: c_int,
            freq_scale: f32,
            ext_factor: f32,
            corr_low: f32,
            corr_high: f32,
            theta_scale: f32,
            attention_sinks: *const c_void,
            output_q8_1: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_router_topk_softmax(
            weights: *const c_void,
            bias: *const c_void,
            input: *const c_void,
            expert_count: c_int,
            input_size: c_int,
            top_k: c_int,
            selected_ids: *mut c_void,
            selected_weights: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_router_topk_delayed_softmax(
            logits: *const c_void,
            expert_count: c_int,
            top_k: c_int,
            selected_ids: *mut c_void,
            selected_weights: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_moe_gate_up_swiglu_q8_1(
            weights: *const c_void,
            mode: c_int,
            row_stride: c_int,
            rows_per_expert: c_int,
            columns: c_int,
            gate_rows: c_int,
            up_rows: c_int,
            selected_ids: *const c_void,
            selected_count: c_int,
            input_q8_1: *const c_void,
            gate_bias: *const c_void,
            up_bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_moe_gate_up_swiglu(
            weights: *const c_void,
            mode: c_int,
            row_stride: c_int,
            rows_per_expert: c_int,
            columns: c_int,
            gate_rows: c_int,
            up_rows: c_int,
            selected_ids: *const c_void,
            selected_count: c_int,
            input: *const c_void,
            gate_bias: *const c_void,
            up_bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_moe_gate_up_swiglu_q8_1_selected4_quantized(
            weights: *const c_void,
            mode: c_int,
            row_stride: c_int,
            rows_per_expert: c_int,
            columns: c_int,
            gate_rows: c_int,
            up_rows: c_int,
            selected_ids: *const c_void,
            selected_count: c_int,
            input_q8_1: *const c_void,
            gate_bias: *const c_void,
            up_bias: *const c_void,
            output_q8_1: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_moe_down_aggregate(
            weights: *const c_void,
            mode: c_int,
            row_stride: c_int,
            rows: c_int,
            columns: c_int,
            selected_ids: *const c_void,
            selected_weights: *const c_void,
            selected_count: c_int,
            activated: *const c_void,
            bias: *const c_void,
            residual: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_moe_down_aggregate_q8_1(
            weights: *const c_void,
            mode: c_int,
            row_stride: c_int,
            rows: c_int,
            columns: c_int,
            selected_ids: *const c_void,
            selected_weights: *const c_void,
            selected_count: c_int,
            activated_q8_1: *const c_void,
            bias: *const c_void,
            residual: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_moe_down_aggregate_q8_1_f32(
            weights: *const c_void,
            mode: c_int,
            row_stride: c_int,
            rows: c_int,
            columns: c_int,
            selected_ids: *const c_void,
            selected_weights: *const c_void,
            selected_count: c_int,
            activated: *const c_void,
            bias: *const c_void,
            residual: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_moe_down_project_q8_1_selected4(
            weights: *const c_void,
            mode: c_int,
            row_stride: c_int,
            rows: c_int,
            columns: c_int,
            selected_ids: *const c_void,
            selected_count: c_int,
            activated_q8_1: *const c_void,
            bias: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
        fn psionic_cuda_accumulate_selected4(
            input: *const c_void,
            selected_weights: *const c_void,
            selected_count: c_int,
            rows: c_int,
            residual: *const c_void,
            output: *mut c_void,
            stream: CudaStream,
        ) -> CudaError;
    }

    pub(super) struct ConfiguredBackend {
        runtime: Arc<CudaRuntime>,
        stream: CudaStream,
    }

    #[derive(Clone)]
    pub(super) struct PlatformBuffer {
        inner: Arc<PlatformBufferInner>,
    }

    #[derive(Clone)]
    pub(super) struct PlatformHostBuffer {
        inner: Arc<PlatformHostBufferInner>,
    }

    pub(super) struct PlatformGraphExec {
        runtime: Arc<CudaRuntime>,
        graph: CudaGraph,
        instance: CudaGraphExec,
        stream: CudaStream,
    }

    pub(super) struct PlatformSubmission {
        runtime: Arc<CudaRuntime>,
        stream: CudaStream,
        status: CudaCommandStatus,
        owned_stream: bool,
        capturing: bool,
    }

    struct PlatformBufferInner {
        runtime: Arc<CudaRuntime>,
        device_ptr: *mut c_void,
    }

    struct PlatformHostBufferInner {
        runtime: Arc<CudaRuntime>,
        host_ptr: *mut c_void,
        byte_len: usize,
    }

    struct CudaRuntime {
        ordinal: u16,
        _cudart_library: Library,
        _cublas_library: Library,
        cuda_get_error_string: CudaGetErrorString,
        cuda_set_device: CudaSetDevice,
        cuda_malloc: CudaMalloc,
        cuda_malloc_host: CudaMallocHost,
        cuda_free: CudaFree,
        cuda_free_host: CudaFreeHost,
        cuda_memcpy: CudaMemcpy,
        cuda_memcpy_async: CudaMemcpyAsync,
        cuda_memset_async: CudaMemsetAsync,
        cuda_stream_create: CudaStreamCreate,
        cuda_stream_destroy: CudaStreamDestroy,
        cuda_stream_synchronize: CudaStreamSynchronize,
        cuda_stream_begin_capture: CudaStreamBeginCapture,
        cuda_stream_end_capture: CudaStreamEndCapture,
        cuda_graph_instantiate: CudaGraphInstantiate,
        cuda_graph_launch: CudaGraphLaunch,
        cuda_graph_exec_destroy: CudaGraphExecDestroy,
        cuda_graph_destroy: CudaGraphDestroy,
        cublas_handle: CublasHandle,
        cublas_create: CublasCreate,
        cublas_destroy: CublasDestroy,
        cublas_set_stream: CublasSetStream,
        cublas_sgemm: CublasSgemm,
        cublas_sgeam: CublasSgeam,
        cublas_gemm_ex: CublasGemmEx,
    }

    impl ConfiguredBackend {
        pub(super) fn allocate(&self, byte_len: usize) -> Result<PlatformBuffer, RuntimeError> {
            self.runtime.set_device()?;
            let mut device_ptr = std::ptr::null_mut();
            let allocation_len = byte_len.max(1);
            self.runtime.check(
                unsafe { (self.runtime.cuda_malloc)(&mut device_ptr, allocation_len) },
                "cudaMalloc",
            )?;
            Ok(PlatformBuffer {
                inner: Arc::new(PlatformBufferInner {
                    runtime: Arc::clone(&self.runtime),
                    device_ptr,
                }),
            })
        }

        pub(super) fn allocate_host(
            &self,
            byte_len: usize,
        ) -> Result<PlatformHostBuffer, RuntimeError> {
            self.runtime.set_device()?;
            let mut host_ptr = std::ptr::null_mut();
            let allocation_len = byte_len.max(1);
            self.runtime.check(
                unsafe { (self.runtime.cuda_malloc_host)(&mut host_ptr, allocation_len) },
                "cudaMallocHost",
            )?;
            Ok(PlatformHostBuffer {
                inner: Arc::new(PlatformHostBufferInner {
                    runtime: Arc::clone(&self.runtime),
                    host_ptr,
                    byte_len: allocation_len,
                }),
            })
        }

        pub(super) fn begin_submission(&self) -> Result<PlatformSubmission, RuntimeError> {
            self.runtime.set_device()?;
            Ok(PlatformSubmission {
                runtime: Arc::clone(&self.runtime),
                stream: self.stream,
                status: CudaCommandStatus::Submitted,
                owned_stream: false,
                capturing: false,
            })
        }

        pub(super) fn begin_capture_submission(&self) -> Result<PlatformSubmission, RuntimeError> {
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (self.runtime.cuda_stream_begin_capture)(
                        self.stream,
                        CUDA_STREAM_CAPTURE_MODE_RELAXED,
                    )
                },
                "cudaStreamBeginCapture",
            )?;
            Ok(PlatformSubmission {
                runtime: Arc::clone(&self.runtime),
                stream: self.stream,
                status: CudaCommandStatus::Submitted,
                owned_stream: false,
                capturing: true,
            })
        }
    }

    pub(super) fn quantized_kernels_compiled() -> bool {
        unsafe { psionic_cuda_quantized_kernels_compiled() != 0 }
    }

    impl PlatformBuffer {
        pub(super) fn allocation_identity(&self) -> usize {
            self.inner.device_ptr as usize
        }

        pub(super) fn write_bytes(&self, bytes: &[u8]) -> Result<(), RuntimeError> {
            if bytes.is_empty() {
                return Ok(());
            }
            self.inner.runtime.set_device()?;
            self.inner.runtime.check(
                unsafe {
                    (self.inner.runtime.cuda_memcpy)(
                        self.inner.device_ptr,
                        bytes.as_ptr().cast(),
                        bytes.len(),
                        CUDA_MEMCPY_HOST_TO_DEVICE,
                    )
                },
                "cudaMemcpy host_to_device",
            )
        }

        pub(super) fn write_bytes_at_offset(
            &self,
            byte_offset: usize,
            bytes: &[u8],
        ) -> Result<(), RuntimeError> {
            if bytes.is_empty() {
                return Ok(());
            }
            self.inner.runtime.set_device()?;
            self.inner.runtime.check(
                unsafe {
                    (self.inner.runtime.cuda_memcpy)(
                        self.inner.device_ptr.cast::<u8>().add(byte_offset).cast(),
                        bytes.as_ptr().cast(),
                        bytes.len(),
                        CUDA_MEMCPY_HOST_TO_DEVICE,
                    )
                },
                "cudaMemcpy host_to_device_region",
            )
        }

        pub(super) fn read_bytes(&self, byte_len: usize) -> Result<Vec<u8>, RuntimeError> {
            if byte_len == 0 {
                return Ok(Vec::new());
            }
            self.inner.runtime.set_device()?;
            let mut bytes = vec![0u8; byte_len];
            self.inner.runtime.check(
                unsafe {
                    (self.inner.runtime.cuda_memcpy)(
                        bytes.as_mut_ptr().cast(),
                        self.inner.device_ptr,
                        byte_len,
                        CUDA_MEMCPY_DEVICE_TO_HOST,
                    )
                },
                "cudaMemcpy device_to_host",
            )?;
            Ok(bytes)
        }

        pub(super) fn read_bytes_at_offset(
            &self,
            byte_offset: usize,
            byte_len: usize,
        ) -> Result<Vec<u8>, RuntimeError> {
            if byte_len == 0 {
                return Ok(Vec::new());
            }
            self.inner.runtime.set_device()?;
            let mut bytes = vec![0u8; byte_len];
            self.inner.runtime.check(
                unsafe {
                    (self.inner.runtime.cuda_memcpy)(
                        bytes.as_mut_ptr().cast(),
                        self.inner.device_ptr.cast::<u8>().add(byte_offset).cast(),
                        byte_len,
                        CUDA_MEMCPY_DEVICE_TO_HOST,
                    )
                },
                "cudaMemcpy device_to_host_region",
            )?;
            Ok(bytes)
        }
    }

    impl PlatformHostBuffer {
        pub(super) fn write_bytes(&self, bytes: &[u8]) -> Result<(), RuntimeError> {
            if bytes.len() != self.inner.byte_len {
                return Err(RuntimeError::Backend(format!(
                    "cuda host buffer write length mismatch: expected {}, actual {}",
                    self.inner.byte_len,
                    bytes.len()
                )));
            }
            if bytes.is_empty() {
                return Ok(());
            }
            unsafe {
                std::ptr::copy_nonoverlapping(
                    bytes.as_ptr(),
                    self.inner.host_ptr.cast::<u8>(),
                    bytes.len(),
                );
            }
            Ok(())
        }

        pub(super) fn read_bytes(&self, byte_len: usize) -> Result<Vec<u8>, RuntimeError> {
            if byte_len != self.inner.byte_len {
                return Err(RuntimeError::Backend(format!(
                    "cuda host buffer read length mismatch: expected {}, actual {}",
                    self.inner.byte_len, byte_len
                )));
            }
            if byte_len == 0 {
                return Ok(Vec::new());
            }
            let mut bytes = vec![0_u8; byte_len];
            unsafe {
                std::ptr::copy_nonoverlapping(
                    self.inner.host_ptr.cast::<u8>(),
                    bytes.as_mut_ptr(),
                    byte_len,
                );
            }
            Ok(bytes)
        }
    }

    impl PlatformSubmission {
        pub(super) fn fill_buffer(
            &mut self,
            buffer: &PlatformBuffer,
            byte_len: usize,
            value: u8,
        ) -> Result<(), RuntimeError> {
            if byte_len == 0 {
                return Ok(());
            }
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (self.runtime.cuda_memset_async)(
                        buffer.inner.device_ptr,
                        i32::from(value),
                        byte_len,
                        self.stream,
                    )
                },
                "cudaMemsetAsync",
            )
        }

        pub(super) fn copy_buffer(
            &mut self,
            source: &PlatformBuffer,
            destination: &PlatformBuffer,
            byte_len: usize,
        ) -> Result<(), RuntimeError> {
            if byte_len == 0 {
                return Ok(());
            }
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (self.runtime.cuda_memcpy_async)(
                        destination.inner.device_ptr,
                        source.inner.device_ptr,
                        byte_len,
                        CUDA_MEMCPY_DEVICE_TO_DEVICE,
                        self.stream,
                    )
                },
                "cudaMemcpyAsync device_to_device",
            )
        }

        pub(super) fn copy_host_to_device(
            &mut self,
            source: &PlatformHostBuffer,
            destination: &PlatformBuffer,
            byte_len: usize,
        ) -> Result<(), RuntimeError> {
            if byte_len == 0 {
                return Ok(());
            }
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (self.runtime.cuda_memcpy_async)(
                        destination.inner.device_ptr,
                        source.inner.host_ptr,
                        byte_len,
                        CUDA_MEMCPY_HOST_TO_DEVICE,
                        self.stream,
                    )
                },
                "cudaMemcpyAsync host_to_device",
            )
        }

        pub(super) fn copy_device_to_host(
            &mut self,
            source: &PlatformBuffer,
            destination: &PlatformHostBuffer,
            byte_len: usize,
        ) -> Result<(), RuntimeError> {
            if byte_len == 0 {
                return Ok(());
            }
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (self.runtime.cuda_memcpy_async)(
                        destination.inner.host_ptr,
                        source.inner.device_ptr,
                        byte_len,
                        CUDA_MEMCPY_DEVICE_TO_HOST,
                        self.stream,
                    )
                },
                "cudaMemcpyAsync device_to_host",
            )
        }

        pub(super) fn copy_buffer_region(
            &mut self,
            source: &PlatformBuffer,
            source_byte_offset: usize,
            destination: &PlatformBuffer,
            destination_byte_offset: usize,
            byte_len: usize,
        ) -> Result<(), RuntimeError> {
            if byte_len == 0 {
                return Ok(());
            }
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (self.runtime.cuda_memcpy_async)(
                        destination
                            .inner
                            .device_ptr
                            .cast::<u8>()
                            .add(destination_byte_offset)
                            .cast(),
                        source
                            .inner
                            .device_ptr
                            .cast::<u8>()
                            .add(source_byte_offset)
                            .cast(),
                        byte_len,
                        CUDA_MEMCPY_DEVICE_TO_DEVICE,
                        self.stream,
                    )
                },
                "cudaMemcpyAsync device_to_device_region",
            )
        }

        pub(super) fn encode_add(
            &mut self,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            if element_count == 0 {
                return Ok(());
            }
            let m = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda add element count exceeds cublas limits"))
            })?;
            let alpha = 1.0_f32;
            let beta = 1.0_f32;
            self.runtime.bind_stream(self.stream)?;
            self.runtime.check_cublas(
                unsafe {
                    (self.runtime.cublas_sgeam)(
                        self.runtime.cublas_handle,
                        CUBLAS_OP_N,
                        CUBLAS_OP_N,
                        m,
                        1,
                        &alpha,
                        left.inner.device_ptr.cast(),
                        m,
                        &beta,
                        right.inner.device_ptr.cast(),
                        m,
                        output.inner.device_ptr.cast(),
                        m,
                    )
                },
                "cublasSgeam",
            )
        }

        pub(super) fn encode_matmul(
            &mut self,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
            rows: usize,
            inner: usize,
            cols: usize,
        ) -> Result<(), RuntimeError> {
            if rows == 0 || inner == 0 || cols == 0 {
                return Ok(());
            }
            let m = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda matmul column count exceeds cublas limits",
                ))
            })?;
            let n = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda matmul row count exceeds cublas limits"))
            })?;
            let k = c_int::try_from(inner).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda matmul inner dimension exceeds cublas limits",
                ))
            })?;
            let alpha = 1.0_f32;
            let beta = 0.0_f32;
            self.runtime.bind_stream(self.stream)?;
            self.runtime.check_cublas(
                unsafe {
                    (self.runtime.cublas_sgemm)(
                        self.runtime.cublas_handle,
                        CUBLAS_OP_N,
                        CUBLAS_OP_N,
                        m,
                        n,
                        k,
                        &alpha,
                        right.inner.device_ptr.cast(),
                        m,
                        left.inner.device_ptr.cast(),
                        k,
                        &beta,
                        output.inner.device_ptr.cast(),
                        m,
                    )
                },
                "cublasSgemm_v2",
            )
        }

        pub(super) fn encode_matmul_f16_to_f32(
            &mut self,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
            rows: usize,
            inner: usize,
            cols: usize,
        ) -> Result<(), RuntimeError> {
            if rows == 0 || inner == 0 || cols == 0 {
                return Ok(());
            }
            let m = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda mixed matmul column count exceeds cublas limits",
                ))
            })?;
            let n = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda mixed matmul row count exceeds cublas limits",
                ))
            })?;
            let k = c_int::try_from(inner).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda mixed matmul inner dimension exceeds cublas limits",
                ))
            })?;
            let alpha = 1.0_f32;
            let beta = 0.0_f32;
            self.runtime.bind_stream(self.stream)?;
            self.runtime.check_cublas(
                unsafe {
                    (self.runtime.cublas_gemm_ex)(
                        self.runtime.cublas_handle,
                        CUBLAS_OP_N,
                        CUBLAS_OP_N,
                        m,
                        n,
                        k,
                        (&alpha as *const f32).cast(),
                        right.inner.device_ptr,
                        CUDA_R_16F,
                        m,
                        left.inner.device_ptr,
                        CUDA_R_16F,
                        k,
                        (&beta as *const f32).cast(),
                        output.inner.device_ptr,
                        CUDA_R_32F,
                        m,
                        CUBLAS_COMPUTE_32F_FAST_16F,
                        CUBLAS_GEMM_DEFAULT_TENSOR_OP,
                    )
                },
                "cublasGemmEx",
            )
        }

        pub(super) fn encode_cast_f32_to_f16(
            &mut self,
            input: &PlatformBuffer,
            output: &PlatformBuffer,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda f32->f16 cast length exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_cast_f32_to_f16(
                        input.inner.device_ptr.cast(),
                        element_count,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_cast_f32_to_f16",
            )
        }

        pub(super) fn encode_quantized_matvec(
            &mut self,
            weights: &PlatformBuffer,
            byte_offset: usize,
            mode: QuantizationMode,
            rows: usize,
            cols: usize,
            input: &PlatformBuffer,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            if !quantized_kernels_compiled() {
                return Err(RuntimeError::Backend(String::from(
                    "cuda quantized text-generation kernels are not available in this build",
                )));
            }
            let Some((elements_per_block, bytes_per_block)) = mode.ggml_block_spec() else {
                return Err(RuntimeError::Backend(format!(
                    "cuda quantized matvec does not support mode {mode:?}"
                )));
            };
            let row_stride = (cols / elements_per_block) * bytes_per_block;
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda quantized matvec rows exceed c_int"))
            })?;
            let cols = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from("cuda quantized matvec cols exceed c_int"))
            })?;
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized matvec row stride exceeds c_int",
                ))
            })?;
            let kernel: QuantizedMatvecKernel = match mode {
                QuantizationMode::GgmlQ8_0 => psionic_cuda_q8_0_matvec,
                QuantizationMode::GgmlMxfp4 => psionic_cuda_mxfp4_matvec,
                _ => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda quantized matvec does not support mode {mode:?}"
                    )));
                }
            };
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    kernel(
                        weights
                            .inner
                            .device_ptr
                            .cast::<u8>()
                            .add(byte_offset)
                            .cast(),
                        rows,
                        cols,
                        row_stride,
                        input.inner.device_ptr,
                        output.inner.device_ptr,
                        self.stream,
                    )
                },
                "psionic_cuda_quantized_matvec",
            )
        }

        pub(super) fn encode_quantize_f32_to_q8_1(
            &mut self,
            input: &PlatformBuffer,
            rows: usize,
            cols: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda q8_1 quantize rows exceed c_int"))
            })?;
            let cols = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from("cuda q8_1 quantize cols exceed c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (psionic_cuda_quantize_q8_1 as QuantizeQ81Kernel)(
                        input.inner.device_ptr.cast(),
                        rows,
                        cols,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_quantize_q8_1",
            )
        }

        pub(super) fn encode_dequantize_row_to_f32(
            &mut self,
            weights: &PlatformBuffer,
            mode: QuantizationMode,
            rows: usize,
            row_stride: usize,
            cols: usize,
            decode_params: &PlatformBuffer,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            if !quantized_kernels_compiled() {
                return Err(RuntimeError::Backend(String::from(
                    "cuda quantized text-generation kernels are not available in this build",
                )));
            }
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda dequantize row rows exceed c_int"))
            })?;
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from("cuda dequantize row stride exceeds c_int"))
            })?;
            let cols = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from("cuda dequantize row cols exceed c_int"))
            })?;
            let kernel: DequantizeRowToF32Kernel = match mode {
                QuantizationMode::GgmlQ8_0 => psionic_cuda_q8_0_dequantize_row_to_f32,
                QuantizationMode::GgmlMxfp4 => psionic_cuda_mxfp4_dequantize_row_to_f32,
                _ => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda dequantize row does not support mode {mode:?}"
                    )));
                }
            };
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    kernel(
                        weights.inner.device_ptr.cast(),
                        rows,
                        cols,
                        row_stride,
                        decode_params.inner.device_ptr.cast(),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_dequantize_row_to_f32",
            )
        }

        pub(super) fn encode_gather_f16_row_to_f32(
            &mut self,
            input: &PlatformBuffer,
            rows: usize,
            cols: usize,
            decode_params: &PlatformBuffer,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda gather f16 row rows exceed c_int"))
            })?;
            let cols = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from("cuda gather f16 row cols exceed c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    (psionic_cuda_gather_f16_row_to_f32 as GatherF16RowToF32Kernel)(
                        input.inner.device_ptr.cast(),
                        rows,
                        cols,
                        decode_params.inner.device_ptr.cast(),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_gather_f16_row_to_f32",
            )
        }

        pub(super) fn encode_quantized_matvec_q8_1(
            &mut self,
            weights: &PlatformBuffer,
            byte_offset: usize,
            mode: QuantizationMode,
            rows: usize,
            cols: usize,
            input_q8_1: &PlatformBuffer,
            bias: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            if !quantized_kernels_compiled() {
                return Err(RuntimeError::Backend(String::from(
                    "cuda quantized text-generation kernels are not available in this build",
                )));
            }
            let Some((elements_per_block, bytes_per_block)) = mode.ggml_block_spec() else {
                return Err(RuntimeError::Backend(format!(
                    "cuda quantized matvec does not support mode {mode:?}"
                )));
            };
            let row_stride = (cols / elements_per_block) * bytes_per_block;
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda quantized matvec rows exceed c_int"))
            })?;
            let cols = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from("cuda quantized matvec cols exceed c_int"))
            })?;
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized matvec row stride exceeds c_int",
                ))
            })?;
            let kernel: QuantizedMatvecQ81Kernel = match mode {
                QuantizationMode::GgmlQ8_0 => psionic_cuda_q8_0_matvec_q8_1,
                QuantizationMode::GgmlMxfp4 => psionic_cuda_mxfp4_matvec_q8_1,
                _ => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda quantized matvec does not support mode {mode:?}"
                    )));
                }
            };
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    kernel(
                        weights
                            .inner
                            .device_ptr
                            .cast::<u8>()
                            .add(byte_offset)
                            .cast(),
                        rows,
                        cols,
                        row_stride,
                        input_q8_1.inner.device_ptr.cast(),
                        bias.map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_quantized_matvec_q8_1",
            )
        }

        pub(super) fn encode_quantized_matvec_q8_1_argmax(
            &mut self,
            weights: &PlatformBuffer,
            byte_offset: usize,
            mode: QuantizationMode,
            rows: usize,
            cols: usize,
            input_q8_1: &PlatformBuffer,
            bias: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            if !quantized_kernels_compiled() {
                return Err(RuntimeError::Backend(String::from(
                    "cuda quantized text-generation kernels are not available in this build",
                )));
            }
            let Some((elements_per_block, bytes_per_block)) = mode.ggml_block_spec() else {
                return Err(RuntimeError::Backend(format!(
                    "cuda quantized matvec argmax does not support mode {mode:?}"
                )));
            };
            let row_stride = (cols / elements_per_block) * bytes_per_block;
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized matvec argmax rows exceed c_int",
                ))
            })?;
            let cols = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized matvec argmax cols exceed c_int",
                ))
            })?;
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized matvec argmax row stride exceeds c_int",
                ))
            })?;
            let kernel: QuantizedMatvecQ81ArgmaxKernel = match mode {
                QuantizationMode::GgmlQ8_0 => psionic_cuda_q8_0_matvec_q8_1_argmax,
                QuantizationMode::GgmlMxfp4 => psionic_cuda_mxfp4_matvec_q8_1_argmax,
                _ => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda quantized matvec argmax does not support mode {mode:?}"
                    )));
                }
            };
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    kernel(
                        weights
                            .inner
                            .device_ptr
                            .cast::<u8>()
                            .add(byte_offset)
                            .cast(),
                        rows,
                        cols,
                        row_stride,
                        input_q8_1.inner.device_ptr.cast(),
                        bias.map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_quantized_matvec_q8_1_argmax",
            )
        }

        pub(super) fn encode_argmax_f32(
            &mut self,
            input: &PlatformBuffer,
            rows: usize,
            cols: usize,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda argmax rows exceed c_int"))
            })?;
            let cols = c_int::try_from(cols).map_err(|_| {
                RuntimeError::Backend(String::from("cuda argmax cols exceed c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_argmax_f32(
                        input.inner.device_ptr.cast(),
                        rows,
                        cols,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_argmax_f32",
            )
        }

        pub(super) fn encode_rms_norm(
            &mut self,
            input: &PlatformBuffer,
            weight: &PlatformBuffer,
            output: &PlatformBuffer,
            element_count: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda rms_norm element count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_rms_norm(
                        input.inner.device_ptr.cast(),
                        weight.inner.device_ptr.cast(),
                        element_count,
                        epsilon,
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_rms_norm",
            )
        }

        pub(super) fn encode_rms_norm_q8_1(
            &mut self,
            input: &PlatformBuffer,
            weight: &PlatformBuffer,
            output_q8_1: &PlatformBuffer,
            element_count: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda rms_norm_q8_1 element count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_rms_norm_q8_1(
                        input.inner.device_ptr.cast(),
                        weight.inner.device_ptr.cast(),
                        element_count,
                        epsilon,
                        output_q8_1.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_rms_norm_q8_1",
            )
        }

        pub(super) fn encode_add_residual_rms_norm(
            &mut self,
            input: &PlatformBuffer,
            residual: &PlatformBuffer,
            input_bias: Option<&PlatformBuffer>,
            weight: &PlatformBuffer,
            summed_output: &PlatformBuffer,
            normalized_output: &PlatformBuffer,
            element_count: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda add_residual_rms_norm element count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_add_residual_rms_norm(
                        input.inner.device_ptr.cast(),
                        residual.inner.device_ptr.cast(),
                        input_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        weight.inner.device_ptr.cast(),
                        element_count,
                        epsilon,
                        summed_output.inner.device_ptr.cast(),
                        normalized_output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_add_residual_rms_norm",
            )
        }

        pub(super) fn encode_add_residual_rms_norm_q8_1(
            &mut self,
            input: &PlatformBuffer,
            residual: &PlatformBuffer,
            input_bias: Option<&PlatformBuffer>,
            weight: &PlatformBuffer,
            summed_output: &PlatformBuffer,
            normalized_output: &PlatformBuffer,
            quantized_output: &PlatformBuffer,
            element_count: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda add_residual_rms_norm_q8_1 element count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_add_residual_rms_norm_q8_1(
                        input.inner.device_ptr.cast(),
                        residual.inner.device_ptr.cast(),
                        input_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        weight.inner.device_ptr.cast(),
                        element_count,
                        epsilon,
                        summed_output.inner.device_ptr.cast(),
                        normalized_output.inner.device_ptr.cast(),
                        quantized_output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_add_residual_rms_norm_q8_1",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_add_residual_rms_norm_q8_1_router_topk(
            &mut self,
            input: &PlatformBuffer,
            residual: &PlatformBuffer,
            input_bias: Option<&PlatformBuffer>,
            weight: &PlatformBuffer,
            summed_output: &PlatformBuffer,
            normalized_output: &PlatformBuffer,
            quantized_output: &PlatformBuffer,
            router_weights: &PlatformBuffer,
            router_bias: Option<&PlatformBuffer>,
            expert_count: usize,
            top_k: usize,
            selected_ids: &PlatformBuffer,
            selected_weights: &PlatformBuffer,
            element_count: usize,
            epsilon: f32,
        ) -> Result<(), RuntimeError> {
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda add_residual_rms_norm_q8_1_router_topk element count exceeds c_int",
                ))
            })?;
            let expert_count = c_int::try_from(expert_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda add_residual_rms_norm_q8_1_router_topk expert count exceeds c_int",
                ))
            })?;
            let top_k = c_int::try_from(top_k).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda add_residual_rms_norm_q8_1_router_topk top-k exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_add_residual_rms_norm_q8_1_router_topk(
                        input.inner.device_ptr.cast(),
                        residual.inner.device_ptr.cast(),
                        input_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        weight.inner.device_ptr.cast(),
                        element_count,
                        epsilon,
                        summed_output.inner.device_ptr.cast(),
                        normalized_output.inner.device_ptr.cast(),
                        quantized_output.inner.device_ptr.cast(),
                        router_weights.inner.device_ptr.cast(),
                        router_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        expert_count,
                        top_k,
                        selected_ids.inner.device_ptr.cast(),
                        selected_weights.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_add_residual_rms_norm_q8_1_router_topk",
            )
        }

        pub(super) fn encode_add_f32_in_place(
            &mut self,
            destination: &PlatformBuffer,
            element_offset: usize,
            rhs: &PlatformBuffer,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            let element_offset = c_int::try_from(element_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda add_f32_in_place element offset exceeds c_int",
                ))
            })?;
            let element_count = c_int::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda add_f32_in_place element count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_add_f32_offset_in_place(
                        destination.inner.device_ptr.cast(),
                        element_offset,
                        rhs.inner.device_ptr.cast(),
                        element_count,
                        self.stream,
                    )
                },
                "psionic_cuda_add_f32_offset_in_place",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_rope_neox_in_place(
            &mut self,
            values: &PlatformBuffer,
            element_offset: usize,
            head_count: usize,
            head_dim: usize,
            rotary_dim: usize,
            position: usize,
            freq_scale: f32,
            ext_factor: f32,
            corr_dims: [f32; 2],
            theta_scale: f32,
        ) -> Result<(), RuntimeError> {
            let element_offset = c_int::try_from(element_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda rope_neox element offset exceeds c_int"))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda rope_neox head count exceeds c_int"))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from("cuda rope_neox head dim exceeds c_int"))
            })?;
            let rotary_dim = c_int::try_from(rotary_dim).map_err(|_| {
                RuntimeError::Backend(String::from("cuda rope_neox rotary dim exceeds c_int"))
            })?;
            let position = c_int::try_from(position).map_err(|_| {
                RuntimeError::Backend(String::from("cuda rope_neox position exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_rope_neox_in_place(
                        values.inner.device_ptr.cast(),
                        element_offset,
                        head_count,
                        head_dim,
                        rotary_dim,
                        position,
                        freq_scale,
                        ext_factor,
                        corr_dims[0],
                        corr_dims[1],
                        theta_scale,
                        self.stream,
                    )
                },
                "psionic_cuda_rope_neox_in_place",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache(
            &mut self,
            qkv: &PlatformBuffer,
            query_offset: usize,
            key_offset: usize,
            value_offset: usize,
            cache_keys: &PlatformBuffer,
            cache_values: &PlatformBuffer,
            cache_width: usize,
            layer_offset: usize,
            past_tokens: usize,
            sliding_window: usize,
            head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            rotary_dim: usize,
            position: usize,
            freq_scale: f32,
            ext_factor: f32,
            corr_dims: [f32; 2],
            theta_scale: f32,
            attention_sinks: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let query_offset = c_int::try_from(query_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention query offset exceeds c_int",
                ))
            })?;
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention key offset exceeds c_int",
                ))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention value offset exceeds c_int",
                ))
            })?;
            let cache_width = c_int::try_from(cache_width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention cache width exceeds c_int",
                ))
            })?;
            let layer_offset = c_int::try_from(layer_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention layer offset exceeds c_int",
                ))
            })?;
            let past_tokens = c_int::try_from(past_tokens).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention past token count exceeds c_int",
                ))
            })?;
            let sliding_window = c_int::try_from(sliding_window).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention sliding window exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention kv head count exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from("cuda fused attention head dim exceeds c_int"))
            })?;
            let rotary_dim = c_int::try_from(rotary_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention rotary dim exceeds c_int",
                ))
            })?;
            let position = c_int::try_from(position).map_err(|_| {
                RuntimeError::Backend(String::from("cuda fused attention position exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_decode_rope_cache(
                        qkv.inner.device_ptr.cast(),
                        query_offset,
                        key_offset,
                        value_offset,
                        cache_keys.inner.device_ptr.cast(),
                        cache_values.inner.device_ptr.cast(),
                        cache_width,
                        layer_offset,
                        past_tokens,
                        sliding_window,
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        position,
                        freq_scale,
                        ext_factor,
                        corr_dims[0],
                        corr_dims[1],
                        theta_scale,
                        attention_sinks
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_decode_rope_cache",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv(
            &mut self,
            qkv: &PlatformBuffer,
            query_offset: usize,
            key_offset: usize,
            value_offset: usize,
            cache_keys: &PlatformBuffer,
            cache_values: &PlatformBuffer,
            cache_width: usize,
            layer_offset: usize,
            past_tokens: usize,
            sliding_window: usize,
            head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            rotary_dim: usize,
            position: usize,
            freq_scale: f32,
            ext_factor: f32,
            corr_dims: [f32; 2],
            theta_scale: f32,
            attention_sinks: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let query_offset = c_int::try_from(query_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) query offset exceeds c_int",
                ))
            })?;
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) key offset exceeds c_int",
                ))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) value offset exceeds c_int",
                ))
            })?;
            let cache_width = c_int::try_from(cache_width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) cache width exceeds c_int",
                ))
            })?;
            let layer_offset = c_int::try_from(layer_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) layer offset exceeds c_int",
                ))
            })?;
            let past_tokens = c_int::try_from(past_tokens).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) past token count exceeds c_int",
                ))
            })?;
            let sliding_window = c_int::try_from(sliding_window).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) sliding window exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) kv head count exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) head dim exceeds c_int",
                ))
            })?;
            let rotary_dim = c_int::try_from(rotary_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) rotary dim exceeds c_int",
                ))
            })?;
            let position = c_int::try_from(position).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv) position exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_decode_rope_cache_f16_kv(
                        qkv.inner.device_ptr.cast(),
                        query_offset,
                        key_offset,
                        value_offset,
                        cache_keys.inner.device_ptr.cast(),
                        cache_values.inner.device_ptr.cast(),
                        cache_width,
                        layer_offset,
                        past_tokens,
                        sliding_window,
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        position,
                        freq_scale,
                        ext_factor,
                        corr_dims[0],
                        corr_dims[1],
                        theta_scale,
                        attention_sinks
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_decode_rope_cache_f16_kv",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv_q8_1(
            &mut self,
            qkv: &PlatformBuffer,
            query_offset: usize,
            key_offset: usize,
            value_offset: usize,
            cache_keys: &PlatformBuffer,
            cache_values: &PlatformBuffer,
            cache_width: usize,
            layer_offset: usize,
            past_tokens: usize,
            sliding_window: usize,
            head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            rotary_dim: usize,
            position: usize,
            freq_scale: f32,
            ext_factor: f32,
            corr_dims: [f32; 2],
            theta_scale: f32,
            attention_sinks: Option<&PlatformBuffer>,
            output_q8_1: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let query_offset = c_int::try_from(query_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) query offset exceeds c_int",
                ))
            })?;
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) key offset exceeds c_int",
                ))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) value offset exceeds c_int",
                ))
            })?;
            let cache_width = c_int::try_from(cache_width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) cache width exceeds c_int",
                ))
            })?;
            let layer_offset = c_int::try_from(layer_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) layer offset exceeds c_int",
                ))
            })?;
            let past_tokens = c_int::try_from(past_tokens).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) past token count exceeds c_int",
                ))
            })?;
            let sliding_window = c_int::try_from(sliding_window).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) sliding window exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) kv head count exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) head dim exceeds c_int",
                ))
            })?;
            let rotary_dim = c_int::try_from(rotary_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) rotary dim exceeds c_int",
                ))
            })?;
            let position = c_int::try_from(position).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(f16 kv, q8_1) position exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_decode_rope_cache_f16_kv_q8_1(
                        qkv.inner.device_ptr.cast(),
                        query_offset,
                        key_offset,
                        value_offset,
                        cache_keys.inner.device_ptr.cast(),
                        cache_values.inner.device_ptr.cast(),
                        cache_width,
                        layer_offset,
                        past_tokens,
                        sliding_window,
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        position,
                        freq_scale,
                        ext_factor,
                        corr_dims[0],
                        corr_dims[1],
                        theta_scale,
                        attention_sinks
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output_q8_1.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_decode_rope_cache_f16_kv_q8_1",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv_graph(
            &mut self,
            qkv: &PlatformBuffer,
            query_offset: usize,
            key_offset: usize,
            value_offset: usize,
            cache_keys: &PlatformBuffer,
            cache_values: &PlatformBuffer,
            cache_width: usize,
            layer_offset: usize,
            decode_params: &PlatformBuffer,
            sliding_window: usize,
            head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            rotary_dim: usize,
            freq_scale: f32,
            ext_factor: f32,
            corr_dims: [f32; 2],
            theta_scale: f32,
            attention_sinks: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let query_offset = c_int::try_from(query_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) query offset exceeds c_int",
                ))
            })?;
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) key offset exceeds c_int",
                ))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) value offset exceeds c_int",
                ))
            })?;
            let cache_width = c_int::try_from(cache_width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) cache width exceeds c_int",
                ))
            })?;
            let layer_offset = c_int::try_from(layer_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) layer offset exceeds c_int",
                ))
            })?;
            let sliding_window = c_int::try_from(sliding_window).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) sliding window exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) kv head count exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) head dim exceeds c_int",
                ))
            })?;
            let rotary_dim = c_int::try_from(rotary_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv) rotary dim exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_decode_rope_cache_f16_kv_graph(
                        qkv.inner.device_ptr.cast(),
                        query_offset,
                        key_offset,
                        value_offset,
                        cache_keys.inner.device_ptr.cast(),
                        cache_values.inner.device_ptr.cast(),
                        cache_width,
                        layer_offset,
                        decode_params.inner.device_ptr.cast(),
                        sliding_window,
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        freq_scale,
                        ext_factor,
                        corr_dims[0],
                        corr_dims[1],
                        theta_scale,
                        attention_sinks
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_decode_rope_cache_f16_kv_graph",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv_graph_q8_1(
            &mut self,
            qkv: &PlatformBuffer,
            query_offset: usize,
            key_offset: usize,
            value_offset: usize,
            cache_keys: &PlatformBuffer,
            cache_values: &PlatformBuffer,
            cache_width: usize,
            layer_offset: usize,
            decode_params: &PlatformBuffer,
            sliding_window: usize,
            head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            rotary_dim: usize,
            freq_scale: f32,
            ext_factor: f32,
            corr_dims: [f32; 2],
            theta_scale: f32,
            attention_sinks: Option<&PlatformBuffer>,
            output_q8_1: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let query_offset = c_int::try_from(query_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) query offset exceeds c_int",
                ))
            })?;
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) key offset exceeds c_int",
                ))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) value offset exceeds c_int",
                ))
            })?;
            let cache_width = c_int::try_from(cache_width).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) cache width exceeds c_int",
                ))
            })?;
            let layer_offset = c_int::try_from(layer_offset).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) layer offset exceeds c_int",
                ))
            })?;
            let sliding_window = c_int::try_from(sliding_window).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) sliding window exceeds c_int",
                ))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) head count exceeds c_int",
                ))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) kv head count exceeds c_int",
                ))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) head dim exceeds c_int",
                ))
            })?;
            let rotary_dim = c_int::try_from(rotary_dim).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda fused attention(graph, f16 kv, q8_1) rotary dim exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_decode_rope_cache_f16_kv_graph_q8_1(
                        qkv.inner.device_ptr.cast(),
                        query_offset,
                        key_offset,
                        value_offset,
                        cache_keys.inner.device_ptr.cast(),
                        cache_values.inner.device_ptr.cast(),
                        cache_width,
                        layer_offset,
                        decode_params.inner.device_ptr.cast(),
                        sliding_window,
                        head_count,
                        kv_head_count,
                        head_dim,
                        rotary_dim,
                        freq_scale,
                        ext_factor,
                        corr_dims[0],
                        corr_dims[1],
                        theta_scale,
                        attention_sinks
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output_q8_1.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_decode_rope_cache_f16_kv_graph_q8_1",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode(
            &mut self,
            query: &PlatformBuffer,
            query_offset: usize,
            current_key: &PlatformBuffer,
            key_offset: usize,
            current_value: &PlatformBuffer,
            value_offset: usize,
            cache_keys: &PlatformBuffer,
            cache_values: &PlatformBuffer,
            cache_width: usize,
            layer_offset: usize,
            past_tokens: usize,
            sliding_window: usize,
            head_count: usize,
            kv_head_count: usize,
            head_dim: usize,
            attention_sinks: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let query_offset = c_int::try_from(query_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention query offset exceeds c_int"))
            })?;
            let key_offset = c_int::try_from(key_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention key offset exceeds c_int"))
            })?;
            let value_offset = c_int::try_from(value_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention value offset exceeds c_int"))
            })?;
            let cache_width = c_int::try_from(cache_width).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention cache width exceeds c_int"))
            })?;
            let layer_offset = c_int::try_from(layer_offset).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention layer offset exceeds c_int"))
            })?;
            let past_tokens = c_int::try_from(past_tokens).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda attention past token count exceeds c_int",
                ))
            })?;
            let sliding_window = c_int::try_from(sliding_window).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention sliding window exceeds c_int"))
            })?;
            let head_count = c_int::try_from(head_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention head count exceeds c_int"))
            })?;
            let kv_head_count = c_int::try_from(kv_head_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention kv head count exceeds c_int"))
            })?;
            let head_dim = c_int::try_from(head_dim).map_err(|_| {
                RuntimeError::Backend(String::from("cuda attention head dim exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_attention_decode(
                        query.inner.device_ptr.cast(),
                        query_offset,
                        current_key.inner.device_ptr.cast(),
                        key_offset,
                        current_value.inner.device_ptr.cast(),
                        value_offset,
                        cache_keys.inner.device_ptr.cast(),
                        cache_values.inner.device_ptr.cast(),
                        cache_width,
                        layer_offset,
                        past_tokens,
                        sliding_window,
                        head_count,
                        kv_head_count,
                        head_dim,
                        attention_sinks
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_attention_decode",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_router_topk_softmax(
            &mut self,
            weights: &PlatformBuffer,
            bias: Option<&PlatformBuffer>,
            input: &PlatformBuffer,
            expert_count: usize,
            input_size: usize,
            top_k: usize,
            selected_ids: &PlatformBuffer,
            selected_weights: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let expert_count = c_int::try_from(expert_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda router expert count exceeds c_int"))
            })?;
            let input_size = c_int::try_from(input_size).map_err(|_| {
                RuntimeError::Backend(String::from("cuda router input size exceeds c_int"))
            })?;
            let top_k = c_int::try_from(top_k).map_err(|_| {
                RuntimeError::Backend(String::from("cuda router top-k exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_router_topk_softmax(
                        weights.inner.device_ptr.cast(),
                        bias.map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        input.inner.device_ptr.cast(),
                        expert_count,
                        input_size,
                        top_k,
                        selected_ids.inner.device_ptr.cast(),
                        selected_weights.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_router_topk_softmax",
            )
        }

        pub(super) fn encode_router_topk_delayed_softmax(
            &mut self,
            logits: &PlatformBuffer,
            expert_count: usize,
            top_k: usize,
            selected_ids: &PlatformBuffer,
            selected_weights: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let expert_count = c_int::try_from(expert_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda router expert count exceeds c_int"))
            })?;
            let top_k = c_int::try_from(top_k).map_err(|_| {
                RuntimeError::Backend(String::from("cuda router top-k exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_router_topk_delayed_softmax(
                        logits.inner.device_ptr.cast(),
                        expert_count,
                        top_k,
                        selected_ids.inner.device_ptr.cast(),
                        selected_weights.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_router_topk_delayed_softmax",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_gate_up_swiglu(
            &mut self,
            weights: &PlatformBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows_per_expert: usize,
            columns: usize,
            gate_rows: usize,
            up_rows: usize,
            selected_ids: &PlatformBuffer,
            selected_count: usize,
            input: &PlatformBuffer,
            gate_bias: Option<&PlatformBuffer>,
            up_bias: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let mode = match mode {
                QuantizationMode::GgmlQ8_0 => 0,
                QuantizationMode::GgmlMxfp4 => 1,
                other => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda moe gate/up kernel does not support {other:?}"
                    )));
                }
            };
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe gate/up row stride exceeds c_int"))
            })?;
            let rows_per_expert = c_int::try_from(rows_per_expert).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda moe gate/up rows per expert exceeds c_int",
                ))
            })?;
            let columns = c_int::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe gate/up columns exceeds c_int"))
            })?;
            let gate_rows = c_int::try_from(gate_rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe gate rows exceeds c_int"))
            })?;
            let up_rows = c_int::try_from(up_rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe up rows exceeds c_int"))
            })?;
            let selected_count = c_int::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe selected count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_moe_gate_up_swiglu(
                        weights.inner.device_ptr.cast(),
                        mode,
                        row_stride,
                        rows_per_expert,
                        columns,
                        gate_rows,
                        up_rows,
                        selected_ids.inner.device_ptr.cast(),
                        selected_count,
                        input.inner.device_ptr.cast(),
                        gate_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        up_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_moe_gate_up_swiglu",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_gate_up_swiglu_q8_1(
            &mut self,
            weights: &PlatformBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows_per_expert: usize,
            columns: usize,
            gate_rows: usize,
            up_rows: usize,
            selected_ids: &PlatformBuffer,
            selected_count: usize,
            input_q8_1: &PlatformBuffer,
            gate_bias: Option<&PlatformBuffer>,
            up_bias: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let mode = match mode {
                QuantizationMode::GgmlQ8_0 => 0,
                QuantizationMode::GgmlMxfp4 => 1,
                other => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda moe gate/up kernel does not support {other:?}"
                    )));
                }
            };
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe gate/up row stride exceeds c_int"))
            })?;
            let rows_per_expert = c_int::try_from(rows_per_expert).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda moe gate/up rows per expert exceeds c_int",
                ))
            })?;
            let columns = c_int::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe gate/up columns exceeds c_int"))
            })?;
            let gate_rows = c_int::try_from(gate_rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe gate rows exceeds c_int"))
            })?;
            let up_rows = c_int::try_from(up_rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe up rows exceeds c_int"))
            })?;
            let selected_count = c_int::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe selected count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_moe_gate_up_swiglu_q8_1(
                        weights.inner.device_ptr.cast(),
                        mode,
                        row_stride,
                        rows_per_expert,
                        columns,
                        gate_rows,
                        up_rows,
                        selected_ids.inner.device_ptr.cast(),
                        selected_count,
                        input_q8_1.inner.device_ptr.cast(),
                        gate_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        up_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_moe_gate_up_swiglu_q8_1",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_gate_up_swiglu_q8_1_selected4_quantized(
            &mut self,
            weights: &PlatformBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows_per_expert: usize,
            columns: usize,
            gate_rows: usize,
            up_rows: usize,
            selected_ids: &PlatformBuffer,
            selected_count: usize,
            input_q8_1: &PlatformBuffer,
            gate_bias: Option<&PlatformBuffer>,
            up_bias: Option<&PlatformBuffer>,
            output_q8_1: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let mode = match mode {
                QuantizationMode::GgmlQ8_0 => 0,
                QuantizationMode::GgmlMxfp4 => 1,
                other => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda quantized selected4 moe gate/up kernel does not support {other:?}"
                    )));
                }
            };
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized moe gate/up row stride exceeds c_int",
                ))
            })?;
            let rows_per_expert = c_int::try_from(rows_per_expert).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized moe gate/up rows per expert exceeds c_int",
                ))
            })?;
            let columns = c_int::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized moe gate/up columns exceed c_int",
                ))
            })?;
            let gate_rows = c_int::try_from(gate_rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda quantized moe gate rows exceed c_int"))
            })?;
            let up_rows = c_int::try_from(up_rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda quantized moe up rows exceed c_int"))
            })?;
            let selected_count = c_int::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda quantized moe selected count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_moe_gate_up_swiglu_q8_1_selected4_quantized(
                        weights.inner.device_ptr.cast(),
                        mode,
                        row_stride,
                        rows_per_expert,
                        columns,
                        gate_rows,
                        up_rows,
                        selected_ids.inner.device_ptr.cast(),
                        selected_count,
                        input_q8_1.inner.device_ptr.cast(),
                        gate_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        up_bias
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output_q8_1.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_moe_gate_up_swiglu_q8_1_selected4_quantized",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_down_aggregate(
            &mut self,
            weights: &PlatformBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows: usize,
            columns: usize,
            selected_ids: &PlatformBuffer,
            selected_weights: &PlatformBuffer,
            selected_count: usize,
            activated: &PlatformBuffer,
            bias: Option<&PlatformBuffer>,
            residual: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let mode = match mode {
                QuantizationMode::GgmlQ8_0 => 0,
                QuantizationMode::GgmlMxfp4 => 1,
                other => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda moe down kernel does not support {other:?}"
                    )));
                }
            };
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe down row stride exceeds c_int"))
            })?;
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe down rows exceeds c_int"))
            })?;
            let columns = c_int::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe down columns exceeds c_int"))
            })?;
            let selected_count = c_int::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe selected count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_moe_down_aggregate(
                        weights.inner.device_ptr.cast(),
                        mode,
                        row_stride,
                        rows,
                        columns,
                        selected_ids.inner.device_ptr.cast(),
                        selected_weights.inner.device_ptr.cast(),
                        selected_count,
                        activated.inner.device_ptr.cast(),
                        bias.map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        residual
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_moe_down_aggregate",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_down_aggregate_q8_1(
            &mut self,
            weights: &PlatformBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows: usize,
            columns: usize,
            selected_ids: &PlatformBuffer,
            selected_weights: &PlatformBuffer,
            selected_count: usize,
            activated_q8_1: &PlatformBuffer,
            bias: Option<&PlatformBuffer>,
            residual: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            let mode = match mode {
                QuantizationMode::GgmlQ8_0 => 0,
                QuantizationMode::GgmlMxfp4 => 1,
                other => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda moe down kernel does not support {other:?}"
                    )));
                }
            };
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe down row stride exceeds c_int"))
            })?;
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe down rows exceeds c_int"))
            })?;
            let columns = c_int::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe down columns exceeds c_int"))
            })?;
            let selected_count = c_int::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda moe selected count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_moe_down_aggregate_q8_1(
                        weights.inner.device_ptr.cast(),
                        mode,
                        row_stride,
                        rows,
                        columns,
                        selected_ids.inner.device_ptr.cast(),
                        selected_weights.inner.device_ptr.cast(),
                        selected_count,
                        activated_q8_1.inner.device_ptr.cast(),
                        bias.map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        residual
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_moe_down_aggregate_q8_1",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_down_aggregate_q8_1_f32(
            &mut self,
            weights: &PlatformBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows: usize,
            columns: usize,
            selected_ids: &PlatformBuffer,
            selected_weights: &PlatformBuffer,
            selected_count: usize,
            activated: &PlatformBuffer,
            bias: Option<&PlatformBuffer>,
            residual: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            if selected_count > 4 {
                return Err(RuntimeError::Backend(String::from(
                    "cuda fused moe down f32->q8_1 path requires selected_count <= 4",
                )));
            }
            let mode = match mode {
                QuantizationMode::GgmlQ8_0 => 0,
                QuantizationMode::GgmlMxfp4 => 1,
                other => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda fused moe down kernel does not support {other:?}"
                    )));
                }
            };
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from("cuda fused moe down row stride exceeds c_int"))
            })?;
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda fused moe down rows exceeds c_int"))
            })?;
            let columns = c_int::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from("cuda fused moe down columns exceeds c_int"))
            })?;
            let selected_count = c_int::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from("cuda fused moe selected count exceeds c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_moe_down_aggregate_q8_1_f32(
                        weights.inner.device_ptr.cast(),
                        mode,
                        row_stride,
                        rows,
                        columns,
                        selected_ids.inner.device_ptr.cast(),
                        selected_weights.inner.device_ptr.cast(),
                        selected_count,
                        activated.inner.device_ptr.cast(),
                        bias.map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        residual
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_moe_down_aggregate_q8_1_f32",
            )
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_down_project_q8_1_selected4(
            &mut self,
            weights: &PlatformBuffer,
            mode: QuantizationMode,
            row_stride: usize,
            rows: usize,
            columns: usize,
            selected_ids: &PlatformBuffer,
            selected_count: usize,
            activated_q8_1: &PlatformBuffer,
            bias: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            if selected_count > 4 {
                return Err(RuntimeError::Backend(String::from(
                    "cuda selected4 moe down projection requires selected_count <= 4",
                )));
            }
            let mode = match mode {
                QuantizationMode::GgmlQ8_0 => 0,
                QuantizationMode::GgmlMxfp4 => 1,
                other => {
                    return Err(RuntimeError::Backend(format!(
                        "cuda selected4 moe down projection does not support {other:?}"
                    )));
                }
            };
            let row_stride = c_int::try_from(row_stride).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda selected4 moe down projection row stride exceeds c_int",
                ))
            })?;
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda selected4 moe down projection rows exceed c_int",
                ))
            })?;
            let columns = c_int::try_from(columns).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda selected4 moe down projection columns exceed c_int",
                ))
            })?;
            let selected_count = c_int::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda selected4 moe down projection selected_count exceeds c_int",
                ))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_moe_down_project_q8_1_selected4(
                        weights.inner.device_ptr.cast(),
                        mode,
                        row_stride,
                        rows,
                        columns,
                        selected_ids.inner.device_ptr.cast(),
                        selected_count,
                        activated_q8_1.inner.device_ptr.cast(),
                        bias.map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_moe_down_project_q8_1_selected4",
            )
        }

        pub(super) fn encode_accumulate_selected4(
            &mut self,
            input: &PlatformBuffer,
            selected_weights: &PlatformBuffer,
            selected_count: usize,
            rows: usize,
            residual: Option<&PlatformBuffer>,
            output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            if selected_count > 4 {
                return Err(RuntimeError::Backend(String::from(
                    "cuda selected4 accumulate requires selected_count <= 4",
                )));
            }
            let selected_count = c_int::try_from(selected_count).map_err(|_| {
                RuntimeError::Backend(String::from(
                    "cuda selected4 accumulate selected_count exceeds c_int",
                ))
            })?;
            let rows = c_int::try_from(rows).map_err(|_| {
                RuntimeError::Backend(String::from("cuda selected4 accumulate rows exceed c_int"))
            })?;
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe {
                    psionic_cuda_accumulate_selected4(
                        input.inner.device_ptr.cast(),
                        selected_weights.inner.device_ptr.cast(),
                        selected_count,
                        rows,
                        residual
                            .map(|buffer| buffer.inner.device_ptr.cast())
                            .unwrap_or(std::ptr::null_mut()),
                        output.inner.device_ptr.cast(),
                        self.stream,
                    )
                },
                "psionic_cuda_accumulate_selected4",
            )
        }

        pub(super) fn commit(
            mut self,
            wait: CudaCommandWait,
        ) -> Result<CudaCommandStatus, RuntimeError> {
            if self.capturing {
                return Err(RuntimeError::Backend(String::from(
                    "captured platform submissions must use commit_captured",
                )));
            }
            match wait {
                CudaCommandWait::Completed => {
                    self.runtime.set_device()?;
                    self.runtime.check(
                        unsafe { (self.runtime.cuda_stream_synchronize)(self.stream) },
                        "cudaStreamSynchronize",
                    )?;
                    self.status = CudaCommandStatus::Completed;
                }
            }
            Ok(self.status)
        }

        pub(super) fn commit_captured(
            mut self,
            wait: CudaCommandWait,
        ) -> Result<(CudaCommandStatus, PlatformGraphExec), RuntimeError> {
            if !self.capturing {
                return Err(RuntimeError::Backend(String::from(
                    "ordinary platform submissions must use commit",
                )));
            }
            self.runtime.set_device()?;
            let mut graph = std::ptr::null_mut();
            self.runtime.check(
                unsafe { (self.runtime.cuda_stream_end_capture)(self.stream, &mut graph) },
                "cudaStreamEndCapture",
            )?;
            let mut instance = std::ptr::null_mut();
            self.runtime.check(
                unsafe { (self.runtime.cuda_graph_instantiate)(&mut instance, graph, 0) },
                "cudaGraphInstantiate",
            )?;
            self.runtime.check(
                unsafe { (self.runtime.cuda_graph_launch)(instance, self.stream) },
                "cudaGraphLaunch",
            )?;
            match wait {
                CudaCommandWait::Completed => {
                    self.runtime.check(
                        unsafe { (self.runtime.cuda_stream_synchronize)(self.stream) },
                        "cudaStreamSynchronize",
                    )?;
                    self.status = CudaCommandStatus::Completed;
                }
            }
            self.capturing = false;
            Ok((
                self.status,
                PlatformGraphExec {
                    runtime: Arc::clone(&self.runtime),
                    graph,
                    instance,
                    stream: self.stream,
                },
            ))
        }
    }

    impl PlatformGraphExec {
        pub(super) fn launch(
            &self,
            wait: CudaCommandWait,
        ) -> Result<CudaCommandStatus, RuntimeError> {
            self.runtime.set_device()?;
            self.runtime.check(
                unsafe { (self.runtime.cuda_graph_launch)(self.instance, self.stream) },
                "cudaGraphLaunch",
            )?;
            match wait {
                CudaCommandWait::Completed => {
                    self.runtime.check(
                        unsafe { (self.runtime.cuda_stream_synchronize)(self.stream) },
                        "cudaStreamSynchronize",
                    )?;
                    Ok(CudaCommandStatus::Completed)
                }
            }
        }
    }

    impl Drop for PlatformSubmission {
        fn drop(&mut self) {
            if self.owned_stream && !self.stream.is_null() {
                let _ = self.runtime.set_device();
                let _ = self.runtime.check(
                    unsafe { (self.runtime.cuda_stream_destroy)(self.stream) },
                    "cudaStreamDestroy",
                );
                self.stream = std::ptr::null_mut();
            }
        }
    }

    impl Drop for PlatformGraphExec {
        fn drop(&mut self) {
            let _ = self.runtime.set_device();
            if !self.instance.is_null() {
                let _ = self.runtime.check(
                    unsafe { (self.runtime.cuda_graph_exec_destroy)(self.instance) },
                    "cudaGraphExecDestroy",
                );
                self.instance = std::ptr::null_mut();
            }
            if !self.graph.is_null() {
                let _ = self.runtime.check(
                    unsafe { (self.runtime.cuda_graph_destroy)(self.graph) },
                    "cudaGraphDestroy",
                );
                self.graph = std::ptr::null_mut();
            }
        }
    }

    impl Drop for PlatformBufferInner {
        fn drop(&mut self) {
            if !self.device_ptr.is_null() {
                let _ = self.runtime.set_device();
                let _ = self.runtime.check(
                    unsafe { (self.runtime.cuda_free)(self.device_ptr) },
                    "cudaFree",
                );
                self.device_ptr = std::ptr::null_mut();
            }
        }
    }

    impl Drop for PlatformHostBufferInner {
        fn drop(&mut self) {
            if !self.host_ptr.is_null() {
                let _ = self.runtime.set_device();
                let _ = self.runtime.check(
                    unsafe { (self.runtime.cuda_free_host)(self.host_ptr) },
                    "cudaFreeHost",
                );
                self.host_ptr = std::ptr::null_mut();
            }
        }
    }

    impl CudaRuntime {
        fn load(ordinal: u16) -> Result<Arc<Self>, RuntimeError> {
            let cudart_library = load_cudart_library()?;
            let cublas_library = load_cublas_library()?;
            let mut runtime = Self {
                ordinal,
                cuda_get_error_string: unsafe {
                    load_symbol(&cudart_library, b"cudaGetErrorString\0")?
                },
                cuda_set_device: unsafe { load_symbol(&cudart_library, b"cudaSetDevice\0")? },
                cuda_malloc: unsafe { load_symbol(&cudart_library, b"cudaMalloc\0")? },
                cuda_malloc_host: unsafe { load_symbol(&cudart_library, b"cudaMallocHost\0")? },
                cuda_free: unsafe { load_symbol(&cudart_library, b"cudaFree\0")? },
                cuda_free_host: unsafe { load_symbol(&cudart_library, b"cudaFreeHost\0")? },
                cuda_memcpy: unsafe { load_symbol(&cudart_library, b"cudaMemcpy\0")? },
                cuda_memcpy_async: unsafe { load_symbol(&cudart_library, b"cudaMemcpyAsync\0")? },
                cuda_memset_async: unsafe { load_symbol(&cudart_library, b"cudaMemsetAsync\0")? },
                cuda_stream_create: unsafe { load_symbol(&cudart_library, b"cudaStreamCreate\0")? },
                cuda_stream_destroy: unsafe {
                    load_symbol(&cudart_library, b"cudaStreamDestroy\0")?
                },
                cuda_stream_synchronize: unsafe {
                    load_symbol(&cudart_library, b"cudaStreamSynchronize\0")?
                },
                cuda_stream_begin_capture: unsafe {
                    load_symbol(&cudart_library, b"cudaStreamBeginCapture\0")?
                },
                cuda_stream_end_capture: unsafe {
                    load_symbol(&cudart_library, b"cudaStreamEndCapture\0")?
                },
                cuda_graph_instantiate: unsafe {
                    load_symbol(&cudart_library, b"cudaGraphInstantiate\0")?
                },
                cuda_graph_launch: unsafe { load_symbol(&cudart_library, b"cudaGraphLaunch\0")? },
                cuda_graph_exec_destroy: unsafe {
                    load_symbol(&cudart_library, b"cudaGraphExecDestroy\0")?
                },
                cuda_graph_destroy: unsafe { load_symbol(&cudart_library, b"cudaGraphDestroy\0")? },
                cublas_handle: std::ptr::null_mut(),
                cublas_create: unsafe { load_symbol(&cublas_library, b"cublasCreate_v2\0")? },
                cublas_destroy: unsafe { load_symbol(&cublas_library, b"cublasDestroy_v2\0")? },
                cublas_set_stream: unsafe {
                    load_symbol(&cublas_library, b"cublasSetStream_v2\0")?
                },
                cublas_sgemm: unsafe { load_symbol(&cublas_library, b"cublasSgemm_v2\0")? },
                cublas_sgeam: unsafe { load_symbol(&cublas_library, b"cublasSgeam\0")? },
                cublas_gemm_ex: unsafe { load_symbol(&cublas_library, b"cublasGemmEx\0")? },
                _cudart_library: cudart_library,
                _cublas_library: cublas_library,
            };
            runtime.set_device()?;
            let mut handle = std::ptr::null_mut();
            runtime.check_cublas(
                unsafe { (runtime.cublas_create)(&mut handle) },
                "cublasCreate_v2",
            )?;
            runtime.cublas_handle = handle;
            Ok(Arc::new(runtime))
        }

        fn set_device(&self) -> Result<(), RuntimeError> {
            self.check(
                unsafe { (self.cuda_set_device)(i32::from(self.ordinal)) },
                "cudaSetDevice",
            )
        }

        fn bind_stream(&self, stream: CudaStream) -> Result<(), RuntimeError> {
            self.set_device()?;
            self.check_cublas(
                unsafe { (self.cublas_set_stream)(self.cublas_handle, stream) },
                "cublasSetStream_v2",
            )
        }

        fn check(&self, code: CudaError, operation: &str) -> Result<(), RuntimeError> {
            if code == CUDA_SUCCESS {
                return Ok(());
            }
            let message = unsafe {
                let error_ptr = (self.cuda_get_error_string)(code);
                if error_ptr.is_null() {
                    None
                } else {
                    Some(CStr::from_ptr(error_ptr).to_string_lossy().into_owned())
                }
            }
            .unwrap_or_else(|| format!("CUDA error code {code}"));
            Err(RuntimeError::Backend(format!(
                "{operation} failed: {message}"
            )))
        }

        fn check_cublas(&self, status: CublasStatus, operation: &str) -> Result<(), RuntimeError> {
            if status == CUBLAS_STATUS_SUCCESS {
                return Ok(());
            }
            Err(RuntimeError::Backend(format!(
                "{operation} failed with cuBLAS status {status}"
            )))
        }
    }

    impl Drop for CudaRuntime {
        fn drop(&mut self) {
            if !self.cublas_handle.is_null() {
                let _ = self.set_device();
                let _ = self.check_cublas(
                    unsafe { (self.cublas_destroy)(self.cublas_handle) },
                    "cublasDestroy_v2",
                );
                self.cublas_handle = std::ptr::null_mut();
            }
        }
    }

    pub(super) fn configure_backend(
        descriptor: super::DeviceDescriptor,
    ) -> Result<ConfiguredBackend, RuntimeError> {
        let runtime = CudaRuntime::load(descriptor.device.ordinal())?;
        runtime.set_device()?;
        let mut stream = std::ptr::null_mut();
        runtime.check(
            unsafe { (runtime.cuda_stream_create)(&mut stream) },
            "cudaStreamCreate",
        )?;
        Ok(ConfiguredBackend { runtime, stream })
    }

    impl Drop for ConfiguredBackend {
        fn drop(&mut self) {
            if !self.stream.is_null() {
                let _ = self.runtime.set_device();
                let _ = self.runtime.check(
                    unsafe { (self.runtime.cuda_stream_destroy)(self.stream) },
                    "cudaStreamDestroy",
                );
                self.stream = std::ptr::null_mut();
            }
        }
    }

    unsafe fn load_symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, RuntimeError> {
        unsafe { library.get::<T>(name) }
            .map(|symbol| *symbol)
            .map_err(|error| {
                RuntimeError::Backend(format!(
                    "failed to load CUDA runtime symbol {}: {error}",
                    String::from_utf8_lossy(name).trim_end_matches('\0')
                ))
            })
    }

    fn load_cudart_library() -> Result<Library, RuntimeError> {
        load_library(
            &["libcudart.so.13", "libcudart.so"],
            "failed to load libcudart.so.13 or libcudart.so",
        )
    }

    fn load_cublas_library() -> Result<Library, RuntimeError> {
        load_library(
            &["libcublas.so.13", "libcublas.so"],
            "failed to load libcublas.so.13 or libcublas.so",
        )
    }

    fn load_library(candidates: &[&str], failure: &str) -> Result<Library, RuntimeError> {
        for candidate in candidates {
            let library = unsafe { Library::new(candidate) };
            if let Ok(library) = library {
                return Ok(library);
            }
        }
        Err(RuntimeError::Backend(String::from(failure)))
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::{CudaCommandStatus, CudaCommandWait, QuantizationMode};
    use psionic_runtime::RuntimeError;

    #[derive(Clone)]
    pub(super) struct PlatformBuffer;

    #[derive(Clone)]
    pub(super) struct PlatformHostBuffer;

    pub(super) struct PlatformGraphExec;

    pub(super) struct PlatformSubmission;

    pub(super) struct ConfiguredBackend;

    impl ConfiguredBackend {
        pub(super) fn allocate(&self, _byte_len: usize) -> Result<PlatformBuffer, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn allocate_host(
            &self,
            _byte_len: usize,
        ) -> Result<PlatformHostBuffer, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn begin_submission(&self) -> Result<PlatformSubmission, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn begin_capture_submission(&self) -> Result<PlatformSubmission, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }
    }

    pub(super) fn quantized_kernels_compiled() -> bool {
        false
    }

    impl PlatformBuffer {
        pub(super) fn allocation_identity(&self) -> usize {
            0
        }

        pub(super) fn write_bytes(&self, _bytes: &[u8]) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn write_bytes_at_offset(
            &self,
            _byte_offset: usize,
            _bytes: &[u8],
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn read_bytes(&self, _byte_len: usize) -> Result<Vec<u8>, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn read_bytes_at_offset(
            &self,
            _byte_offset: usize,
            _byte_len: usize,
        ) -> Result<Vec<u8>, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }
    }

    impl PlatformHostBuffer {
        pub(super) fn write_bytes(&self, _bytes: &[u8]) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn read_bytes(&self, _byte_len: usize) -> Result<Vec<u8>, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }
    }

    impl PlatformSubmission {
        pub(super) fn fill_buffer(
            &mut self,
            _buffer: &PlatformBuffer,
            _byte_len: usize,
            _value: u8,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn copy_buffer(
            &mut self,
            _source: &PlatformBuffer,
            _destination: &PlatformBuffer,
            _byte_len: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn copy_host_to_device(
            &mut self,
            _source: &PlatformHostBuffer,
            _destination: &PlatformBuffer,
            _byte_len: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn copy_device_to_host(
            &mut self,
            _source: &PlatformBuffer,
            _destination: &PlatformHostBuffer,
            _byte_len: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn copy_buffer_region(
            &mut self,
            _source: &PlatformBuffer,
            _source_byte_offset: usize,
            _destination: &PlatformBuffer,
            _destination_byte_offset: usize,
            _byte_len: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn encode_add(
            &mut self,
            _left: &PlatformBuffer,
            _right: &PlatformBuffer,
            _output: &PlatformBuffer,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn encode_matmul(
            &mut self,
            _left: &PlatformBuffer,
            _right: &PlatformBuffer,
            _output: &PlatformBuffer,
            _rows: usize,
            _inner: usize,
            _cols: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn encode_matmul_f16_to_f32(
            &mut self,
            _left: &PlatformBuffer,
            _right: &PlatformBuffer,
            _output: &PlatformBuffer,
            _rows: usize,
            _inner: usize,
            _cols: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn encode_quantized_matvec(
            &mut self,
            _weights: &PlatformBuffer,
            _byte_offset: usize,
            _mode: QuantizationMode,
            _rows: usize,
            _cols: usize,
            _input: &PlatformBuffer,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_quantize_f32_to_q8_1(
            &mut self,
            _input: &PlatformBuffer,
            _rows: usize,
            _cols: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_dequantize_row_to_f32(
            &mut self,
            _weights: &PlatformBuffer,
            _mode: QuantizationMode,
            _rows: usize,
            _row_stride: usize,
            _cols: usize,
            _decode_params: &PlatformBuffer,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_gather_f16_row_to_f32(
            &mut self,
            _input: &PlatformBuffer,
            _rows: usize,
            _cols: usize,
            _decode_params: &PlatformBuffer,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_cast_f32_to_f16(
            &mut self,
            _input: &PlatformBuffer,
            _output: &PlatformBuffer,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_quantized_matvec_q8_1(
            &mut self,
            _weights: &PlatformBuffer,
            _byte_offset: usize,
            _mode: QuantizationMode,
            _rows: usize,
            _cols: usize,
            _input_q8_1: &PlatformBuffer,
            _bias: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_quantized_matvec_q8_1_argmax(
            &mut self,
            _weights: &PlatformBuffer,
            _byte_offset: usize,
            _mode: QuantizationMode,
            _rows: usize,
            _cols: usize,
            _input_q8_1: &PlatformBuffer,
            _bias: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_argmax_f32(
            &mut self,
            _input: &PlatformBuffer,
            _rows: usize,
            _cols: usize,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_rms_norm(
            &mut self,
            _input: &PlatformBuffer,
            _weight: &PlatformBuffer,
            _output: &PlatformBuffer,
            _element_count: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_rms_norm_q8_1(
            &mut self,
            _input: &PlatformBuffer,
            _weight: &PlatformBuffer,
            _output_q8_1: &PlatformBuffer,
            _element_count: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_add_residual_rms_norm(
            &mut self,
            _input: &PlatformBuffer,
            _residual: &PlatformBuffer,
            _input_bias: Option<&PlatformBuffer>,
            _weight: &PlatformBuffer,
            _summed_output: &PlatformBuffer,
            _normalized_output: &PlatformBuffer,
            _element_count: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_add_residual_rms_norm_q8_1(
            &mut self,
            _input: &PlatformBuffer,
            _residual: &PlatformBuffer,
            _input_bias: Option<&PlatformBuffer>,
            _weight: &PlatformBuffer,
            _summed_output: &PlatformBuffer,
            _normalized_output: &PlatformBuffer,
            _quantized_output: &PlatformBuffer,
            _element_count: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_add_residual_rms_norm_q8_1_router_topk(
            &mut self,
            _input: &PlatformBuffer,
            _residual: &PlatformBuffer,
            _input_bias: Option<&PlatformBuffer>,
            _weight: &PlatformBuffer,
            _summed_output: &PlatformBuffer,
            _normalized_output: &PlatformBuffer,
            _quantized_output: &PlatformBuffer,
            _router_weights: &PlatformBuffer,
            _router_bias: Option<&PlatformBuffer>,
            _expert_count: usize,
            _top_k: usize,
            _selected_ids: &PlatformBuffer,
            _selected_weights: &PlatformBuffer,
            _element_count: usize,
            _epsilon: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_add_f32_in_place(
            &mut self,
            _destination: &PlatformBuffer,
            _element_offset: usize,
            _rhs: &PlatformBuffer,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_rope_neox_in_place(
            &mut self,
            _values: &PlatformBuffer,
            _element_offset: usize,
            _head_count: usize,
            _head_dim: usize,
            _rotary_dim: usize,
            _position: usize,
            _freq_scale: f32,
            _ext_factor: f32,
            _corr_dims: [f32; 2],
            _theta_scale: f32,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache(
            &mut self,
            _qkv: &PlatformBuffer,
            _query_offset: usize,
            _key_offset: usize,
            _value_offset: usize,
            _cache_keys: &PlatformBuffer,
            _cache_values: &PlatformBuffer,
            _cache_width: usize,
            _layer_offset: usize,
            _past_tokens: usize,
            _sliding_window: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _rotary_dim: usize,
            _position: usize,
            _freq_scale: f32,
            _ext_factor: f32,
            _corr_dims: [f32; 2],
            _theta_scale: f32,
            _attention_sinks: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv(
            &mut self,
            _qkv: &PlatformBuffer,
            _query_offset: usize,
            _key_offset: usize,
            _value_offset: usize,
            _cache_keys: &PlatformBuffer,
            _cache_values: &PlatformBuffer,
            _cache_width: usize,
            _layer_offset: usize,
            _past_tokens: usize,
            _sliding_window: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _rotary_dim: usize,
            _position: usize,
            _freq_scale: f32,
            _ext_factor: f32,
            _corr_dims: [f32; 2],
            _theta_scale: f32,
            _attention_sinks: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv_q8_1(
            &mut self,
            _qkv: &PlatformBuffer,
            _query_offset: usize,
            _key_offset: usize,
            _value_offset: usize,
            _cache_keys: &PlatformBuffer,
            _cache_values: &PlatformBuffer,
            _cache_width: usize,
            _layer_offset: usize,
            _past_tokens: usize,
            _sliding_window: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _rotary_dim: usize,
            _position: usize,
            _freq_scale: f32,
            _ext_factor: f32,
            _corr_dims: [f32; 2],
            _theta_scale: f32,
            _attention_sinks: Option<&PlatformBuffer>,
            _output_q8_1: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv_graph(
            &mut self,
            _qkv: &PlatformBuffer,
            _query_offset: usize,
            _key_offset: usize,
            _value_offset: usize,
            _cache_keys: &PlatformBuffer,
            _cache_values: &PlatformBuffer,
            _cache_width: usize,
            _layer_offset: usize,
            _decode_params: &PlatformBuffer,
            _sliding_window: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _rotary_dim: usize,
            _freq_scale: f32,
            _ext_factor: f32,
            _corr_dims: [f32; 2],
            _theta_scale: f32,
            _attention_sinks: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode_rope_cache_f16_kv_graph_q8_1(
            &mut self,
            _qkv: &PlatformBuffer,
            _query_offset: usize,
            _key_offset: usize,
            _value_offset: usize,
            _cache_keys: &PlatformBuffer,
            _cache_values: &PlatformBuffer,
            _cache_width: usize,
            _layer_offset: usize,
            _decode_params: &PlatformBuffer,
            _sliding_window: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _rotary_dim: usize,
            _freq_scale: f32,
            _ext_factor: f32,
            _corr_dims: [f32; 2],
            _theta_scale: f32,
            _attention_sinks: Option<&PlatformBuffer>,
            _output_q8_1: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_attention_decode(
            &mut self,
            _query: &PlatformBuffer,
            _query_offset: usize,
            _current_key: &PlatformBuffer,
            _key_offset: usize,
            _current_value: &PlatformBuffer,
            _value_offset: usize,
            _cache_keys: &PlatformBuffer,
            _cache_values: &PlatformBuffer,
            _cache_width: usize,
            _layer_offset: usize,
            _past_tokens: usize,
            _sliding_window: usize,
            _head_count: usize,
            _kv_head_count: usize,
            _head_dim: usize,
            _attention_sinks: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_router_topk_softmax(
            &mut self,
            _weights: &PlatformBuffer,
            _bias: Option<&PlatformBuffer>,
            _input: &PlatformBuffer,
            _expert_count: usize,
            _input_size: usize,
            _top_k: usize,
            _selected_ids: &PlatformBuffer,
            _selected_weights: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_router_topk_delayed_softmax(
            &mut self,
            _logits: &PlatformBuffer,
            _expert_count: usize,
            _top_k: usize,
            _selected_ids: &PlatformBuffer,
            _selected_weights: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_gate_up_swiglu(
            &mut self,
            _weights: &PlatformBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows_per_expert: usize,
            _columns: usize,
            _gate_rows: usize,
            _up_rows: usize,
            _selected_ids: &PlatformBuffer,
            _selected_count: usize,
            _input: &PlatformBuffer,
            _gate_bias: Option<&PlatformBuffer>,
            _up_bias: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_gate_up_swiglu_q8_1(
            &mut self,
            _weights: &PlatformBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows_per_expert: usize,
            _columns: usize,
            _gate_rows: usize,
            _up_rows: usize,
            _selected_ids: &PlatformBuffer,
            _selected_count: usize,
            _input_q8_1: &PlatformBuffer,
            _gate_bias: Option<&PlatformBuffer>,
            _up_bias: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_gate_up_swiglu_q8_1_selected4_quantized(
            &mut self,
            _weights: &PlatformBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows_per_expert: usize,
            _columns: usize,
            _gate_rows: usize,
            _up_rows: usize,
            _selected_ids: &PlatformBuffer,
            _selected_count: usize,
            _input_q8_1: &PlatformBuffer,
            _gate_bias: Option<&PlatformBuffer>,
            _up_bias: Option<&PlatformBuffer>,
            _output_q8_1: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_down_aggregate(
            &mut self,
            _weights: &PlatformBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows: usize,
            _columns: usize,
            _selected_ids: &PlatformBuffer,
            _selected_weights: &PlatformBuffer,
            _selected_count: usize,
            _activated: &PlatformBuffer,
            _bias: Option<&PlatformBuffer>,
            _residual: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_down_aggregate_q8_1(
            &mut self,
            _weights: &PlatformBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows: usize,
            _columns: usize,
            _selected_ids: &PlatformBuffer,
            _selected_weights: &PlatformBuffer,
            _selected_count: usize,
            _activated_q8_1: &PlatformBuffer,
            _bias: Option<&PlatformBuffer>,
            _residual: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_down_aggregate_q8_1_f32(
            &mut self,
            _weights: &PlatformBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows: usize,
            _columns: usize,
            _selected_ids: &PlatformBuffer,
            _selected_weights: &PlatformBuffer,
            _selected_count: usize,
            _activated: &PlatformBuffer,
            _bias: Option<&PlatformBuffer>,
            _residual: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn encode_moe_down_project_q8_1_selected4(
            &mut self,
            _weights: &PlatformBuffer,
            _mode: QuantizationMode,
            _row_stride: usize,
            _rows: usize,
            _columns: usize,
            _selected_ids: &PlatformBuffer,
            _selected_count: usize,
            _activated_q8_1: &PlatformBuffer,
            _bias: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn encode_accumulate_selected4(
            &mut self,
            _input: &PlatformBuffer,
            _selected_weights: &PlatformBuffer,
            _selected_count: usize,
            _rows: usize,
            _residual: Option<&PlatformBuffer>,
            _output: &PlatformBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda quantized text-generation kernels require Linux CUDA support",
            )))
        }

        pub(super) fn commit(
            self,
            _wait: CudaCommandWait,
        ) -> Result<CudaCommandStatus, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }

        pub(super) fn commit_captured(
            self,
            _wait: CudaCommandWait,
        ) -> Result<(CudaCommandStatus, PlatformGraphExec), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }
    }

    impl PlatformGraphExec {
        pub(super) fn launch(
            &self,
            _wait: CudaCommandWait,
        ) -> Result<CudaCommandStatus, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "cuda runtime substrate currently requires Linux libcudart",
            )))
        }
    }

    pub(super) fn configure_backend(
        _descriptor: super::DeviceDescriptor,
    ) -> Result<ConfiguredBackend, RuntimeError> {
        Err(RuntimeError::Backend(String::from(
            "cuda runtime substrate currently requires Linux libcudart",
        )))
    }
}

#[cfg(test)]
mod tests {
    use psionic_backend_cpu::CpuBackend;
    use psionic_backend_cpu::decode_quantized_row_into;
    use psionic_backend_cpu::quantized_row_dot;
    use psionic_compiler::compile_graph;
    use psionic_core::{DType, DeviceKind, QuantizationMode, Shape, TensorSpec};
    use psionic_ir::GraphBuilder;

    use super::CudaMemorySpace;
    use super::{
        CudaBackend, CudaCommandStatus, CudaCommandWait, HealthStatus, SUPPORTED_OPS,
        architecture_from_compute_capability, cuda_health, parse_inventory_row, parse_mig_profile,
        recovery_profile, risk_profile, validate_supported_plan,
    };
    use psionic_core::Device;
    use psionic_runtime::{
        Allocator, BackendDegradedPolicy, BackendSelectionState, DeviceDiscovery,
        NvidiaRecoveryAction, NvidiaRiskLevel, ServedProductBackendPolicy,
    };

    #[test]
    fn inventory_row_parses_into_expected_descriptor_inputs() {
        let row = parse_inventory_row(
            "0, NVIDIA GeForce RTX 4080, 00000000:01:00.0, 16376, 8.9, Yes, [N/A], Disabled, HMM",
        )
        .expect("inventory row should parse");

        assert_eq!(row.ordinal, 0);
        assert_eq!(row.name, "NVIDIA GeForce RTX 4080");
        assert_eq!(row.compute_capability.as_deref(), Some("8.9"));
        assert_eq!(row.memory_bytes, Some(16376 * 1024 * 1024));
        assert_eq!(row.display_attached, Some(true));
        assert_eq!(row.mig_profile, None);
        assert_eq!(row.persistence_mode_enabled, Some(false));
        assert_eq!(row.addressing_mode.as_deref(), Some("hmm"));
    }

    #[test]
    fn architecture_mapping_handles_known_compute_capabilities() {
        assert_eq!(
            architecture_from_compute_capability(Some("8.9")).as_deref(),
            Some("ada")
        );
        assert_eq!(
            architecture_from_compute_capability(Some("9.0")).as_deref(),
            Some("hopper")
        );
        assert_eq!(architecture_from_compute_capability(Some("6.1")), None);
    }

    #[test]
    fn mig_parser_skips_disabled_and_na_values() {
        assert_eq!(parse_mig_profile("[N/A]"), None);
        assert_eq!(parse_mig_profile("Disabled"), None);
        assert_eq!(parse_mig_profile("1g.10gb"), Some(String::from("1g.10gb")));
    }

    #[test]
    fn risk_profile_marks_display_and_mig_devices_as_elevated() {
        let display_risk = risk_profile(Some(true), None, Some(false));
        assert_eq!(display_risk.level, NvidiaRiskLevel::Elevated);
        assert!(
            display_risk
                .warnings
                .iter()
                .any(|warning| warning.contains("display-attached"))
        );

        let mig_risk = risk_profile(Some(false), Some("1g.10gb"), Some(true));
        assert_eq!(mig_risk.level, NvidiaRiskLevel::Elevated);
        assert!(mig_risk.mig_partitioned);
    }

    #[test]
    fn recovery_profile_preserves_gpu_reset_when_driver_reports_none() {
        let recovery = recovery_profile(Some("None"));
        assert_eq!(recovery.supports_gpu_reset, Some(true));
        assert_eq!(
            recovery.expected_actions,
            vec![
                NvidiaRecoveryAction::ProcessRestart,
                NvidiaRecoveryAction::GpuReset,
                NvidiaRecoveryAction::RebootHost,
            ]
        );
    }

    #[test]
    fn cuda_dense_surface_is_documented() {
        assert_eq!(SUPPORTED_OPS, &["input", "constant", "matmul", "add"]);
    }

    #[test]
    fn cuda_plan_validation_rejects_unsupported_ops() -> Result<(), Box<dyn std::error::Error>> {
        let device = Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0")));
        let mut builder = GraphBuilder::new(device);
        let input = builder.input("features", Shape::new(vec![1, 2]), DType::F32);
        let weights = builder.constant_f32(Shape::new(vec![1, 2]), vec![1.0, 0.0])?;
        let unsupported = builder.mul(&input, &weights)?;
        let graph = builder.finish(vec![unsupported]);
        let plan = compile_graph(&graph)?;
        let error = validate_supported_plan(&plan).expect_err("mul should be rejected");
        assert_eq!(
            error,
            psionic_runtime::RuntimeError::UnsupportedStep(String::from("mul"))
        );
        Ok(())
    }

    #[test]
    fn cuda_health_is_degraded_when_a_discovered_device_is_elevated() {
        let devices = vec![
            parse_inventory_row(
                "0, NVIDIA GeForce RTX 4080, 00000000:01:00.0, 16376, 8.9, Yes, [N/A], Disabled, HMM",
            )
            .expect("inventory row should parse")
            .into_device_descriptor(),
        ];
        let health = cuda_health(&devices);
        assert_eq!(health.status, HealthStatus::Degraded);
        assert!(health.message.contains("display-attached"));
    }

    #[test]
    fn cuda_backend_report_is_self_consistent() -> Result<(), psionic_runtime::RuntimeError> {
        let backend = CudaBackend::new();
        let report = backend.discovery_report()?;
        assert_eq!(backend.backend_name(), "cuda");
        match report.health.status {
            HealthStatus::Ready => assert!(!report.devices.is_empty()),
            HealthStatus::Degraded => assert!(!report.devices.is_empty()),
            HealthStatus::Offline => {}
        }
        Ok(())
    }

    #[test]
    fn cuda_backend_runtime_resources_are_explicit_when_available() {
        let backend = CudaBackend::new();
        if let Some(descriptor) = backend.selected_device() {
            let resources = backend
                .runtime_resources()
                .expect("available cuda backend should surface runtime resources");
            assert_eq!(resources.allocator_pool.policy.max_cached_buffers, 128);
            assert!(!resources.kernel_cache.policy.enabled);
            assert_eq!(
                resources
                    .device_memory_budget
                    .as_ref()
                    .and_then(|budget| budget.total_bytes),
                descriptor.memory_capacity_bytes
            );
        } else {
            assert!(backend.runtime_resources().is_none());
        }
    }

    #[test]
    fn cuda_backend_selection_reports_ready_or_degraded_cuda_when_available()
    -> Result<(), psionic_runtime::RuntimeError> {
        let backend = CudaBackend::new();
        let Some(_) = backend.selected_device() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let selection = backend.backend_selection(SUPPORTED_OPS)?;
        assert_eq!(selection.requested_backend, "cuda");
        assert_eq!(selection.effective_backend, "cuda");
        assert_eq!(
            selection.supported_ops,
            SUPPORTED_OPS
                .iter()
                .map(|label| String::from(*label))
                .collect::<Vec<_>>()
        );
        assert!(selection.selected_device.is_some());
        assert!(selection.runtime_resources.is_some());
        assert_eq!(
            selection.policy,
            ServedProductBackendPolicy::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend
            )
        );

        match backend.health().status {
            HealthStatus::Ready => {
                assert_eq!(selection.selection_state, BackendSelectionState::Direct);
                assert!(selection.degraded_reason.is_none());
            }
            HealthStatus::Degraded => {
                assert_eq!(
                    selection.selection_state,
                    BackendSelectionState::SameBackendDegraded
                );
                assert!(selection.degraded_reason.is_some());
            }
            HealthStatus::Offline => {
                panic!("selected CUDA device should not report offline health");
            }
        }

        Ok(())
    }

    #[test]
    fn cuda_backend_fallback_selection_reports_explicit_cpu_fallback_when_unavailable()
    -> Result<(), psionic_runtime::RuntimeError> {
        let backend = CudaBackend::new();
        if backend.selected_device().is_none() {
            let cpu = CpuBackend::new();
            let selection = backend.fallback_selection(&cpu, SUPPORTED_OPS)?;
            assert_eq!(selection.requested_backend, "cuda");
            assert_eq!(selection.effective_backend, "cpu");
            assert_eq!(
                selection.supported_ops,
                SUPPORTED_OPS
                    .iter()
                    .map(|label| String::from(*label))
                    .collect::<Vec<_>>()
            );
            assert!(selection.selected_device.is_some());
            assert!(selection.runtime_resources.is_some());
            assert!(selection.fallback_reason.is_some());
            assert_eq!(
                selection.policy,
                ServedProductBackendPolicy::fallback_to_compatible_backend(
                    BackendDegradedPolicy::AllowSameBackend
                )
            );
            assert_eq!(
                selection.selection_state,
                BackendSelectionState::CrossBackendFallback
            );
            assert!(selection.degraded_reason.is_none());
        }
        Ok(())
    }

    #[test]
    fn cuda_backend_allocates_and_submits_copy_when_available()
    -> Result<(), psionic_runtime::RuntimeError> {
        let mut backend = CudaBackend::new();
        let Some(device) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let spec = TensorSpec::new(Shape::new(vec![4]), DType::F32, device.device.clone());
        let mut left = backend.allocate(&spec)?;
        let right = backend.allocate(&spec)?;
        assert_eq!(left.memory_space(), CudaMemorySpace::Device);
        assert!(!left.host_visible());
        left.write_f32(&[1.0, 2.0, 3.0, 4.0])?;

        let mut submission = backend.begin_submission()?;
        submission.copy_buffer(&left, &right)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.status, CudaCommandStatus::Completed);
        assert_eq!(report.encoded_operations, 1);
        assert_eq!(right.read_f32()?, vec![1.0, 2.0, 3.0, 4.0]);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_dense_surface_when_available() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut builder = GraphBuilder::new(selected.device.clone());
        let input = builder.input("features", Shape::new(vec![1, 2]), DType::F32);
        let weights = builder.constant_f32(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])?;
        let bias = builder.constant_f32(Shape::new(vec![1, 2]), vec![0.5, 0.5])?;
        let projected = builder.matmul(&input, &weights)?;
        let shifted = builder.add(&projected, &bias)?;
        let graph = builder.finish(vec![shifted.clone()]);

        let mut inputs = std::collections::BTreeMap::new();
        inputs.insert(
            input.id(),
            backend.input_buffer(Shape::new(vec![1, 2]), vec![1.0, 0.0])?,
        );

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let output = result
            .outputs
            .get(&shifted.id())
            .ok_or("missing cuda dense output")?;
        assert_eq!(output.read_f32()?, vec![1.5, 2.5]);
        assert_eq!(result.metrics.steps_executed, 5);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_f16_rhs_matmul_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut left = backend.f16_buffer(2)?;
        let mut left_bytes = Vec::new();
        for value in [1.0_f32, 2.0] {
            left_bytes.extend_from_slice(&f32_to_f16_bits(value).to_le_bytes());
        }
        left.write_bytes(left_bytes.as_slice())?;
        let mut right = backend.f16_buffer(4)?;
        let mut right_bytes = Vec::new();
        for value in [1.0_f32, 2.0, 3.0, 4.0] {
            right_bytes.extend_from_slice(&f32_to_f16_bits(value).to_le_bytes());
        }
        right.write_bytes(right_bytes.as_slice())?;
        let output = backend.f32_buffer(2)?;

        let mut submission = backend.begin_submission()?;
        submission.matmul_f16_to_f32(&left, &right, &output, 1, 2, 2)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.status, CudaCommandStatus::Completed);
        assert_eq!(output.read_f32()?, vec![7.0, 10.0]);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_multiple_matmuls_and_adds_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };

        let mut builder = GraphBuilder::new(selected.device.clone());
        let token_input = builder.input("token", Shape::new(vec![1, 2]), DType::F32);
        let position_input = builder.input("position", Shape::new(vec![1, 2]), DType::F32);
        let context_input = builder.input("context", Shape::new(vec![1, 2]), DType::F32);
        let token_embedding =
            builder.constant_f32(Shape::new(vec![2, 2]), vec![1.0, 2.0, 3.0, 4.0])?;
        let position_embedding =
            builder.constant_f32(Shape::new(vec![2, 2]), vec![0.5, 1.5, 2.5, 3.5])?;
        let context_projection =
            builder.constant_f32(Shape::new(vec![2, 2]), vec![2.0, 0.0, 0.0, 2.0])?;
        let lm_head =
            builder.constant_f32(Shape::new(vec![2, 3]), vec![1.0, 0.0, 2.0, 0.5, 1.0, -1.0])?;
        let lm_bias = builder.constant_f32(Shape::new(vec![1, 3]), vec![0.25, -0.5, 1.0])?;

        let token_hidden = builder.matmul(&token_input, &token_embedding)?;
        let position_hidden = builder.matmul(&position_input, &position_embedding)?;
        let context_hidden = builder.matmul(&context_input, &context_projection)?;
        let hidden = builder.add(&token_hidden, &position_hidden)?;
        let hidden = builder.add(&hidden, &context_hidden)?;
        let logits = builder.matmul(&hidden, &lm_head)?;
        let logits = builder.add(&logits, &lm_bias)?;
        let graph = builder.finish(vec![hidden.clone(), logits.clone()]);

        let mut inputs = std::collections::BTreeMap::new();
        inputs.insert(
            token_input.id(),
            backend.input_buffer(Shape::new(vec![1, 2]), vec![1.0, 0.0])?,
        );
        inputs.insert(
            position_input.id(),
            backend.input_buffer(Shape::new(vec![1, 2]), vec![0.0, 1.0])?,
        );
        inputs.insert(
            context_input.id(),
            backend.input_buffer(Shape::new(vec![1, 2]), vec![0.5, 0.25])?,
        );

        let result = backend.compile_and_execute(&graph, &inputs)?;
        let hidden_output = result
            .outputs
            .get(&hidden.id())
            .ok_or("missing cuda hidden output")?;
        let logits_output = result
            .outputs
            .get(&logits.id())
            .ok_or("missing cuda logits output")?;
        assert_eq!(hidden_output.read_f32()?, vec![4.5, 6.0]);
        assert_eq!(logits_output.read_f32()?, vec![7.75, 5.5, 4.0]);
        assert_eq!(result.metrics.steps_executed, 15);
        Ok(())
    }

    #[test]
    fn cuda_backend_refuses_allocation_when_unavailable() {
        let mut backend = CudaBackend::new();
        if backend.selected_device().is_some() {
            return;
        }
        let spec = TensorSpec::new(Shape::new(vec![1]), DType::F32, Device::cpu());
        assert!(backend.allocate(&spec).is_err());
    }

    #[test]
    fn cuda_backend_executes_q8_0_quantized_matvec_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = vec![1.0_f32; 32];
        let row_a = sample_q8_0_row(0.25, 1);
        let row_b = sample_q8_0_row(0.5, -1);
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_a)?,
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_b)?,
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let actual =
            backend.quantized_matvec(&weights, QuantizationMode::GgmlQ8_0, 2, 32, &input)?;
        assert_close(&actual, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_backend_dequantizes_q8_0_row_when_available() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let row_a = sample_q8_0_row(0.25, 1);
        let row_b = sample_q8_0_row(0.5, -1);
        let mut expected = Vec::new();
        decode_quantized_row_into(QuantizationMode::GgmlQ8_0, &row_b, &mut expected)?;
        let mut bytes = row_a;
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let decode_params = backend.byte_buffer(&i32_slice_to_bytes(&[0_i32, 0_i32, 1_i32]))?;
        let output = backend.f32_buffer(32)?;
        let mut submission = backend.begin_submission()?;
        submission.dequantize_row_to_f32(
            &weights,
            QuantizationMode::GgmlQ8_0,
            2,
            34,
            32,
            &decode_params,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);
        assert_close(&output.read_f32()?, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_backend_executes_mxfp4_quantized_matvec_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_reference_vector();
        let row_a = sample_mxfp4_row(4);
        let row_b = sample_mxfp4_row(6);
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlMxfp4, &row_a)?,
            quantized_row_dot(&input, QuantizationMode::GgmlMxfp4, &row_b)?,
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let actual =
            backend.quantized_matvec(&weights, QuantizationMode::GgmlMxfp4, 2, 32, &input)?;
        assert_close(&actual, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_backend_dequantizes_mxfp4_row_when_available() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let row_a = sample_mxfp4_row(4);
        let row_b = sample_mxfp4_row(6);
        let mut expected = Vec::new();
        decode_quantized_row_into(QuantizationMode::GgmlMxfp4, &row_b, &mut expected)?;
        let mut bytes = row_a;
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let decode_params = backend.byte_buffer(&i32_slice_to_bytes(&[0_i32, 0_i32, 1_i32]))?;
        let output = backend.f32_buffer(32)?;
        let mut submission = backend.begin_submission()?;
        submission.dequantize_row_to_f32(
            &weights,
            QuantizationMode::GgmlMxfp4,
            2,
            17,
            32,
            &decode_params,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);
        assert_close(&output.read_f32()?, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_backend_gathers_f16_row_into_f32_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        let row_a = sample_reference_vector();
        let row_b = sample_q8_1_exact_vector();
        let mut bytes =
            Vec::with_capacity((row_a.len() + row_b.len()) * std::mem::size_of::<u16>());
        for value in row_a.iter().chain(row_b.iter()) {
            bytes.extend_from_slice(&f32_to_f16_bits(*value).to_le_bytes());
        }
        let input = backend.byte_buffer(&bytes)?;
        let decode_params = backend.byte_buffer(&i32_slice_to_bytes(&[0_i32, 0_i32, 1_i32]))?;
        let output = backend.f32_buffer(row_b.len())?;
        let mut submission = backend.begin_submission()?;
        submission.gather_f16_row_to_f32(&input, 2, row_b.len(), &decode_params, &output)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);
        assert_close(&output.read_f32()?, &row_b, 1e-3);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_argmax_when_available() -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = backend.input_buffer(
            Shape::new(vec![6]),
            vec![1.0_f32, -2.0, 4.25, 3.0, 9.5, 0.0],
        )?;
        let output = backend.byte_buffer(&vec![0_u8; std::mem::size_of::<i32>()])?;
        let mut submission = backend.begin_submission()?;
        submission.argmax_f32(&input, 1, 6, &output)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);

        let bytes = output.read_bytes()?;
        let argmax = i32::from_ne_bytes(bytes[..4].try_into().expect("argmax bytes"));
        assert_eq!(argmax, 4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_argmax_for_wide_rows_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let mut values = vec![-1.0_f32; 2049];
        values[1733] = 42.0;
        let input = backend.input_buffer(Shape::new(vec![values.len()]), values)?;
        let output = backend.byte_buffer(&vec![0_u8; std::mem::size_of::<i32>()])?;
        let mut submission = backend.begin_submission()?;
        submission.argmax_f32(&input, 1, 2049, &output)?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);

        let bytes = output.read_bytes()?;
        let argmax = i32::from_ne_bytes(bytes[..4].try_into().expect("argmax bytes"));
        assert_eq!(argmax, 1733);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_add_residual_rms_norm_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = vec![1.0_f32, -2.0, 3.0, 0.5];
        let residual = vec![0.25_f32, 0.5, -1.0, 2.0];
        let weight = vec![1.0_f32, 0.5, 2.0, 1.5];
        let combined = input
            .iter()
            .zip(residual.iter())
            .map(|(left, right)| left + right)
            .collect::<Vec<_>>();
        let mean_square =
            combined.iter().map(|value| value * value).sum::<f32>() / combined.len() as f32;
        let inv_rms = (mean_square + 1e-5_f32).sqrt().recip();
        let expected = combined
            .iter()
            .zip(weight.iter())
            .map(|(value, scale)| value * scale * inv_rms)
            .collect::<Vec<_>>();

        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let residual_buffer = backend.input_buffer(Shape::new(vec![residual.len()]), residual)?;
        let weight_buffer = backend.input_buffer(Shape::new(vec![weight.len()]), weight)?;
        let summed_output = backend.f32_buffer(expected.len())?;
        let normalized_output = backend.f32_buffer(expected.len())?;

        let mut submission = backend.begin_submission()?;
        submission.add_residual_rms_norm(
            &input_buffer,
            &residual_buffer,
            None,
            &weight_buffer,
            &summed_output,
            &normalized_output,
            expected.len(),
            1e-5,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);
        assert_close(&summed_output.read_f32()?, &combined, 1e-6);
        assert_close(&normalized_output.read_f32()?, &expected, 1e-5);
        Ok(())
    }

    #[test]
    fn cuda_submission_fused_attention_matches_separate_rope_attention_and_cache_path_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let head_count = 2usize;
        let kv_head_count = 1usize;
        let head_dim = 4usize;
        let rotary_dim = 4usize;
        let q_rows = head_count * head_dim;
        let k_rows = kv_head_count * head_dim;
        let v_rows = kv_head_count * head_dim;
        let qkv = vec![
            0.5_f32, -1.0, 0.75, 0.25, 1.25, 0.5, -0.5, 2.0, -0.25, 1.5, 0.75, -1.25, 0.5, 1.0,
            -0.5, 2.0,
        ];
        let freq_scale = 1.0_f32;
        let ext_factor = 0.0_f32;
        let corr_dims = [0.0_f32, rotary_dim as f32 - 1.0];
        let theta_scale = 10000.0_f32.powf(-2.0 / rotary_dim as f32);

        let qkv_separate = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv.clone())?;
        let qkv_fused = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv)?;
        let cache_keys_separate = backend.f32_buffer(k_rows)?;
        let cache_values_separate = backend.f32_buffer(v_rows)?;
        let cache_keys_fused = backend.f32_buffer(k_rows)?;
        let cache_values_fused = backend.f32_buffer(v_rows)?;
        let output_separate = backend.f32_buffer(q_rows)?;
        let output_fused = backend.f32_buffer(q_rows)?;

        let mut separate = backend.begin_submission()?;
        separate.rope_neox_in_place(
            &qkv_separate,
            0,
            head_count,
            head_dim,
            rotary_dim,
            3,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
        )?;
        separate.rope_neox_in_place(
            &qkv_separate,
            q_rows,
            kv_head_count,
            head_dim,
            rotary_dim,
            3,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
        )?;
        separate.attention_decode(
            &qkv_separate,
            0,
            &qkv_separate,
            q_rows,
            &qkv_separate,
            q_rows + k_rows,
            &cache_keys_separate,
            &cache_values_separate,
            k_rows,
            0,
            0,
            0,
            head_count,
            kv_head_count,
            head_dim,
            None,
            &output_separate,
        )?;
        separate.copy_buffer_region(
            &qkv_separate,
            q_rows * std::mem::size_of::<f32>(),
            &cache_keys_separate,
            0,
            k_rows * std::mem::size_of::<f32>(),
        )?;
        separate.copy_buffer_region(
            &qkv_separate,
            (q_rows + k_rows) * std::mem::size_of::<f32>(),
            &cache_values_separate,
            0,
            v_rows * std::mem::size_of::<f32>(),
        )?;
        let separate_report = separate.commit(CudaCommandWait::Completed)?;
        assert_eq!(separate_report.encoded_operations, 5);

        let mut fused = backend.begin_submission()?;
        fused.attention_decode_rope_cache(
            &qkv_fused,
            0,
            q_rows,
            q_rows + k_rows,
            &cache_keys_fused,
            &cache_values_fused,
            k_rows,
            0,
            0,
            0,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            3,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            None,
            &output_fused,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 1);

        assert_close(
            &output_fused.read_f32()?,
            &output_separate.read_f32()?,
            1e-5,
        );
        assert_close(
            &cache_keys_fused.read_f32()?,
            &cache_keys_separate.read_f32()?,
            1e-5,
        );
        assert_close(
            &cache_values_fused.read_f32()?,
            &cache_values_separate.read_f32()?,
            1e-5,
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_fused_attention_f16_kv_q8_1_matches_separate_attention_and_quantize_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let head_count = 2usize;
        let kv_head_count = 1usize;
        let head_dim = 32usize;
        let rotary_dim = 32usize;
        let q_rows = head_count * head_dim;
        let k_rows = kv_head_count * head_dim;
        let v_rows = kv_head_count * head_dim;
        let qkv = (0..(q_rows + k_rows + v_rows))
            .map(|index| ((index as f32 % 19.0) - 9.0) * 0.125)
            .collect::<Vec<_>>();
        let previous_keys = (0..k_rows)
            .map(|index| ((index as f32 % 13.0) - 6.0) * 0.2)
            .collect::<Vec<_>>();
        let previous_values = (0..v_rows)
            .map(|index| ((index as f32 % 11.0) - 5.0) * 0.15)
            .collect::<Vec<_>>();
        let cache_token_capacity = 2usize;
        let mut cache_keys_bytes =
            vec![0_u8; cache_token_capacity * k_rows * std::mem::size_of::<u16>()];
        let mut cache_values_bytes =
            vec![0_u8; cache_token_capacity * v_rows * std::mem::size_of::<u16>()];
        let previous_key_bytes = f32_slice_to_f16_le_bytes(&previous_keys);
        let previous_value_bytes = f32_slice_to_f16_le_bytes(&previous_values);
        cache_keys_bytes[..previous_key_bytes.len()].copy_from_slice(&previous_key_bytes);
        cache_values_bytes[..previous_value_bytes.len()].copy_from_slice(&previous_value_bytes);

        let qkv_separate = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv.clone())?;
        let qkv_fused = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv)?;
        let cache_keys_separate = backend.byte_buffer(&cache_keys_bytes)?;
        let cache_values_separate = backend.byte_buffer(&cache_values_bytes)?;
        let cache_keys_fused = backend.byte_buffer(&cache_keys_bytes)?;
        let cache_values_fused = backend.byte_buffer(&cache_values_bytes)?;
        let output_separate = backend.f32_buffer(q_rows)?;
        let output_q8_1_separate =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, q_rows)?])?;
        let output_q8_1_fused =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, q_rows)?])?;
        let freq_scale = 1.0_f32;
        let ext_factor = 0.0_f32;
        let corr_dims = [0.0_f32, 0.0_f32];
        let theta_scale = 0.5_f32;

        let mut separate = backend.begin_submission()?;
        separate.attention_decode_rope_cache_f16_kv(
            &qkv_separate,
            0,
            q_rows,
            q_rows + k_rows,
            &cache_keys_separate,
            &cache_values_separate,
            k_rows,
            0,
            1,
            0,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            5,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            None,
            &output_separate,
        )?;
        separate.quantize_f32_to_q8_1(&output_separate, 1, q_rows, &output_q8_1_separate)?;
        let separate_report = separate.commit(CudaCommandWait::Completed)?;
        assert_eq!(separate_report.encoded_operations, 2);

        let mut fused = backend.begin_submission()?;
        fused.attention_decode_rope_cache_f16_kv_q8_1(
            &qkv_fused,
            0,
            q_rows,
            q_rows + k_rows,
            &cache_keys_fused,
            &cache_values_fused,
            k_rows,
            0,
            1,
            0,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            5,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            None,
            &output_q8_1_fused,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 1);

        assert_eq!(
            output_q8_1_fused.read_bytes()?,
            output_q8_1_separate.read_bytes()?
        );
        assert_eq!(
            cache_keys_fused.read_bytes()?,
            cache_keys_separate.read_bytes()?
        );
        assert_eq!(
            cache_values_fused.read_bytes()?,
            cache_values_separate.read_bytes()?
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_fused_attention_graph_f16_kv_q8_1_matches_separate_attention_and_quantize_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let head_count = 2usize;
        let kv_head_count = 1usize;
        let head_dim = 32usize;
        let rotary_dim = 32usize;
        let q_rows = head_count * head_dim;
        let k_rows = kv_head_count * head_dim;
        let v_rows = kv_head_count * head_dim;
        let qkv = (0..(q_rows + k_rows + v_rows))
            .map(|index| ((index as f32 % 23.0) - 11.0) * 0.11)
            .collect::<Vec<_>>();
        let previous_keys = (0..k_rows)
            .map(|index| ((index as f32 % 17.0) - 8.0) * 0.14)
            .collect::<Vec<_>>();
        let previous_values = (0..v_rows)
            .map(|index| ((index as f32 % 7.0) - 3.0) * 0.21)
            .collect::<Vec<_>>();
        let cache_token_capacity = 2usize;
        let mut cache_keys_bytes =
            vec![0_u8; cache_token_capacity * k_rows * std::mem::size_of::<u16>()];
        let mut cache_values_bytes =
            vec![0_u8; cache_token_capacity * v_rows * std::mem::size_of::<u16>()];
        let previous_key_bytes = f32_slice_to_f16_le_bytes(&previous_keys);
        let previous_value_bytes = f32_slice_to_f16_le_bytes(&previous_values);
        cache_keys_bytes[..previous_key_bytes.len()].copy_from_slice(&previous_key_bytes);
        cache_values_bytes[..previous_value_bytes.len()].copy_from_slice(&previous_value_bytes);

        let qkv_separate = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv.clone())?;
        let qkv_fused = backend.input_buffer(Shape::new(vec![qkv.len()]), qkv)?;
        let cache_keys_separate = backend.byte_buffer(&cache_keys_bytes)?;
        let cache_values_separate = backend.byte_buffer(&cache_values_bytes)?;
        let cache_keys_fused = backend.byte_buffer(&cache_keys_bytes)?;
        let cache_values_fused = backend.byte_buffer(&cache_values_bytes)?;
        let decode_params = backend.byte_buffer(&i32_slice_to_bytes(&[1_i32, 6_i32]))?;
        let output_separate = backend.f32_buffer(q_rows)?;
        let output_q8_1_separate =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, q_rows)?])?;
        let output_q8_1_fused =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, q_rows)?])?;
        let freq_scale = 1.0_f32;
        let ext_factor = 0.0_f32;
        let corr_dims = [0.0_f32, 0.0_f32];
        let theta_scale = 0.5_f32;

        let mut separate = backend.begin_submission()?;
        separate.attention_decode_rope_cache_f16_kv_graph(
            &qkv_separate,
            0,
            q_rows,
            q_rows + k_rows,
            &cache_keys_separate,
            &cache_values_separate,
            k_rows,
            0,
            &decode_params,
            0,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            None,
            &output_separate,
        )?;
        separate.quantize_f32_to_q8_1(&output_separate, 1, q_rows, &output_q8_1_separate)?;
        let separate_report = separate.commit(CudaCommandWait::Completed)?;
        assert_eq!(separate_report.encoded_operations, 2);

        let mut fused = backend.begin_submission()?;
        fused.attention_decode_rope_cache_f16_kv_graph_q8_1(
            &qkv_fused,
            0,
            q_rows,
            q_rows + k_rows,
            &cache_keys_fused,
            &cache_values_fused,
            k_rows,
            0,
            &decode_params,
            0,
            head_count,
            kv_head_count,
            head_dim,
            rotary_dim,
            freq_scale,
            ext_factor,
            corr_dims,
            theta_scale,
            None,
            &output_q8_1_fused,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 1);

        assert_eq!(
            output_q8_1_fused.read_bytes()?,
            output_q8_1_separate.read_bytes()?
        );
        assert_eq!(
            cache_keys_fused.read_bytes()?,
            cache_keys_separate.read_bytes()?
        );
        assert_eq!(
            cache_values_fused.read_bytes()?,
            cache_values_separate.read_bytes()?
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_fused_add_residual_rms_norm_q8_1_router_topk_matches_separate_kernels_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let element_count = 32usize;
        let expert_count = 4usize;
        let top_k = 2usize;
        let input = (0..element_count)
            .map(|index| ((index as f32 % 9.0) - 4.0) * 0.35)
            .collect::<Vec<_>>();
        let residual = (0..element_count)
            .map(|index| ((index as f32 % 11.0) - 5.0) * 0.2)
            .collect::<Vec<_>>();
        let input_bias = (0..element_count)
            .map(|index| ((index as f32 % 7.0) - 3.0) * 0.05)
            .collect::<Vec<_>>();
        let weight = (0..element_count)
            .map(|index| 0.8 + index as f32 * 0.01)
            .collect::<Vec<_>>();
        let router_weights = (0..expert_count)
            .flat_map(|expert| {
                (0..element_count).map(move |index| {
                    ((expert as f32 + 1.0) * 0.09) + ((index as f32 % 13.0) - 6.0) * 0.015
                })
            })
            .collect::<Vec<_>>();
        let router_bias = vec![0.25_f32, -0.5_f32, 0.1_f32, -0.15_f32];

        let input_buffer = backend.input_buffer(Shape::new(vec![element_count]), input)?;
        let residual_buffer = backend.input_buffer(Shape::new(vec![element_count]), residual)?;
        let input_bias_buffer =
            backend.input_buffer(Shape::new(vec![element_count]), input_bias)?;
        let weight_buffer = backend.input_buffer(Shape::new(vec![element_count]), weight)?;
        let router_weights_buffer = backend.input_buffer(
            Shape::new(vec![expert_count, element_count]),
            router_weights,
        )?;
        let router_bias_buffer =
            backend.input_buffer(Shape::new(vec![expert_count]), router_bias)?;
        let q8_1_bytes = crate::ggml_q8_1_storage_bytes(1, element_count)?;

        let summed_separate = backend.f32_buffer(element_count)?;
        let normalized_separate = backend.f32_buffer(element_count)?;
        let quantized_separate = backend.byte_buffer(&vec![0_u8; q8_1_bytes])?;
        let selected_ids_separate = backend.byte_buffer(&vec![0_u8; top_k * size_of::<i32>()])?;
        let selected_weights_separate = backend.f32_buffer(top_k)?;

        let summed_fused = backend.f32_buffer(element_count)?;
        let normalized_fused = backend.f32_buffer(element_count)?;
        let quantized_fused = backend.byte_buffer(&vec![0_u8; q8_1_bytes])?;
        let selected_ids_fused = backend.byte_buffer(&vec![0_u8; top_k * size_of::<i32>()])?;
        let selected_weights_fused = backend.f32_buffer(top_k)?;

        let mut separate = backend.begin_submission()?;
        separate.add_residual_rms_norm_q8_1(
            &input_buffer,
            &residual_buffer,
            Some(&input_bias_buffer),
            &weight_buffer,
            &summed_separate,
            &normalized_separate,
            &quantized_separate,
            element_count,
            1.0e-5,
        )?;
        separate.router_topk_softmax(
            &router_weights_buffer,
            Some(&router_bias_buffer),
            &normalized_separate,
            expert_count,
            element_count,
            top_k,
            &selected_ids_separate,
            &selected_weights_separate,
        )?;
        let separate_report = separate.commit(CudaCommandWait::Completed)?;
        assert_eq!(separate_report.encoded_operations, 2);

        let mut fused = backend.begin_submission()?;
        fused.add_residual_rms_norm_q8_1_router_topk(
            &input_buffer,
            &residual_buffer,
            Some(&input_bias_buffer),
            &weight_buffer,
            &summed_fused,
            &normalized_fused,
            &quantized_fused,
            &router_weights_buffer,
            Some(&router_bias_buffer),
            expert_count,
            top_k,
            &selected_ids_fused,
            &selected_weights_fused,
            element_count,
            1.0e-5,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 1);

        assert_close(
            &summed_fused.read_f32()?,
            &summed_separate.read_f32()?,
            1e-5,
        );
        assert_close(
            &normalized_fused.read_f32()?,
            &normalized_separate.read_f32()?,
            1e-5,
        );
        assert_eq!(
            quantized_fused.read_bytes()?,
            quantized_separate.read_bytes()?
        );
        assert_eq!(
            selected_ids_fused.read_bytes()?,
            selected_ids_separate.read_bytes()?
        );
        assert_close(
            &selected_weights_fused.read_f32()?,
            &selected_weights_separate.read_f32()?,
            1e-6,
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_router_topk_delayed_softmax_matches_fused_router_kernel_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let element_count = 2880usize;
        let expert_count = 32usize;
        let top_k = 4usize;
        let input = (0..element_count)
            .map(|index| ((index as f32 % 17.0) - 8.0) * 0.03125)
            .collect::<Vec<_>>();
        let router_weights = (0..expert_count)
            .flat_map(|expert| {
                (0..element_count).map(move |index| {
                    ((expert as f32 + 1.0) * 0.0015) + (((index as f32 % 29.0) - 14.0) * 0.00075)
                })
            })
            .collect::<Vec<_>>();
        let mut router_weights_transposed = vec![0.0_f32; router_weights.len()];
        for expert in 0..expert_count {
            let row_start = expert * element_count;
            let row = &router_weights[row_start..row_start + element_count];
            for (column, value) in row.iter().copied().enumerate() {
                router_weights_transposed[column * expert_count + expert] = value;
            }
        }
        let router_bias = (0..expert_count)
            .map(|expert| ((expert as f32 % 5.0) - 2.0) * 0.0625)
            .collect::<Vec<_>>();

        let input_buffer = backend.input_buffer(Shape::new(vec![element_count]), input)?;
        let router_weights_buffer = backend.input_buffer(
            Shape::new(vec![expert_count, element_count]),
            router_weights,
        )?;
        let router_weights_transposed_buffer = backend.input_buffer(
            Shape::new(vec![element_count, expert_count]),
            router_weights_transposed,
        )?;
        let router_bias_buffer =
            backend.input_buffer(Shape::new(vec![expert_count]), router_bias)?;
        let selected_ids_fused = backend.byte_buffer(&vec![0_u8; top_k * size_of::<i32>()])?;
        let selected_weights_fused = backend.f32_buffer(top_k)?;
        let router_logits = backend.f32_buffer(expert_count)?;
        let selected_ids_split = backend.byte_buffer(&vec![0_u8; top_k * size_of::<i32>()])?;
        let selected_weights_split = backend.f32_buffer(top_k)?;

        let mut fused = backend.begin_submission()?;
        fused.router_topk_softmax(
            &router_weights_buffer,
            Some(&router_bias_buffer),
            &input_buffer,
            expert_count,
            element_count,
            top_k,
            &selected_ids_fused,
            &selected_weights_fused,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 1);

        let mut split = backend.begin_submission()?;
        split.matmul(
            &input_buffer,
            &router_weights_transposed_buffer,
            &router_logits,
            1,
            element_count,
            expert_count,
        )?;
        split.add_f32_in_place(&router_logits, 0, &router_bias_buffer, expert_count)?;
        split.router_topk_delayed_softmax(
            &router_logits,
            expert_count,
            top_k,
            &selected_ids_split,
            &selected_weights_split,
        )?;
        let split_report = split.commit(CudaCommandWait::Completed)?;
        assert_eq!(split_report.encoded_operations, 3);

        assert_eq!(
            selected_ids_split.read_bytes()?,
            selected_ids_fused.read_bytes()?
        );
        assert_close(
            &selected_weights_split.read_f32()?,
            &selected_weights_fused.read_f32()?,
            1e-6,
        );
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_q8_0_quantized_matvec_q8_1_fast_path_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector();
        let row_a = sample_q8_0_row(0.25, 1);
        let row_b = sample_q8_0_row(0.5, -1);
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_a)?,
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_b)?,
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let q8_1_buffer =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 32)?])?;
        let output = backend.f32_buffer(2)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 32, &q8_1_buffer)?;
        submission.quantized_matvec_q8_1(
            &weights,
            0,
            QuantizationMode::GgmlQ8_0,
            2,
            32,
            &q8_1_buffer,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);
        assert_close(&output.read_f32()?, &expected, 5e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_q8_0_quantized_matvec_q8_1_fast_path_with_bias_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector();
        let row_a = sample_q8_0_row(0.25, 1);
        let row_b = sample_q8_0_row(0.5, -1);
        let bias = vec![0.75_f32, -1.25_f32];
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_a)? + bias[0],
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_b)? + bias[1],
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let bias_buffer = backend.input_buffer(Shape::new(vec![bias.len()]), bias)?;
        let q8_1_buffer =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 32)?])?;
        let output = backend.f32_buffer(2)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 32, &q8_1_buffer)?;
        submission.quantized_matvec_q8_1(
            &weights,
            0,
            QuantizationMode::GgmlQ8_0,
            2,
            32,
            &q8_1_buffer,
            Some(&bias_buffer),
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);
        assert_close(&output.read_f32()?, &expected, 5e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_q8_0_quantized_matvec_q8_1_argmax_fast_path_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector();
        let row_a = sample_q8_0_row(0.25, 1);
        let row_b = sample_q8_0_row(0.5, -1);
        let bias = vec![0.75_f32, -1.25_f32];
        let expected = [
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_a)? + bias[0],
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_b)? + bias[1],
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let bias_buffer = backend.input_buffer(Shape::new(vec![bias.len()]), bias)?;
        let q8_1_buffer =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 32)?])?;
        let mut argmax_output = backend.byte_buffer(&vec![0_u8; std::mem::size_of::<u64>()])?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 32, &q8_1_buffer)?;
        argmax_output.write_bytes(
            &(((u64::from(i32::MAX as u32)) << 32) | u64::from(f32::NEG_INFINITY.to_bits()))
                .to_ne_bytes(),
        )?;
        submission.quantized_matvec_q8_1_argmax(
            &weights,
            0,
            QuantizationMode::GgmlQ8_0,
            2,
            32,
            &q8_1_buffer,
            Some(&bias_buffer),
            &argmax_output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);

        let packed_bytes = argmax_output.read_bytes()?;
        let packed = u64::from_ne_bytes(
            packed_bytes[..std::mem::size_of::<u64>()]
                .try_into()
                .expect("packed argmax buffer should be eight bytes"),
        );
        let actual_index = (packed >> 32) as usize;
        let expected_index = if expected[0] >= expected[1] { 0 } else { 1 };
        assert_eq!(actual_index, expected_index);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_mxfp4_quantized_matvec_q8_1_fast_path_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector();
        let row_a = sample_mxfp4_row(4);
        let row_b = sample_mxfp4_row(6);
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlMxfp4, &row_a)?,
            quantized_row_dot(&input, QuantizationMode::GgmlMxfp4, &row_b)?,
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let q8_1_buffer =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 32)?])?;
        let output = backend.f32_buffer(2)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 32, &q8_1_buffer)?;
        submission.quantized_matvec_q8_1(
            &weights,
            0,
            QuantizationMode::GgmlMxfp4,
            2,
            32,
            &q8_1_buffer,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);
        assert_close(&output.read_f32()?, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_q8_0_quantized_matvec_q8_1_fast_path_for_multi_block_rows_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector_64();
        let row_a = sample_q8_0_row(0.25, 1)
            .into_iter()
            .chain(sample_q8_0_row(0.5, -1))
            .collect::<Vec<_>>();
        let row_b = sample_q8_0_row(0.125, -1)
            .into_iter()
            .chain(sample_q8_0_row(0.375, 1))
            .collect::<Vec<_>>();
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_a)?,
            quantized_row_dot(&input, QuantizationMode::GgmlQ8_0, &row_b)?,
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let q8_1_buffer =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 64)?])?;
        let output = backend.f32_buffer(2)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 64, &q8_1_buffer)?;
        submission.quantized_matvec_q8_1(
            &weights,
            0,
            QuantizationMode::GgmlQ8_0,
            2,
            64,
            &q8_1_buffer,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);
        assert_close(&output.read_f32()?, &expected, 5e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_mxfp4_quantized_matvec_q8_1_fast_path_for_multi_block_rows_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector_64();
        let row_a = sample_mxfp4_row(4)
            .into_iter()
            .chain(sample_mxfp4_row(5))
            .collect::<Vec<_>>();
        let row_b = sample_mxfp4_row(6)
            .into_iter()
            .chain(sample_mxfp4_row(7))
            .collect::<Vec<_>>();
        let expected = vec![
            quantized_row_dot(&input, QuantizationMode::GgmlMxfp4, &row_a)?,
            quantized_row_dot(&input, QuantizationMode::GgmlMxfp4, &row_b)?,
        ];
        let mut bytes = row_a.clone();
        bytes.extend_from_slice(&row_b);
        let weights = backend.byte_buffer(&bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input)?;
        let q8_1_buffer =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 64)?])?;
        let output = backend.f32_buffer(2)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 64, &q8_1_buffer)?;
        submission.quantized_matvec_q8_1(
            &weights,
            0,
            QuantizationMode::GgmlMxfp4,
            2,
            64,
            &q8_1_buffer,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);
        assert_close(&output.read_f32()?, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_mxfp4_moe_gate_up_swiglu_q8_1_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector();
        let gate_rows = 32usize;
        let up_rows = 32usize;
        let rows_per_expert = gate_rows + up_rows;
        let selected_ids = [1_i32, 0_i32];
        let selected_count = selected_ids.len();
        let weights_bytes = sample_mxfp4_expert_gate_up_weights(2, gate_rows, up_rows);
        let weights = backend.byte_buffer(&weights_bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input.clone())?;
        let input_q8_1 =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 32)?])?;
        let selected_ids_buffer = backend.byte_buffer(&i32_slice_to_bytes(&selected_ids))?;
        let output = backend.f32_buffer(selected_count * gate_rows)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(&input_buffer, 1, 32, &input_q8_1)?;
        submission.moe_gate_up_swiglu_q8_1(
            &weights,
            QuantizationMode::GgmlMxfp4,
            17,
            rows_per_expert,
            32,
            gate_rows,
            up_rows,
            &selected_ids_buffer,
            selected_count,
            &input_q8_1,
            None,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);

        let actual = output.read_f32()?;
        let expected = expected_moe_gate_up_outputs(
            &input,
            &weights_bytes,
            selected_ids.as_slice(),
            gate_rows,
            up_rows,
        )?;
        assert_close(&actual, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_mxfp4_expert_gate_up_swiglu_q8_1_ids_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let input = sample_q8_1_exact_vector();
        let gate_rows = 32usize;
        let up_rows = 32usize;
        let rows_per_expert = gate_rows + up_rows;
        let selected_ids = [3_i32, 1_i32, 0_i32, 2_i32];
        let selected_count = selected_ids.len();
        let weights_bytes = sample_mxfp4_expert_gate_up_weights(4, gate_rows, up_rows);
        let weights = backend.byte_buffer(&weights_bytes)?;
        let input_buffer = backend.input_buffer(Shape::new(vec![input.len()]), input.clone())?;
        let input_q8_1 =
            backend.byte_buffer(&vec![0_u8; crate::ggml_q8_1_storage_bytes(1, 32)?])?;
        let selected_ids_buffer = backend.byte_buffer(&i32_slice_to_bytes(&selected_ids))?;
        let output_f32 = backend.f32_buffer(selected_count * gate_rows)?;
        let output_q8_1 = backend.byte_buffer(&vec![
            0_u8;
            crate::ggml_q8_1_storage_bytes(
                selected_count,
                gate_rows
            )?
        ])?;
        let separate_q8_1 = backend.byte_buffer(&vec![
            0_u8;
            crate::ggml_q8_1_storage_bytes(
                selected_count,
                gate_rows
            )?
        ])?;

        let mut separate = backend.begin_submission()?;
        separate.quantize_f32_to_q8_1(&input_buffer, 1, 32, &input_q8_1)?;
        separate.moe_gate_up_swiglu_q8_1(
            &weights,
            QuantizationMode::GgmlMxfp4,
            17,
            rows_per_expert,
            32,
            gate_rows,
            up_rows,
            &selected_ids_buffer,
            selected_count,
            &input_q8_1,
            None,
            None,
            &output_f32,
        )?;
        separate.quantize_f32_to_q8_1(&output_f32, selected_count, gate_rows, &separate_q8_1)?;
        let separate_report = separate.commit(CudaCommandWait::Completed)?;
        assert_eq!(separate_report.encoded_operations, 3);

        let mut fused = backend.begin_submission()?;
        fused.quantize_f32_to_q8_1(&input_buffer, 1, 32, &input_q8_1)?;
        fused.expert_gate_up_swiglu_q8_1_ids(
            &weights,
            QuantizationMode::GgmlMxfp4,
            17,
            rows_per_expert,
            32,
            gate_rows,
            up_rows,
            &selected_ids_buffer,
            selected_count,
            &input_q8_1,
            None,
            None,
            &output_q8_1,
        )?;
        let fused_report = fused.commit(CudaCommandWait::Completed)?;
        assert_eq!(fused_report.encoded_operations, 2);

        assert_eq!(output_q8_1.read_bytes()?, separate_q8_1.read_bytes()?);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_mxfp4_moe_down_aggregate_q8_1_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let selected_ids = [1_i32, 0_i32];
        let selected_weights = [0.75_f32, 0.25_f32];
        let activated = sample_selected_activated_vectors();
        let weights_bytes = sample_mxfp4_expert_down_weights(2, 32, 32);
        let weights = backend.byte_buffer(&weights_bytes)?;
        let activated_buffer =
            backend.input_buffer(Shape::new(vec![activated.len()]), activated.clone())?;
        let activated_q8_1 =
            backend.byte_buffer(&vec![
                0_u8;
                crate::ggml_q8_1_storage_bytes(selected_ids.len(), 32)?
            ])?;
        let selected_ids_buffer = backend.byte_buffer(&i32_slice_to_bytes(&selected_ids))?;
        let selected_weights_buffer = backend.input_buffer(
            Shape::new(vec![selected_weights.len()]),
            selected_weights.to_vec(),
        )?;
        let output = backend.f32_buffer(32)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(
            &activated_buffer,
            selected_ids.len(),
            32,
            &activated_q8_1,
        )?;
        submission.moe_down_aggregate_q8_1(
            &weights,
            QuantizationMode::GgmlMxfp4,
            17,
            32,
            32,
            &selected_ids_buffer,
            &selected_weights_buffer,
            selected_ids.len(),
            &activated_q8_1,
            None,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);

        let actual = output.read_f32()?;
        let expected = expected_moe_down_outputs(
            &activated,
            &weights_bytes,
            selected_ids.as_slice(),
            selected_weights.as_slice(),
            32,
        )?;
        assert_close(&actual, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_mxfp4_moe_down_aggregate_q8_1_f32_selected4_path_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let selected_ids = [3_i32, 1_i32, 0_i32, 2_i32];
        let selected_weights = [0.40_f32, 0.27_f32, 0.19_f32, 0.14_f32];
        let activated = sample_selected_activated_vectors_for_count(selected_ids.len());
        let weights_bytes = sample_mxfp4_expert_down_weights(selected_ids.len(), 32, 32);
        let weights = backend.byte_buffer(&weights_bytes)?;
        let activated_buffer =
            backend.input_buffer(Shape::new(vec![activated.len()]), activated.clone())?;
        let selected_ids_buffer = backend.byte_buffer(&i32_slice_to_bytes(&selected_ids))?;
        let selected_weights_buffer = backend.input_buffer(
            Shape::new(vec![selected_weights.len()]),
            selected_weights.to_vec(),
        )?;
        let output = backend.f32_buffer(32)?;
        let mut submission = backend.begin_submission()?;
        submission.moe_down_aggregate_q8_1_f32(
            &weights,
            QuantizationMode::GgmlMxfp4,
            17,
            32,
            32,
            &selected_ids_buffer,
            &selected_weights_buffer,
            selected_ids.len(),
            &activated_buffer,
            None,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 1);

        let actual = output.read_f32()?;
        let expected = expected_moe_down_outputs(
            &activated,
            &weights_bytes,
            selected_ids.as_slice(),
            selected_weights.as_slice(),
            32,
        )?;
        assert_close(&actual, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_mxfp4_moe_down_aggregate_q8_1_grouped_selected_path_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let selected_ids = [4_i32, 1_i32, 3_i32, 0_i32, 2_i32];
        let selected_weights = [0.30_f32, 0.24_f32, 0.18_f32, 0.16_f32, 0.12_f32];
        let activated = sample_selected_activated_vectors_for_count(selected_ids.len());
        let weights_bytes = sample_mxfp4_expert_down_weights(selected_ids.len(), 32, 32);
        let weights = backend.byte_buffer(&weights_bytes)?;
        let activated_buffer =
            backend.input_buffer(Shape::new(vec![activated.len()]), activated.clone())?;
        let activated_q8_1 =
            backend.byte_buffer(&vec![
                0_u8;
                crate::ggml_q8_1_storage_bytes(selected_ids.len(), 32)?
            ])?;
        let selected_ids_buffer = backend.byte_buffer(&i32_slice_to_bytes(&selected_ids))?;
        let selected_weights_buffer = backend.input_buffer(
            Shape::new(vec![selected_weights.len()]),
            selected_weights.to_vec(),
        )?;
        let output = backend.f32_buffer(32)?;
        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(
            &activated_buffer,
            selected_ids.len(),
            32,
            &activated_q8_1,
        )?;
        submission.moe_down_aggregate_q8_1(
            &weights,
            QuantizationMode::GgmlMxfp4,
            17,
            32,
            32,
            &selected_ids_buffer,
            &selected_weights_buffer,
            selected_ids.len(),
            &activated_q8_1,
            None,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 2);

        let actual = output.read_f32()?;
        let expected = expected_moe_down_outputs(
            &activated,
            &weights_bytes,
            selected_ids.as_slice(),
            selected_weights.as_slice(),
            32,
        )?;
        assert_close(&actual, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_submission_executes_mxfp4_grouped_expert_matvec_and_accumulate_when_available()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = CudaBackend::new();
        let Some(_selected) = backend.selected_device().cloned() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return Ok(());
        };
        if !backend.quantized_kernels_available() {
            return Ok(());
        }

        let selected_ids = [3_i32, 1_i32, 0_i32, 2_i32];
        let selected_weights = [0.40_f32, 0.27_f32, 0.19_f32, 0.14_f32];
        let activated = sample_selected_activated_vectors_for_count(selected_ids.len());
        let weights_bytes = sample_mxfp4_expert_down_weights(selected_ids.len(), 32, 32);
        let weights = backend.byte_buffer(&weights_bytes)?;
        let activated_buffer =
            backend.input_buffer(Shape::new(vec![activated.len()]), activated.clone())?;
        let activated_q8_1 =
            backend.byte_buffer(&vec![
                0_u8;
                crate::ggml_q8_1_storage_bytes(selected_ids.len(), 32)?
            ])?;
        let selected_ids_buffer = backend.byte_buffer(&i32_slice_to_bytes(&selected_ids))?;
        let selected_weights_buffer = backend.input_buffer(
            Shape::new(vec![selected_weights.len()]),
            selected_weights.to_vec(),
        )?;
        let projected = backend.f32_buffer(selected_ids.len() * 32)?;
        let output = backend.f32_buffer(32)?;

        let mut submission = backend.begin_submission()?;
        submission.quantize_f32_to_q8_1(
            &activated_buffer,
            selected_ids.len(),
            32,
            &activated_q8_1,
        )?;
        submission.expert_matvec_q8_1_ids(
            &weights,
            QuantizationMode::GgmlMxfp4,
            17,
            32,
            32,
            &selected_ids_buffer,
            selected_ids.len(),
            &activated_q8_1,
            None,
            &projected,
        )?;
        submission.accumulate_expert_outputs(
            &projected,
            &selected_weights_buffer,
            selected_ids.len(),
            32,
            None,
            &output,
        )?;
        let report = submission.commit(CudaCommandWait::Completed)?;
        assert_eq!(report.encoded_operations, 3);

        let actual_projected = projected.read_f32()?;
        let expected_projected = expected_moe_down_project_outputs(
            &activated,
            &weights_bytes,
            selected_ids.as_slice(),
            32,
        )?;
        assert_close(&actual_projected, &expected_projected, 1e-4);

        let actual = output.read_f32()?;
        let expected = expected_moe_down_outputs(
            &activated,
            &weights_bytes,
            selected_ids.as_slice(),
            selected_weights.as_slice(),
            32,
        )?;
        assert_close(&actual, &expected, 1e-4);
        Ok(())
    }

    #[test]
    fn cuda_backend_rejects_non_cuda_tensor_specs_when_available() {
        let mut backend = CudaBackend::new();
        let Some(_) = backend.selected_device() else {
            assert_eq!(backend.health().status, HealthStatus::Offline);
            return;
        };
        let spec = TensorSpec::new(Shape::new(vec![1]), DType::F32, Device::cpu());
        assert!(backend.allocate(&spec).is_err());
    }

    fn assert_close(actual: &[f32], expected: &[f32], tolerance: f32) {
        assert_eq!(actual.len(), expected.len());
        for (index, (actual, expected)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!(
                (actual - expected).abs() <= tolerance,
                "index {index}: actual={actual} expected={expected}"
            );
        }
    }

    fn sample_q8_0_row(scale: f32, multiplier: i8) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(34);
        bytes.extend_from_slice(&f32_to_f16_bits(scale).to_le_bytes());
        for index in 0_i8..32_i8 {
            bytes.push(index.saturating_mul(multiplier).to_le_bytes()[0]);
        }
        bytes
    }

    fn sample_reference_vector() -> Vec<f32> {
        (0..32).map(|index| (index as f32 + 1.0) * 0.25).collect()
    }

    fn sample_q8_1_exact_vector() -> Vec<f32> {
        let mut values = Vec::with_capacity(32);
        for index in 0..31_i32 {
            values.push(((index * 11) % 29 - 14) as f32);
        }
        values.push(127.0);
        values
    }

    fn sample_q8_1_exact_vector_64() -> Vec<f32> {
        sample_q8_1_exact_vector()
            .into_iter()
            .chain((0..31_i32).map(|index| ((index * 17) % 41 - 20) as f32))
            .chain(std::iter::once(127.0))
            .collect()
    }

    fn sample_mxfp4_row(scale_exponent: u8) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(17);
        bytes.push(scale_exponent);
        for pair in 0..16_u8 {
            let low = pair & 0x07;
            let high = 0x0f_u8.saturating_sub(pair & 0x07);
            bytes.push(low | (high << 4));
        }
        bytes
    }

    fn sample_mxfp4_expert_gate_up_weights(
        expert_count: usize,
        gate_rows: usize,
        up_rows: usize,
    ) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(expert_count * (gate_rows + up_rows) * 17);
        for expert_index in 0..expert_count {
            for row in 0..gate_rows {
                bytes.extend_from_slice(&sample_mxfp4_row(4 + ((expert_index + row) % 4) as u8));
            }
            for row in 0..up_rows {
                bytes.extend_from_slice(&sample_mxfp4_row(5 + ((expert_index + row) % 4) as u8));
            }
        }
        bytes
    }

    fn sample_mxfp4_expert_down_weights(
        expert_count: usize,
        rows: usize,
        columns: usize,
    ) -> Vec<u8> {
        assert_eq!(columns, 32);
        let mut bytes = Vec::with_capacity(expert_count * rows * 17);
        for expert_index in 0..expert_count {
            for row in 0..rows {
                bytes
                    .extend_from_slice(&sample_mxfp4_row(3 + ((expert_index * 3 + row) % 5) as u8));
            }
        }
        bytes
    }

    fn sample_selected_activated_vectors() -> Vec<f32> {
        sample_selected_activated_vectors_for_count(2)
    }

    fn sample_selected_activated_vectors_for_count(selected_count: usize) -> Vec<f32> {
        let mut values = Vec::with_capacity(selected_count.saturating_mul(32));
        for selected_index in 0..selected_count {
            if selected_index == 0 {
                values.extend(sample_q8_1_exact_vector());
                continue;
            }
            values.extend((0..31_i32).map(|index| {
                let step = (selected_index as i32 + 3) * 5;
                ((index * step) % 37 - 18) as f32
            }));
            values.push(127.0 - selected_index as f32);
        }
        values
    }

    fn expected_moe_gate_up_outputs(
        input: &[f32],
        weights: &[u8],
        selected_ids: &[i32],
        gate_rows: usize,
        up_rows: usize,
    ) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let rows_per_expert = gate_rows + up_rows;
        let row_byte_len = 17usize;
        let mut outputs = Vec::with_capacity(selected_ids.len() * gate_rows);
        for &selected_id in selected_ids {
            let expert_index = usize::try_from(selected_id)?;
            for row in 0..gate_rows {
                let gate_start = (expert_index * rows_per_expert + row) * row_byte_len;
                let up_start = (expert_index * rows_per_expert + gate_rows + row) * row_byte_len;
                let gate = quantized_row_dot(
                    input,
                    QuantizationMode::GgmlMxfp4,
                    &weights[gate_start..gate_start + row_byte_len],
                )?;
                let up = quantized_row_dot(
                    input,
                    QuantizationMode::GgmlMxfp4,
                    &weights[up_start..up_start + row_byte_len],
                )?;
                outputs.push(reference_oai_swiglu(gate, up));
            }
        }
        Ok(outputs)
    }

    fn expected_moe_down_outputs(
        activated: &[f32],
        weights: &[u8],
        selected_ids: &[i32],
        selected_weights: &[f32],
        rows: usize,
    ) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let projected = expected_moe_down_project_outputs(activated, weights, selected_ids, rows)?;
        let mut outputs = vec![0.0_f32; rows];
        for (selected_slot, &route) in selected_weights.iter().enumerate() {
            for row_index in 0..rows {
                outputs[row_index] += projected[selected_slot * rows + row_index] * route;
            }
        }
        Ok(outputs)
    }

    fn expected_moe_down_project_outputs(
        activated: &[f32],
        weights: &[u8],
        selected_ids: &[i32],
        rows: usize,
    ) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let row_byte_len = 17usize;
        let columns = 32usize;
        let mut outputs = vec![0.0_f32; selected_ids.len() * rows];
        for (selected_slot, &selected_id) in selected_ids.iter().enumerate() {
            let expert_index = usize::try_from(selected_id)?;
            let activated_row = &activated[selected_slot * columns..(selected_slot + 1) * columns];
            for row_index in 0..rows {
                let row_start = (expert_index * rows + row_index) * row_byte_len;
                outputs[selected_slot * rows + row_index] = quantized_row_dot(
                    activated_row,
                    QuantizationMode::GgmlMxfp4,
                    &weights[row_start..row_start + row_byte_len],
                )?;
            }
        }
        Ok(outputs)
    }

    fn i32_slice_to_bytes(values: &[i32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(values.len() * size_of::<i32>());
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    fn f32_slice_to_f16_le_bytes(values: &[f32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(values.len() * size_of::<u16>());
        for &value in values {
            bytes.extend_from_slice(&f32_to_f16_bits(value).to_le_bytes());
        }
        bytes
    }

    fn reference_oai_swiglu(gate: f32, up: f32) -> f32 {
        const LIMIT: f32 = 7.0;
        const ALPHA: f32 = 1.702;
        let x = gate.min(LIMIT);
        let y = up.clamp(-LIMIT, LIMIT);
        let out_glu = x / (1.0 + (ALPHA * -x).exp());
        out_glu * (y + 1.0)
    }

    fn f32_to_f16_bits(value: f32) -> u16 {
        let bits = value.to_bits();
        let sign = ((bits >> 16) & 0x8000) as u16;
        let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
        let mantissa = bits & 0x007f_ffff;
        if exponent <= 0 {
            return sign;
        }
        if exponent >= 0x1f {
            return sign | 0x7c00;
        }
        sign | ((exponent as u16) << 10) | ((mantissa >> 13) as u16)
    }
}
