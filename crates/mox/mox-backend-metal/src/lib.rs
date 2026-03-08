//! Metal backend discovery, allocation, submission, and minimal execution
//! surfaces for Mox.

#![allow(clippy::result_large_err)]

use std::{collections::BTreeMap, fmt};

use mox_compiler::compile_graph;
#[cfg(target_os = "macos")]
use mox_core::QuantizationMode;
use mox_core::{DType, DeviceKind, Shape, TensorData, TensorId, TensorSpec};
use mox_ir::{ExecutionOp, ExecutionPlan, ExecutionStep, Graph};
use mox_runtime::{
    Allocator, BackendName, BackendSelection, BufferHandle, DeviceDescriptor, DeviceDiscovery,
    ExecutionBackend, ExecutionMetrics, ExecutionResult, HealthStatus, RuntimeError, RuntimeHealth,
};
#[cfg(target_os = "macos")]
use mox_runtime::{QuantizationExecution, QuantizationLoadPath, QuantizationSupport};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "Metal backend discovery, allocation, and submission";

#[cfg(target_os = "macos")]
const MODERN_FAMILY_FLAG: &str = "family_modern";
#[cfg(target_os = "macos")]
const LEGACY_FAMILY_FLAG: &str = "family_legacy";

/// Exact plan surface currently supported for the first accelerated
/// `mox.embeddings` milestone.
pub const EMBEDDINGS_SUPPORTED_OPS: &[&str] = &["input", "constant", "matmul", "add"];

/// Metal buffer storage mode visible to Mox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetalStorageMode {
    /// Host-visible storage shared with the GPU.
    Shared,
    /// Host-visible managed storage that requires explicit GPU-to-host sync.
    Managed,
    /// GPU-private storage that is not host visible.
    Private,
}

/// How long Mox should wait after a Metal submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetalCommandWait {
    /// Commit and return immediately.
    None,
    /// Wait until the command buffer is scheduled.
    Scheduled,
    /// Wait until the command buffer is completed.
    Completed,
}

/// Stable command-buffer lifecycle state exposed by Mox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetalCommandStatus {
    /// The command buffer has not been enqueued yet.
    NotEnqueued,
    /// The command buffer is enqueued.
    Enqueued,
    /// The command buffer was committed.
    Committed,
    /// The command buffer is scheduled on the device.
    Scheduled,
    /// The command buffer completed successfully.
    Completed,
    /// The command buffer failed.
    Error,
}

/// Submission metadata returned after a command buffer is committed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetalSubmissionReport {
    /// Final command-buffer status observed by Mox.
    pub status: MetalCommandStatus,
    /// Number of explicit encoded operations recorded in the submission.
    pub encoded_operations: usize,
    /// Number of explicit GPU-to-host synchronizations encoded.
    pub synchronized_buffers: usize,
}

/// Metal-backed tensor buffer.
#[derive(Clone)]
pub struct MetalBuffer {
    spec: TensorSpec,
    byte_len: usize,
    storage_mode: MetalStorageMode,
    host_visible: bool,
    platform: platform::PlatformBuffer,
}

impl fmt::Debug for MetalBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MetalBuffer")
            .field("spec", &self.spec)
            .field("byte_len", &self.byte_len)
            .field("storage_mode", &self.storage_mode)
            .field("host_visible", &self.host_visible)
            .field("platform", &"<metal platform buffer>")
            .finish()
    }
}

impl MetalBuffer {
    /// Returns the backing allocation size in bytes.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// Returns the Metal storage mode backing the buffer.
    #[must_use]
    pub const fn storage_mode(&self) -> MetalStorageMode {
        self.storage_mode
    }

    /// Returns whether the CPU can map the backing storage directly.
    #[must_use]
    pub const fn host_visible(&self) -> bool {
        self.host_visible
    }

    /// Writes raw bytes into the host-visible buffer contents.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        if bytes.len() != self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "metal buffer write length mismatch: expected {}, actual {}",
                self.byte_len,
                bytes.len()
            )));
        }
        self.platform.write_bytes(bytes, self.storage_mode)
    }

    /// Reads raw bytes from the host-visible buffer contents.
    pub fn read_bytes(&self) -> Result<Vec<u8>, RuntimeError> {
        self.platform.read_bytes(self.byte_len)
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
                "metal buffer write length mismatch: expected {} values, actual {}",
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
}

impl BufferHandle for MetalBuffer {
    fn spec(&self) -> &TensorSpec {
        &self.spec
    }
}

/// Metal command submission that keeps synchronization explicit.
pub struct MetalSubmission {
    encoded_operations: usize,
    synchronized_buffers: usize,
    platform: platform::PlatformSubmission,
}

impl MetalSubmission {
    /// Fills a buffer with a constant byte value using a blit command.
    pub fn fill_buffer(&mut self, buffer: &MetalBuffer, value: u8) -> Result<(), RuntimeError> {
        self.platform
            .fill_buffer(&buffer.platform, buffer.byte_len, value)?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Copies one Metal buffer into another with explicit size checking.
    pub fn copy_buffer(
        &mut self,
        source: &MetalBuffer,
        destination: &MetalBuffer,
    ) -> Result<(), RuntimeError> {
        if source.byte_len != destination.byte_len {
            return Err(RuntimeError::Backend(format!(
                "metal buffer copy length mismatch: source {}, destination {}",
                source.byte_len, destination.byte_len
            )));
        }
        self.platform
            .copy_buffer(&source.platform, &destination.platform, source.byte_len)?;
        self.encoded_operations += 1;
        Ok(())
    }

    /// Encodes an explicit GPU-to-host synchronization for managed storage.
    pub fn synchronize_buffer(&mut self, buffer: &MetalBuffer) -> Result<(), RuntimeError> {
        if self
            .platform
            .synchronize_buffer(&buffer.platform, buffer.storage_mode)?
        {
            self.synchronized_buffers += 1;
        }
        Ok(())
    }

    /// Commits the submission and optionally waits for scheduling/completion.
    pub fn commit(self, wait: MetalCommandWait) -> Result<MetalSubmissionReport, RuntimeError> {
        let status = self.platform.commit(wait)?;
        Ok(MetalSubmissionReport {
            status,
            encoded_operations: self.encoded_operations,
            synchronized_buffers: self.synchronized_buffers,
        })
    }
}

/// Discovery report containing device descriptors and backend health.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetalDiscoveryReport {
    /// Discovered Metal devices.
    pub devices: Vec<DeviceDescriptor>,
    /// Backend health derived from discovery.
    pub health: RuntimeHealth,
}

#[cfg(any(test, target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeviceSupportTier {
    Modern,
    Legacy,
}

#[cfg(any(test, target_os = "macos"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct FamilySupport {
    common2: bool,
    common3: bool,
    mac1: bool,
    mac2: bool,
    metal3: bool,
    metal4: bool,
    apple: bool,
}

#[cfg(any(test, target_os = "macos"))]
fn classify_support(family: FamilySupport) -> DeviceSupportTier {
    if family.apple || family.common3 || family.metal3 || family.metal4 {
        DeviceSupportTier::Modern
    } else {
        DeviceSupportTier::Legacy
    }
}

enum MetalBackendState {
    Available(Box<AvailableMetalBackend>),
    Unavailable(RuntimeHealth),
}

struct AvailableMetalBackend {
    descriptor: DeviceDescriptor,
    platform: platform::ConfiguredBackend,
}

/// Metal backend discovery, allocation, and submission implementation.
pub struct MetalBackend {
    state: MetalBackendState,
}

impl Default for MetalBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MetalBackend {
    /// Creates a Metal backend and selects the first modern device when one is
    /// available.
    #[must_use]
    pub fn new() -> Self {
        match platform::configure_preferred_backend() {
            Ok(platform_backend) => {
                let descriptor = platform_backend.descriptor().clone();
                Self {
                    state: MetalBackendState::Available(Box::new(AvailableMetalBackend {
                        descriptor,
                        platform: platform_backend,
                    })),
                }
            }
            Err(health) => Self {
                state: MetalBackendState::Unavailable(health),
            },
        }
    }

    /// Returns the device selected for allocation/submission, when available.
    #[must_use]
    pub fn selected_device(&self) -> Option<&DeviceDescriptor> {
        match &self.state {
            MetalBackendState::Available(backend) => Some(&backend.descriptor),
            MetalBackendState::Unavailable(_) => None,
        }
    }

    /// Returns the current discovery report for the local machine.
    pub fn discovery_report(&self) -> Result<MetalDiscoveryReport, RuntimeError> {
        platform::discovery_report()
    }

    /// Creates a host-visible `f32` input buffer on the selected Metal device.
    pub fn input_buffer(
        &mut self,
        shape: Shape,
        values: impl Into<Vec<f32>>,
    ) -> Result<MetalBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "metal backend unavailable: no selected execution device",
            )));
        };
        let mut buffer = self.allocate(&TensorSpec::new(shape, DType::F32, device))?;
        buffer.write_f32(values.into().as_slice())?;
        Ok(buffer)
    }

    /// Compiles and executes a graph on the supported Metal embeddings surface.
    pub fn compile_and_execute(
        &mut self,
        graph: &Graph,
        inputs: &BTreeMap<TensorId, MetalBuffer>,
    ) -> Result<ExecutionResult<MetalBuffer>, RuntimeError> {
        let plan =
            compile_graph(graph).map_err(|error| RuntimeError::Backend(error.to_string()))?;
        self.execute(&plan, inputs)
    }

    /// Begins an explicit command submission on the selected Metal device.
    pub fn begin_submission(
        &self,
        label: impl Into<String>,
    ) -> Result<MetalSubmission, RuntimeError> {
        match &self.state {
            MetalBackendState::Available(backend) => Ok(MetalSubmission {
                encoded_operations: 0,
                synchronized_buffers: 0,
                platform: backend.platform.begin_submission(label.into())?,
            }),
            MetalBackendState::Unavailable(health) => Err(RuntimeError::Backend(format!(
                "metal backend unavailable: {}",
                health.message
            ))),
        }
    }

    /// Returns truthful backend-selection data for a supported Metal product path.
    pub fn backend_selection(
        &self,
        supported_ops: &[&str],
    ) -> Result<BackendSelection, RuntimeError> {
        match &self.state {
            MetalBackendState::Available(backend) => Ok(BackendSelection::direct(
                self.backend_name(),
                Some(backend.descriptor.clone()),
                supported_ops
                    .iter()
                    .map(|label| String::from(*label))
                    .collect(),
            )),
            MetalBackendState::Unavailable(health) => Err(RuntimeError::Backend(format!(
                "metal backend unavailable: {}",
                health.message
            ))),
        }
    }

    /// Returns an explicit fallback selection when Metal cannot execute the
    /// requested product path on the local machine.
    pub fn fallback_selection<B>(
        &self,
        fallback_backend: &B,
        supported_ops: &[&str],
    ) -> Result<BackendSelection, RuntimeError>
    where
        B: DeviceDiscovery + ?Sized,
    {
        match &self.state {
            MetalBackendState::Available(_) => self.backend_selection(supported_ops),
            MetalBackendState::Unavailable(health) => BackendSelection::fallback_to_backend(
                self.backend_name(),
                fallback_backend,
                supported_ops,
                format!("metal backend unavailable: {}", health.message),
            ),
        }
    }
}

impl DeviceDiscovery for MetalBackend {
    fn backend_name(&self) -> BackendName {
        "metal"
    }

    fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError> {
        self.discovery_report().map(|report| report.devices)
    }

    fn health(&self) -> RuntimeHealth {
        match self.discovery_report() {
            Ok(report) => report.health,
            Err(error) => RuntimeHealth {
                status: HealthStatus::Degraded,
                message: format!("metal discovery failed: {error}"),
            },
        }
    }
}

impl Allocator for MetalBackend {
    type Buffer = MetalBuffer;

    fn allocate(&mut self, spec: &TensorSpec) -> Result<Self::Buffer, RuntimeError> {
        match &mut self.state {
            MetalBackendState::Available(backend) => backend.allocate(spec),
            MetalBackendState::Unavailable(health) => Err(RuntimeError::Backend(format!(
                "metal backend unavailable: {}",
                health.message
            ))),
        }
    }
}

impl ExecutionBackend for MetalBackend {
    type Buffer = MetalBuffer;

    fn execute(
        &mut self,
        plan: &ExecutionPlan,
        inputs: &BTreeMap<TensorId, Self::Buffer>,
    ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError> {
        validate_supported_plan(plan)?;
        match &mut self.state {
            MetalBackendState::Available(backend) => backend.execute(plan, inputs),
            MetalBackendState::Unavailable(health) => Err(RuntimeError::Backend(format!(
                "metal backend unavailable: {}",
                health.message
            ))),
        }
    }
}

impl AvailableMetalBackend {
    fn allocate(&mut self, spec: &TensorSpec) -> Result<MetalBuffer, RuntimeError> {
        if spec.device().kind() != DeviceKind::Metal {
            return Err(RuntimeError::Backend(format!(
                "metal allocator requires a Metal tensor spec, actual device kind {}",
                spec.device().kind()
            )));
        }
        if spec.device().ordinal() != self.descriptor.device.ordinal() {
            return Err(RuntimeError::Backend(format!(
                "metal allocator requires device ordinal {}, actual {}",
                self.descriptor.device.ordinal(),
                spec.device().ordinal()
            )));
        }

        let byte_len = spec
            .storage_size()
            .checked_mul(size_of_dtype(spec.dtype()))
            .ok_or_else(|| RuntimeError::Backend(String::from("metal buffer size overflow")))?;
        let storage_mode = self.platform.storage_mode();
        Ok(MetalBuffer {
            spec: spec.clone(),
            byte_len,
            storage_mode,
            host_visible: matches!(
                storage_mode,
                MetalStorageMode::Shared | MetalStorageMode::Managed
            ),
            platform: self.platform.allocate_buffer(byte_len)?,
        })
    }

    fn buffer_from_tensor_data(
        &mut self,
        spec: &TensorSpec,
        data: &TensorData,
    ) -> Result<MetalBuffer, RuntimeError> {
        let mut buffer = self.allocate(spec)?;
        match data {
            TensorData::F32(values) => buffer.write_f32(values.as_slice())?,
        }
        Ok(buffer)
    }

    fn execute(
        &mut self,
        plan: &ExecutionPlan,
        inputs: &BTreeMap<TensorId, MetalBuffer>,
    ) -> Result<ExecutionResult<MetalBuffer>, RuntimeError> {
        let mut submission = MetalSubmission {
            encoded_operations: 0,
            synchronized_buffers: 0,
            platform: self
                .platform
                .begin_submission(String::from("mox.execute"))?,
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
                    self.platform.encode_add(
                        &mut submission.platform,
                        left,
                        right,
                        &output,
                        step.spec.element_count(),
                    )?;
                    submission.encoded_operations += 1;
                    values.insert(step.output, output);
                }
                ExecutionOp::Matmul => {
                    let (left, right) = binary_inputs(step, &values)?;
                    let output = self.allocate(&step.spec)?;
                    self.platform
                        .encode_matmul(&mut submission.platform, left, right, &output)?;
                    submission.encoded_operations += 1;
                    values.insert(step.output, output);
                }
                _ => {
                    return Err(RuntimeError::UnsupportedStep(step.op.label().to_string()));
                }
            }
        }

        for output_id in &plan.outputs {
            let Some(buffer) = values.get(output_id) else {
                return Err(RuntimeError::MissingInput(*output_id));
            };
            if self
                .platform
                .synchronize_output(&mut submission.platform, buffer)?
            {
                submission.synchronized_buffers += 1;
            }
        }

        let _report = submission.commit(MetalCommandWait::Completed)?;
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
            },
        })
    }
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
                    "metal input step {} unexpectedly has inputs",
                    step.output
                )));
            }
        }
        ExecutionOp::Constant { data } => {
            if data.as_f32_slice().len() != step.spec.storage_size() {
                return Err(RuntimeError::Backend(format!(
                    "metal constant {} payload length mismatch",
                    step.output
                )));
            }
        }
        ExecutionOp::Add => {
            if step.inputs.len() != 2 {
                return Err(RuntimeError::Backend(format!(
                    "metal add step {} requires two inputs",
                    step.output
                )));
            }
        }
        ExecutionOp::Matmul => {
            if step.inputs.len() != 2 {
                return Err(RuntimeError::Backend(format!(
                    "metal matmul step {} requires two inputs",
                    step.output
                )));
            }
            let dims = step.spec.shape().dims();
            if dims.len() != 2 {
                return Err(RuntimeError::Backend(format!(
                    "metal matmul step {} requires a rank-2 output, actual rank {}",
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
            "metal embeddings surface only supports F32 tensors, actual {:?}",
            spec.dtype()
        )));
    }
    if spec.device().kind() != DeviceKind::Metal {
        return Err(RuntimeError::Backend(format!(
            "metal embeddings surface requires Metal tensor specs, actual device kind {}",
            spec.device().kind()
        )));
    }
    if !spec.layout().is_contiguous() || spec.layout().offset() != 0 {
        return Err(RuntimeError::Backend(String::from(
            "metal embeddings surface requires contiguous zero-offset tensors",
        )));
    }
    Ok(())
}

fn binary_inputs<'a>(
    step: &ExecutionStep,
    values: &'a BTreeMap<TensorId, MetalBuffer>,
) -> Result<(&'a MetalBuffer, &'a MetalBuffer), RuntimeError> {
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
            "metal {} requires matching input specs",
            step.op.label()
        )));
    }
    Ok((left, right))
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ptr;

    use metal::{
        Buffer, CommandBuffer, CommandQueue, CompileOptions, ComputePipelineState,
        Device as MetalDevice, DeviceRef as MetalDeviceRef, MTLCommandBufferStatus,
        MTLDeviceLocation, MTLGPUFamily, MTLResourceOptions, MTLSize, NSRange,
    };
    use mox_core::{DType, Device, DeviceKind, QuantizationMode};
    use mox_runtime::{
        BufferHandle, DeviceDescriptor, HealthStatus, QuantizationExecution, QuantizationSupport,
        RuntimeError, RuntimeHealth,
    };

    use super::{
        DeviceSupportTier, FamilySupport, LEGACY_FAMILY_FLAG, MODERN_FAMILY_FLAG, MetalBuffer,
        MetalCommandStatus, MetalCommandWait, MetalDiscoveryReport, MetalStorageMode,
        classify_support,
    };

    #[derive(Clone)]
    pub(super) struct PlatformBuffer {
        raw: Buffer,
    }

    struct EmbeddingsPipelines {
        add: ComputePipelineState,
        matmul: ComputePipelineState,
    }

    impl PlatformBuffer {
        pub(super) fn write_bytes(
            &self,
            bytes: &[u8],
            storage_mode: MetalStorageMode,
        ) -> Result<(), RuntimeError> {
            let contents = self.raw.contents().cast::<u8>();
            if contents.is_null() {
                return Err(RuntimeError::Backend(String::from(
                    "metal buffer is not host visible",
                )));
            }
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), contents, bytes.len());
            }
            if matches!(storage_mode, MetalStorageMode::Managed) {
                self.raw.did_modify_range(byte_range(bytes.len())?);
            }
            Ok(())
        }

        pub(super) fn read_bytes(&self, byte_len: usize) -> Result<Vec<u8>, RuntimeError> {
            let contents = self.raw.contents().cast::<u8>();
            if contents.is_null() {
                return Err(RuntimeError::Backend(String::from(
                    "metal buffer is not host visible",
                )));
            }
            let mut bytes = vec![0u8; byte_len];
            unsafe {
                ptr::copy_nonoverlapping(contents, bytes.as_mut_ptr(), byte_len);
            }
            Ok(bytes)
        }
    }

    pub(super) struct PlatformSubmission {
        command_buffer: CommandBuffer,
    }

    impl PlatformSubmission {
        pub(super) fn fill_buffer(
            &mut self,
            buffer: &PlatformBuffer,
            byte_len: usize,
            value: u8,
        ) -> Result<(), RuntimeError> {
            let encoder = self.command_buffer.new_blit_command_encoder();
            encoder.fill_buffer(&buffer.raw, byte_range(byte_len)?, value);
            encoder.end_encoding();
            Ok(())
        }

        pub(super) fn copy_buffer(
            &mut self,
            source: &PlatformBuffer,
            destination: &PlatformBuffer,
            byte_len: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.command_buffer.new_blit_command_encoder();
            let size = to_metal_size(byte_len)?;
            encoder.copy_from_buffer(&source.raw, 0, &destination.raw, 0, size);
            encoder.end_encoding();
            Ok(())
        }

        pub(super) fn synchronize_buffer(
            &mut self,
            buffer: &PlatformBuffer,
            storage_mode: MetalStorageMode,
        ) -> Result<bool, RuntimeError> {
            if !matches!(storage_mode, MetalStorageMode::Managed) {
                return Ok(false);
            }
            let encoder = self.command_buffer.new_blit_command_encoder();
            encoder.synchronize_resource(&buffer.raw);
            encoder.end_encoding();
            Ok(true)
        }

        pub(super) fn encode_add(
            &mut self,
            pipeline: &ComputePipelineState,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.command_buffer.new_compute_command_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&left.raw), 0);
            encoder.set_buffer(1, Some(&right.raw), 0);
            encoder.set_buffer(2, Some(&output.raw), 0);

            let element_count = u32::try_from(element_count).map_err(|_| {
                RuntimeError::Backend(String::from("metal add element count overflow"))
            })?;
            encoder.set_bytes(3, 4, (&element_count as *const u32).cast());

            let threadgroup_size = compute_threadgroup_size(
                pipeline,
                usize::try_from(element_count).map_err(|_| {
                    RuntimeError::Backend(String::from(
                        "metal add element count conversion overflow",
                    ))
                })?,
            )?;
            encoder.dispatch_threads(
                MTLSize::new(u64::from(element_count), 1, 1),
                threadgroup_size,
            );
            encoder.end_encoding();
            Ok(())
        }

        pub(super) fn encode_matmul(
            &mut self,
            pipeline: &ComputePipelineState,
            left: &PlatformBuffer,
            right: &PlatformBuffer,
            output: &PlatformBuffer,
            m: usize,
            k: usize,
            n: usize,
        ) -> Result<(), RuntimeError> {
            let encoder = self.command_buffer.new_compute_command_encoder();
            encoder.set_compute_pipeline_state(pipeline);
            encoder.set_buffer(0, Some(&left.raw), 0);
            encoder.set_buffer(1, Some(&right.raw), 0);
            encoder.set_buffer(2, Some(&output.raw), 0);

            let m = u32::try_from(m)
                .map_err(|_| RuntimeError::Backend(String::from("metal matmul m overflow")))?;
            let k = u32::try_from(k)
                .map_err(|_| RuntimeError::Backend(String::from("metal matmul k overflow")))?;
            let n = u32::try_from(n)
                .map_err(|_| RuntimeError::Backend(String::from("metal matmul n overflow")))?;
            encoder.set_bytes(3, 4, (&m as *const u32).cast());
            encoder.set_bytes(4, 4, (&k as *const u32).cast());
            encoder.set_bytes(5, 4, (&n as *const u32).cast());

            let grid_width = u64::from(m)
                .checked_mul(u64::from(n))
                .ok_or_else(|| RuntimeError::Backend(String::from("metal matmul grid overflow")))?;
            let threadgroup_size = compute_threadgroup_size(
                pipeline,
                usize::try_from(grid_width).map_err(|_| {
                    RuntimeError::Backend(String::from("metal matmul grid conversion overflow"))
                })?,
            )?;
            encoder.dispatch_threads(MTLSize::new(grid_width, 1, 1), threadgroup_size);
            encoder.end_encoding();
            Ok(())
        }

        pub(super) fn commit(
            self,
            wait: MetalCommandWait,
        ) -> Result<MetalCommandStatus, RuntimeError> {
            self.command_buffer.commit();
            match wait {
                MetalCommandWait::None => {}
                MetalCommandWait::Scheduled => self.command_buffer.wait_until_scheduled(),
                MetalCommandWait::Completed => self.command_buffer.wait_until_completed(),
            }

            let status = map_command_status(self.command_buffer.status());
            if status == MetalCommandStatus::Error {
                return Err(RuntimeError::Backend(String::from(
                    "metal command buffer reported an error",
                )));
            }
            match wait {
                MetalCommandWait::Completed if status != MetalCommandStatus::Completed => {
                    Err(RuntimeError::Backend(format!(
                        "metal command buffer did not complete cleanly: {status:?}"
                    )))
                }
                MetalCommandWait::Scheduled
                    if !matches!(
                        status,
                        MetalCommandStatus::Scheduled | MetalCommandStatus::Completed
                    ) =>
                {
                    Err(RuntimeError::Backend(format!(
                        "metal command buffer did not schedule cleanly: {status:?}"
                    )))
                }
                _ => Ok(status),
            }
        }
    }

    pub(super) struct ConfiguredBackend {
        descriptor: DeviceDescriptor,
        device: MetalDevice,
        command_queue: CommandQueue,
        storage_mode: MetalStorageMode,
        pipelines: Option<EmbeddingsPipelines>,
    }

    impl ConfiguredBackend {
        pub(super) fn descriptor(&self) -> &DeviceDescriptor {
            &self.descriptor
        }

        pub(super) fn storage_mode(&self) -> MetalStorageMode {
            self.storage_mode
        }

        pub(super) fn allocate_buffer(
            &self,
            byte_len: usize,
        ) -> Result<PlatformBuffer, RuntimeError> {
            let raw = self.device.new_buffer(
                to_metal_size(byte_len)?,
                resource_options(self.storage_mode),
            );
            Ok(PlatformBuffer { raw })
        }

        pub(super) fn begin_submission(
            &self,
            label: String,
        ) -> Result<PlatformSubmission, RuntimeError> {
            let command_buffer = self.command_queue.new_command_buffer().to_owned();
            if !label.is_empty() {
                command_buffer.set_label(&label);
            }
            Ok(PlatformSubmission { command_buffer })
        }

        fn pipelines(&mut self) -> Result<&EmbeddingsPipelines, RuntimeError> {
            if self.pipelines.is_none() {
                self.pipelines = Some(compile_embeddings_pipelines(&self.device)?);
            }
            let Some(pipelines) = self.pipelines.as_ref() else {
                return Err(RuntimeError::Backend(String::from(
                    "metal embeddings pipelines were not initialized",
                )));
            };
            Ok(pipelines)
        }

        pub(super) fn encode_add(
            &mut self,
            submission: &mut PlatformSubmission,
            left: &MetalBuffer,
            right: &MetalBuffer,
            output: &MetalBuffer,
            element_count: usize,
        ) -> Result<(), RuntimeError> {
            let pipeline = &self.pipelines()?.add;
            submission.encode_add(
                pipeline,
                &left.platform,
                &right.platform,
                &output.platform,
                element_count,
            )
        }

        pub(super) fn encode_matmul(
            &mut self,
            submission: &mut PlatformSubmission,
            left: &MetalBuffer,
            right: &MetalBuffer,
            output: &MetalBuffer,
        ) -> Result<(), RuntimeError> {
            let left_dims = left.spec().shape().dims();
            let right_dims = right.spec().shape().dims();
            if left_dims.len() != 2 || right_dims.len() != 2 || left_dims[1] != right_dims[0] {
                return Err(RuntimeError::Backend(String::from(
                    "metal matmul requires rank-2 tensors with matching inner dimensions",
                )));
            }
            let pipeline = &self.pipelines()?.matmul;
            submission.encode_matmul(
                pipeline,
                &left.platform,
                &right.platform,
                &output.platform,
                left_dims[0],
                left_dims[1],
                right_dims[1],
            )
        }

        pub(super) fn synchronize_output(
            &self,
            submission: &mut PlatformSubmission,
            output: &MetalBuffer,
        ) -> Result<bool, RuntimeError> {
            submission.synchronize_buffer(&output.platform, output.storage_mode())
        }
    }

    pub(super) fn configure_preferred_backend() -> Result<ConfiguredBackend, RuntimeHealth> {
        let records = collect_device_records().map_err(|error| RuntimeHealth {
            status: HealthStatus::Degraded,
            message: format!("metal backend discovery failed during configuration: {error}"),
        })?;
        let Some(record) = records
            .into_iter()
            .find(|record| record.support_tier == DeviceSupportTier::Modern)
        else {
            return Err(discovery_report()
                .map(|report| report.health)
                .unwrap_or(RuntimeHealth {
                    status: HealthStatus::Offline,
                    message: String::from("metal runtime reported no devices"),
                }));
        };

        let command_queue = record.device.new_command_queue();
        command_queue.set_label(&format!("mox.metal.queue.{}", record.descriptor.device));
        let storage_mode = if record.descriptor.unified_memory == Some(true) {
            MetalStorageMode::Shared
        } else {
            MetalStorageMode::Managed
        };

        Ok(ConfiguredBackend {
            descriptor: record.descriptor,
            device: record.device,
            command_queue,
            storage_mode,
            pipelines: None,
        })
    }

    pub(super) fn discovery_report() -> Result<MetalDiscoveryReport, RuntimeError> {
        let records = collect_device_records()?;
        let mut devices = Vec::with_capacity(records.len());
        let mut modern_count = 0usize;
        let mut legacy_count = 0usize;

        for record in records {
            match record.support_tier {
                DeviceSupportTier::Modern => modern_count += 1,
                DeviceSupportTier::Legacy => legacy_count += 1,
            }
            devices.push(record.descriptor);
        }

        let health = if modern_count > 0 {
            let message = if legacy_count > 0 {
                format!(
                    "metal discovery ready on {modern_count} modern device(s); {legacy_count} legacy-only device(s) remain degraded"
                )
            } else {
                format!("metal discovery ready on {modern_count} modern device(s)")
            };
            RuntimeHealth {
                status: HealthStatus::Ready,
                message,
            }
        } else if legacy_count > 0 {
            RuntimeHealth {
                status: HealthStatus::Degraded,
                message: format!(
                    "metal discovered {legacy_count} legacy-only device(s); Mox currently targets Apple-family or Common3-class GPUs first"
                ),
            }
        } else {
            RuntimeHealth {
                status: HealthStatus::Offline,
                message: String::from("metal runtime reported no devices"),
            }
        };

        Ok(MetalDiscoveryReport { devices, health })
    }

    struct DeviceRecord {
        device: MetalDevice,
        descriptor: DeviceDescriptor,
        support_tier: DeviceSupportTier,
    }

    fn collect_device_records() -> Result<Vec<DeviceRecord>, RuntimeError> {
        let mut records = Vec::new();
        for (ordinal, device) in MetalDevice::all().into_iter().enumerate() {
            let family = collect_family_support(&device);
            let tier = classify_support(family);
            let descriptor = build_descriptor(ordinal, &device, tier, family)?;
            records.push(DeviceRecord {
                device,
                descriptor,
                support_tier: tier,
            });
        }
        Ok(records)
    }

    fn build_descriptor(
        ordinal: usize,
        device: &MetalDeviceRef,
        tier: DeviceSupportTier,
        family: FamilySupport,
    ) -> Result<DeviceDescriptor, RuntimeError> {
        let ordinal = u16::try_from(ordinal)
            .map_err(|_| RuntimeError::Backend(String::from("metal device ordinal overflow")))?;
        let mut feature_flags = Vec::new();
        feature_flags.push(match tier {
            DeviceSupportTier::Modern => String::from(MODERN_FAMILY_FLAG),
            DeviceSupportTier::Legacy => String::from(LEGACY_FAMILY_FLAG),
        });
        feature_flags.push(location_flag(device.location()).to_owned());
        feature_flags.push(if device.has_unified_memory() {
            String::from("unified_memory")
        } else {
            String::from("discrete_memory")
        });
        push_flag(&mut feature_flags, device.is_low_power(), "low_power");
        push_flag(&mut feature_flags, device.is_headless(), "headless");
        push_flag(&mut feature_flags, device.is_removable(), "removable");
        push_flag(&mut feature_flags, family.apple, "gpu_family_apple");
        push_flag(&mut feature_flags, family.common2, "gpu_family_common2");
        push_flag(&mut feature_flags, family.common3, "gpu_family_common3");
        push_flag(&mut feature_flags, family.mac1, "gpu_family_mac1");
        push_flag(&mut feature_flags, family.mac2, "gpu_family_mac2");
        push_flag(&mut feature_flags, family.metal3, "gpu_family_metal3");
        push_flag(&mut feature_flags, family.metal4, "gpu_family_metal4");
        push_flag(
            &mut feature_flags,
            matches!(tier, DeviceSupportTier::Modern),
            "submit_ready",
        );
        push_flag(
            &mut feature_flags,
            matches!(tier, DeviceSupportTier::Legacy),
            "submit_degraded",
        );

        let memory_capacity_bytes = match device.recommended_max_working_set_size() {
            0 => None,
            size => Some(size),
        };

        Ok(DeviceDescriptor {
            backend: String::from("metal"),
            device: Device::new(DeviceKind::Metal, ordinal, Some(format!("metal:{ordinal}"))),
            device_name: Some(device.name().to_owned()),
            supported_dtypes: vec![DType::F32],
            supported_quantization: vec![QuantizationSupport {
                mode: QuantizationMode::None,
                load_path: QuantizationLoadPath::DenseF32,
                execution: QuantizationExecution::Native,
            }],
            memory_capacity_bytes,
            unified_memory: Some(device.has_unified_memory()),
            feature_flags,
            amd_metadata: None,
        })
    }

    fn push_flag(feature_flags: &mut Vec<String>, enabled: bool, flag: &str) {
        if enabled {
            feature_flags.push(flag.to_owned());
        }
    }

    fn location_flag(location: MTLDeviceLocation) -> &'static str {
        match location {
            MTLDeviceLocation::BuiltIn => "location_built_in",
            MTLDeviceLocation::Slot => "location_slot",
            MTLDeviceLocation::External => "location_external",
            MTLDeviceLocation::Unspecified => "location_unspecified",
        }
    }

    fn collect_family_support(device: &MetalDeviceRef) -> FamilySupport {
        FamilySupport {
            common2: device.supports_family(MTLGPUFamily::Common2),
            common3: device.supports_family(MTLGPUFamily::Common3),
            mac1: device.supports_family(MTLGPUFamily::Mac1),
            mac2: device.supports_family(MTLGPUFamily::Mac2),
            metal3: device.supports_family(MTLGPUFamily::Metal3),
            metal4: device.supports_family(MTLGPUFamily::Metal4),
            apple: supports_any_apple_family(device),
        }
    }

    fn supports_any_apple_family(device: &MetalDeviceRef) -> bool {
        [
            MTLGPUFamily::Apple1,
            MTLGPUFamily::Apple2,
            MTLGPUFamily::Apple3,
            MTLGPUFamily::Apple4,
            MTLGPUFamily::Apple5,
            MTLGPUFamily::Apple6,
            MTLGPUFamily::Apple7,
            MTLGPUFamily::Apple8,
            MTLGPUFamily::Apple9,
        ]
        .into_iter()
        .any(|family| device.supports_family(family))
    }

    fn resource_options(storage_mode: MetalStorageMode) -> MTLResourceOptions {
        match storage_mode {
            MetalStorageMode::Shared => {
                MTLResourceOptions::CPUCacheModeDefaultCache | MTLResourceOptions::StorageModeShared
            }
            MetalStorageMode::Managed => {
                MTLResourceOptions::CPUCacheModeDefaultCache
                    | MTLResourceOptions::StorageModeManaged
            }
            MetalStorageMode::Private => MTLResourceOptions::StorageModePrivate,
        }
    }

    fn compile_embeddings_pipelines(
        device: &MetalDeviceRef,
    ) -> Result<EmbeddingsPipelines, RuntimeError> {
        let options = CompileOptions::new();
        options.set_fast_math_enabled(false);
        let library = device
            .new_library_with_source(EMBEDDINGS_METAL_SOURCE, &options)
            .map_err(|error| {
                RuntimeError::Backend(format!("metal shader compile failed: {error}"))
            })?;
        let add = library
            .get_function("mox_add", None)
            .map_err(|error| RuntimeError::Backend(format!("missing Metal add kernel: {error}")))?;
        let matmul = library.get_function("mox_matmul", None).map_err(|error| {
            RuntimeError::Backend(format!("missing Metal matmul kernel: {error}"))
        })?;

        Ok(EmbeddingsPipelines {
            add: device
                .new_compute_pipeline_state_with_function(&add)
                .map_err(|error| {
                    RuntimeError::Backend(format!("metal add pipeline build failed: {error}"))
                })?,
            matmul: device
                .new_compute_pipeline_state_with_function(&matmul)
                .map_err(|error| {
                    RuntimeError::Backend(format!("metal matmul pipeline build failed: {error}"))
                })?,
        })
    }

    fn compute_threadgroup_size(
        pipeline: &ComputePipelineState,
        grid_width: usize,
    ) -> Result<MTLSize, RuntimeError> {
        let width = pipeline.thread_execution_width();
        let max_threads = pipeline.max_total_threads_per_threadgroup();
        let grid_width = to_metal_size(grid_width)?;
        let width = width.min(max_threads).min(grid_width.max(1));
        Ok(MTLSize::new(width.max(1), 1, 1))
    }

    fn map_command_status(status: MTLCommandBufferStatus) -> MetalCommandStatus {
        match status {
            MTLCommandBufferStatus::NotEnqueued => MetalCommandStatus::NotEnqueued,
            MTLCommandBufferStatus::Enqueued => MetalCommandStatus::Enqueued,
            MTLCommandBufferStatus::Committed => MetalCommandStatus::Committed,
            MTLCommandBufferStatus::Scheduled => MetalCommandStatus::Scheduled,
            MTLCommandBufferStatus::Completed => MetalCommandStatus::Completed,
            MTLCommandBufferStatus::Error => MetalCommandStatus::Error,
        }
    }

    fn to_metal_size(size: usize) -> Result<u64, RuntimeError> {
        u64::try_from(size)
            .map_err(|_| RuntimeError::Backend(String::from("metal size conversion overflow")))
    }

    fn byte_range(byte_len: usize) -> Result<NSRange, RuntimeError> {
        Ok(NSRange::new(0, to_metal_size(byte_len)?))
    }

    const EMBEDDINGS_METAL_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

kernel void mox_add(
    const device float* left [[buffer(0)]],
    const device float* right [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& element_count [[buffer(3)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid >= element_count) {
        return;
    }
    output[gid] = left[gid] + right[gid];
}

kernel void mox_matmul(
    const device float* left [[buffer(0)]],
    const device float* right [[buffer(1)]],
    device float* output [[buffer(2)]],
    constant uint& m [[buffer(3)]],
    constant uint& k [[buffer(4)]],
    constant uint& n [[buffer(5)]],
    uint gid [[thread_position_in_grid]]
) {
    uint row = gid / n;
    uint col = gid % n;
    if (row >= m || col >= n) {
        return;
    }

    float sum = 0.0f;
    for (uint inner = 0; inner < k; inner++) {
        sum += left[(row * k) + inner] * right[(inner * n) + col];
    }
    output[(row * n) + col] = sum;
}
"#;
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use mox_runtime::{HealthStatus, RuntimeHealth};

    use super::{
        MetalBuffer, MetalCommandStatus, MetalCommandWait, MetalDiscoveryReport, MetalStorageMode,
        RuntimeError,
    };

    #[derive(Clone)]
    pub(super) struct PlatformBuffer;

    impl PlatformBuffer {
        pub(super) fn write_bytes(
            &self,
            _bytes: &[u8],
            _storage_mode: MetalStorageMode,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn read_bytes(&self, _byte_len: usize) -> Result<Vec<u8>, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }
    }

    pub(super) struct PlatformSubmission;

    impl PlatformSubmission {
        pub(super) fn fill_buffer(
            &mut self,
            _buffer: &PlatformBuffer,
            _byte_len: usize,
            _value: u8,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn copy_buffer(
            &mut self,
            _source: &PlatformBuffer,
            _destination: &PlatformBuffer,
            _byte_len: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn synchronize_buffer(
            &mut self,
            _buffer: &PlatformBuffer,
            _storage_mode: MetalStorageMode,
        ) -> Result<bool, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn commit(
            self,
            _wait: MetalCommandWait,
        ) -> Result<MetalCommandStatus, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }
    }

    pub(super) struct ConfiguredBackend {
        descriptor: mox_runtime::DeviceDescriptor,
    }

    impl ConfiguredBackend {
        pub(super) fn descriptor(&self) -> &mox_runtime::DeviceDescriptor {
            &self.descriptor
        }

        pub(super) const fn storage_mode(&self) -> MetalStorageMode {
            MetalStorageMode::Shared
        }

        pub(super) fn allocate_buffer(
            &self,
            _byte_len: usize,
        ) -> Result<PlatformBuffer, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn begin_submission(
            &self,
            _label: String,
        ) -> Result<PlatformSubmission, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_add(
            &mut self,
            _submission: &mut PlatformSubmission,
            _left: &MetalBuffer,
            _right: &MetalBuffer,
            _output: &MetalBuffer,
            _element_count: usize,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn encode_matmul(
            &mut self,
            _submission: &mut PlatformSubmission,
            _left: &MetalBuffer,
            _right: &MetalBuffer,
            _output: &MetalBuffer,
        ) -> Result<(), RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }

        pub(super) fn synchronize_output(
            &self,
            _submission: &mut PlatformSubmission,
            _output: &MetalBuffer,
        ) -> Result<bool, RuntimeError> {
            Err(RuntimeError::Backend(String::from(
                "metal backend is only available on macOS",
            )))
        }
    }

    pub(super) fn configure_preferred_backend() -> Result<ConfiguredBackend, RuntimeHealth> {
        Err(RuntimeHealth {
            status: HealthStatus::Offline,
            message: String::from("metal backend is only available on macOS"),
        })
    }

    pub(super) fn discovery_report() -> Result<MetalDiscoveryReport, mox_runtime::RuntimeError> {
        Ok(MetalDiscoveryReport {
            devices: Vec::new(),
            health: RuntimeHealth {
                status: HealthStatus::Offline,
                message: String::from("metal backend is only available on macOS"),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use mox_backend_cpu::CpuBackend;
    use mox_compiler::compile_graph;
    use mox_core::{DType, Device, DeviceKind, QuantizationMode, Shape, TensorSpec};
    use mox_ir::GraphBuilder;
    use mox_runtime::{Allocator, BackendParityPolicy, HealthStatus};

    use super::{
        DeviceSupportTier, EMBEDDINGS_SUPPORTED_OPS, FamilySupport, MetalBackend, classify_support,
        validate_supported_plan,
    };

    #[test]
    fn apple_family_devices_classify_as_modern() {
        let family = FamilySupport {
            apple: true,
            ..FamilySupport::default()
        };
        assert_eq!(classify_support(family), DeviceSupportTier::Modern);
    }

    #[test]
    fn common_three_devices_classify_as_modern() {
        let family = FamilySupport {
            common3: true,
            ..FamilySupport::default()
        };
        assert_eq!(classify_support(family), DeviceSupportTier::Modern);
    }

    #[test]
    fn legacy_devices_without_modern_families_degrade() {
        let family = FamilySupport {
            common2: true,
            mac1: true,
            ..FamilySupport::default()
        };
        assert_eq!(classify_support(family), DeviceSupportTier::Legacy);
    }

    #[test]
    fn metal_embeddings_surface_and_parity_policy_are_documented() {
        assert_eq!(
            EMBEDDINGS_SUPPORTED_OPS,
            &["input", "constant", "matmul", "add"]
        );
        let budget = BackendParityPolicy::default().embedding_budget(QuantizationMode::None);
        assert_eq!(budget.numeric.max_abs_delta, 1.0e-5);
        assert_eq!(budget.numeric.max_rel_delta, 1.0e-5);
    }

    #[test]
    fn metal_plan_validation_rejects_unsupported_ops() -> Result<(), Box<dyn std::error::Error>> {
        let device = Device::new(DeviceKind::Metal, 0, Some(String::from("metal:0")));
        let mut builder = GraphBuilder::new(device);
        let input = builder.input("features", Shape::new(vec![1, 2]), DType::F32);
        let weights = builder.constant_f32(Shape::new(vec![1, 2]), vec![1.0, 0.0])?;
        let unsupported = builder.mul(&input, &weights)?;
        let graph = builder.finish(vec![unsupported]);
        let plan = compile_graph(&graph)?;
        let error = validate_supported_plan(&plan).expect_err("mul should be rejected");
        assert_eq!(
            error,
            mox_runtime::RuntimeError::UnsupportedStep(String::from("mul"))
        );
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn metal_backend_reports_offline_on_unsupported_platform()
    -> Result<(), mox_runtime::RuntimeError> {
        let backend = MetalBackend::new();
        let report = backend.discovery_report()?;
        assert!(report.devices.is_empty());
        assert_eq!(report.health.status, HealthStatus::Offline);
        assert_eq!(
            report.health.message,
            String::from("metal backend is only available on macOS")
        );
        assert!(backend.selected_device().is_none());
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn metal_backend_fallback_selection_reports_explicit_cpu_fallback()
    -> Result<(), mox_runtime::RuntimeError> {
        let backend = MetalBackend::new();
        let cpu = CpuBackend::new();
        let selection = backend.fallback_selection(&cpu, EMBEDDINGS_SUPPORTED_OPS)?;
        assert_eq!(selection.requested_backend, "metal");
        assert_eq!(selection.effective_backend, "cpu");
        assert_eq!(
            selection.fallback_reason.as_deref(),
            Some("metal backend unavailable: metal backend is only available on macOS")
        );
        assert_eq!(
            selection.supported_ops,
            EMBEDDINGS_SUPPORTED_OPS
                .iter()
                .map(|label| String::from(*label))
                .collect::<Vec<_>>()
        );
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn metal_backend_rejects_allocation_and_submission_when_unavailable() {
        let mut backend = MetalBackend::new();
        let spec = TensorSpec::new(
            Shape::new(vec![1, 4]),
            DType::F32,
            Device::new(DeviceKind::Metal, 0, Some(String::from("metal:0"))),
        );
        let allocation = backend.allocate(&spec);
        assert!(allocation.is_err());

        let submission = backend.begin_submission("noop");
        assert!(submission.is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_health_matches_discovery() -> Result<(), mox_runtime::RuntimeError> {
        use mox_runtime::DeviceDiscovery;

        use super::{LEGACY_FAMILY_FLAG, MODERN_FAMILY_FLAG};

        let backend = MetalBackend::new();
        let report = backend.discovery_report()?;
        let health = backend.health();
        assert_eq!(report.health, health);
        match health.status {
            HealthStatus::Ready => assert!(report.devices.iter().any(|descriptor| {
                descriptor
                    .feature_flags
                    .contains(&String::from(MODERN_FAMILY_FLAG))
            })),
            HealthStatus::Degraded => assert!(report.devices.iter().all(|descriptor| {
                descriptor
                    .feature_flags
                    .contains(&String::from(LEGACY_FAMILY_FLAG))
            })),
            HealthStatus::Offline => assert!(report.devices.is_empty()),
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_selection_reports_ready_metal_or_explicit_cpu_fallback()
    -> Result<(), mox_runtime::RuntimeError> {
        let backend = MetalBackend::new();
        let cpu = CpuBackend::new();
        match backend.backend_selection(EMBEDDINGS_SUPPORTED_OPS) {
            Ok(selection) => {
                assert_eq!(selection.requested_backend, "metal");
                assert_eq!(selection.effective_backend, "metal");
                assert!(selection.selected_device.is_some());
                assert!(selection.fallback_reason.is_none());
            }
            Err(error) => {
                assert!(error.to_string().starts_with("metal backend unavailable: "));
                let fallback = backend.fallback_selection(&cpu, EMBEDDINGS_SUPPORTED_OPS)?;
                assert_eq!(fallback.requested_backend, "metal");
                assert_eq!(fallback.effective_backend, "cpu");
                assert!(fallback.selected_device.is_some());
                assert!(fallback.fallback_reason.is_some());
            }
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_allocates_and_submits_copy_on_supported_hardware()
    -> Result<(), mox_runtime::RuntimeError> {
        use super::{MetalCommandStatus, MetalCommandWait};

        let mut backend = MetalBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let spec = TensorSpec::new(Shape::new(vec![1, 4]), DType::F32, selected.device.clone());
        let mut source = backend.allocate(&spec)?;
        source.write_f32(&[1.0, 2.0, 3.0, 4.0])?;
        let destination = backend.allocate(&spec)?;

        let mut submission = backend.begin_submission("buffer_copy")?;
        submission.copy_buffer(&source, &destination)?;
        submission.synchronize_buffer(&destination)?;
        let report = submission.commit(MetalCommandWait::Completed)?;
        assert_eq!(report.status, MetalCommandStatus::Completed);
        assert_eq!(report.encoded_operations, 1);
        assert_eq!(destination.read_f32()?, vec![1.0, 2.0, 3.0, 4.0]);
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn metal_backend_executes_embedding_surface_on_supported_hardware()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut backend = MetalBackend::new();
        let Some(selected) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
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
            .ok_or("missing metal embedding output")?;
        assert_eq!(output.read_f32()?, vec![1.5, 2.5]);
        assert_eq!(result.metrics.steps_executed, 5);
        Ok(())
    }
}
