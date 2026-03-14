//! AMD userspace backend discovery, opt-in gating, staging allocation, and
//! submission substrate for Psionic.

#![allow(dead_code)]
#![cfg_attr(
    test,
    allow(unused_imports, clippy::expect_used, clippy::panic_in_result_fn)
)]

use std::{
    env, fmt, fs,
    path::Path,
    sync::{Arc, Mutex},
};

use psionic_core::{DType, Device, DeviceKind, Shape, TensorSpec};
use psionic_runtime::{
    Allocator, AllocatorPoolPolicy, AllocatorPoolReport, AllocatorPoolState, AmdBackendReport,
    AmdDeviceMetadata, AmdDriverBinding, AmdOptInStatus, AmdRecoveryAction, AmdRecoveryProfile,
    AmdRiskLevel, AmdRiskProfile, AmdRuntimeMode, AmdTopologyInfo, BackendDegradedPolicy,
    BackendName, BackendRuntimeResources, BackendSelection, BufferHandle, DeviceDescriptor,
    DeviceDiscovery, DeviceMemoryBudget, ExecutionPlanCachePolicy, ExecutionPlanCacheReport,
    ExecutionPlanCacheState, HealthStatus, KernelCachePolicy, KernelCacheReport, KernelCacheState,
    RuntimeError, RuntimeHealth, ServedProductBackendPolicy,
};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str =
    "AMD userspace backend discovery, opt-in, staging allocation, and submission";

const AMD_VENDOR_ID: u32 = 0x1002;
const DRM_ROOT: &str = "/sys/class/drm";
const AMDGPU_MODULE: &str = "/sys/module/amdgpu";
const OPT_IN_ENV: &str = "RUSTYGRAD_AMD_USERSPACE_ENABLE";
const AMD_POOL_MAX_CACHED_BUFFERS: usize = 128;
const AMD_POOL_MAX_CACHED_BYTES: u64 = 64 * 1024 * 1024;

/// Exact execution-substrate ops that are truthful before graph lowering lands.
pub const EXECUTION_SUBSTRATE_SUPPORTED_OPS: &[&str] =
    &["input", "constant", "fill_buffer", "copy_buffer"];

/// Userspace-visible memory class surfaced by Psionic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmdUserspaceMemorySpace {
    /// Host-visible userspace-claimed staging memory.
    HostVisibleUserspace,
}

/// How long Psionic should wait after recording an AMD userspace submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmdUserspaceCommandWait {
    /// Wait until the recorded submission completes.
    Completed,
}

/// Stable submission lifecycle state exposed by Psionic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmdUserspaceCommandStatus {
    /// The submission was recorded by Psionic.
    Submitted,
    /// The submission completed successfully.
    Completed,
    /// The submission failed.
    Error,
}

/// Submission metadata returned after an AMD userspace submission is committed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AmdUserspaceSubmissionReport {
    /// Final submission status observed by Psionic.
    pub status: AmdUserspaceCommandStatus,
    /// Number of explicit operations recorded in the submission.
    pub encoded_operations: usize,
}

/// Backend-owned AMD userspace buffer.
#[derive(Clone)]
pub struct AmdUserspaceBuffer {
    spec: TensorSpec,
    byte_len: usize,
    memory_space: AmdUserspaceMemorySpace,
    host_visible: bool,
    storage: Arc<Mutex<Vec<u8>>>,
}

impl fmt::Debug for AmdUserspaceBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AmdUserspaceBuffer")
            .field("spec", &self.spec)
            .field("byte_len", &self.byte_len)
            .field("memory_space", &self.memory_space)
            .field("host_visible", &self.host_visible)
            .field("storage", &"<amd userspace staging buffer>")
            .finish()
    }
}

impl AmdUserspaceBuffer {
    /// Returns the backing allocation size in bytes.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// Returns the AMD userspace memory space backing the buffer.
    #[must_use]
    pub const fn memory_space(&self) -> AmdUserspaceMemorySpace {
        self.memory_space
    }

    /// Returns whether the CPU can directly map the backing storage.
    #[must_use]
    pub const fn host_visible(&self) -> bool {
        self.host_visible
    }

    /// Writes raw bytes into the host-visible backing storage.
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        if bytes.len() != self.byte_len {
            return Err(RuntimeError::Backend(format!(
                "amd_userspace buffer write length mismatch: expected {}, actual {}",
                self.byte_len,
                bytes.len()
            )));
        }
        self.storage
            .lock()
            .map_err(|error| {
                RuntimeError::Backend(format!("amd_userspace buffer lock poisoned: {error}"))
            })?
            .copy_from_slice(bytes);
        Ok(())
    }

    /// Reads raw bytes from the host-visible backing storage.
    pub fn read_bytes(&self) -> Result<Vec<u8>, RuntimeError> {
        Ok(self
            .storage
            .lock()
            .map_err(|error| {
                RuntimeError::Backend(format!("amd_userspace buffer lock poisoned: {error}"))
            })?
            .clone())
    }

    /// Writes contiguous `f32` values into an `f32` buffer.
    pub fn write_f32(&mut self, values: &[f32]) -> Result<(), RuntimeError> {
        if self.spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "amd_userspace write_f32 requires F32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        if values.len() != self.spec.storage_size() {
            return Err(RuntimeError::Backend(format!(
                "amd_userspace buffer write length mismatch: expected {} values, actual {}",
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
                "amd_userspace read_f32 requires F32 buffer, actual {:?}",
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

impl BufferHandle for AmdUserspaceBuffer {
    fn spec(&self) -> &TensorSpec {
        &self.spec
    }
}

/// Explicit submission over the AMD userspace staging substrate.
pub struct AmdUserspaceSubmission {
    encoded_operations: usize,
    status: AmdUserspaceCommandStatus,
}

impl AmdUserspaceSubmission {
    /// Fills a buffer with one repeated byte value.
    pub fn fill_buffer(
        &mut self,
        buffer: &AmdUserspaceBuffer,
        value: u8,
    ) -> Result<(), RuntimeError> {
        let mut storage = buffer.storage.lock().map_err(|error| {
            RuntimeError::Backend(format!("amd_userspace buffer lock poisoned: {error}"))
        })?;
        storage.fill(value);
        self.encoded_operations += 1;
        Ok(())
    }

    /// Copies one AMD userspace buffer into another with explicit size checking.
    pub fn copy_buffer(
        &mut self,
        source: &AmdUserspaceBuffer,
        destination: &AmdUserspaceBuffer,
    ) -> Result<(), RuntimeError> {
        if source.byte_len != destination.byte_len {
            return Err(RuntimeError::Backend(format!(
                "amd_userspace buffer copy length mismatch: source {}, destination {}",
                source.byte_len, destination.byte_len
            )));
        }
        let source_bytes = source.read_bytes()?;
        destination
            .storage
            .lock()
            .map_err(|error| {
                RuntimeError::Backend(format!("amd_userspace buffer lock poisoned: {error}"))
            })?
            .copy_from_slice(&source_bytes);
        self.encoded_operations += 1;
        Ok(())
    }

    /// Completes the explicit submission.
    pub fn commit(
        mut self,
        wait: AmdUserspaceCommandWait,
    ) -> Result<AmdUserspaceSubmissionReport, RuntimeError> {
        match wait {
            AmdUserspaceCommandWait::Completed => {
                self.status = AmdUserspaceCommandStatus::Completed;
            }
        }
        Ok(AmdUserspaceSubmissionReport {
            status: self.status,
            encoded_operations: self.encoded_operations,
        })
    }
}

enum AmdUserspaceBackendState {
    Available(Box<AvailableAmdUserspaceBackend>),
    Unavailable(RuntimeHealth),
}

struct AvailableAmdUserspaceBackend {
    descriptor: DeviceDescriptor,
    allocator_pool: AllocatorPoolReport,
    kernel_cache: KernelCacheReport,
}

/// AMD userspace backend discovery plus staging allocation/submission substrate.
pub struct AmdUserspaceBackend {
    state: AmdUserspaceBackendState,
}

impl Default for AmdUserspaceBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AmdUserspaceBackend {
    /// Creates an AMD userspace backend and selects the first execution-ready device
    /// when explicit opt-in and kernel-driver handoff are both present.
    #[must_use]
    pub fn new() -> Self {
        match platform::discovery_report() {
            Ok(report) => {
                let Some(descriptor) = report.devices.first().cloned() else {
                    return Self {
                        state: AmdUserspaceBackendState::Unavailable(report.health),
                    };
                };
                if report.health.status != HealthStatus::Ready {
                    return Self {
                        state: AmdUserspaceBackendState::Unavailable(report.health),
                    };
                }
                Self {
                    state: AmdUserspaceBackendState::Available(Box::new(
                        AvailableAmdUserspaceBackend {
                            descriptor,
                            allocator_pool: AllocatorPoolReport {
                                policy: amd_allocator_pool_policy(),
                                state: AllocatorPoolState::default(),
                            },
                            kernel_cache: KernelCacheReport {
                                policy: KernelCachePolicy::disabled(),
                                state: KernelCacheState::default(),
                            },
                        },
                    )),
                }
            }
            Err(error) => Self {
                state: AmdUserspaceBackendState::Unavailable(RuntimeHealth {
                    status: HealthStatus::Offline,
                    message: error.to_string(),
                }),
            },
        }
    }

    /// Returns the required opt-in environment variable name.
    #[must_use]
    pub const fn opt_in_env() -> &'static str {
        OPT_IN_ENV
    }

    /// Returns the backend-local AMD discovery report.
    pub fn discovery_report(&self) -> Result<AmdBackendReport, RuntimeError> {
        platform::discovery_report()
    }

    /// Returns the device selected for allocation/submission, when available.
    #[must_use]
    pub fn selected_device(&self) -> Option<&DeviceDescriptor> {
        match &self.state {
            AmdUserspaceBackendState::Available(backend) => Some(&backend.descriptor),
            AmdUserspaceBackendState::Unavailable(_) => None,
        }
    }

    /// Returns explicit runtime-resource posture for the selected AMD userspace device.
    #[must_use]
    pub fn runtime_resources(&self) -> Option<BackendRuntimeResources> {
        match &self.state {
            AmdUserspaceBackendState::Available(backend) => Some(BackendRuntimeResources {
                execution_plan_cache: ExecutionPlanCacheReport {
                    policy: ExecutionPlanCachePolicy::disabled(),
                    state: ExecutionPlanCacheState::default(),
                },
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
            AmdUserspaceBackendState::Unavailable(_) => None,
        }
    }

    /// Creates a dense `f32` input buffer on the selected AMD userspace device.
    pub fn input_buffer(
        &mut self,
        shape: Shape,
        values: impl Into<Vec<f32>>,
    ) -> Result<AmdUserspaceBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "amd_userspace backend unavailable: no selected execution device",
            )));
        };
        let mut buffer = self.allocate(&TensorSpec::new(shape, DType::F32, device))?;
        buffer.write_f32(values.into().as_slice())?;
        Ok(buffer)
    }

    /// Returns explicit backend-selection truth for the current AMD userspace execution substrate.
    pub fn backend_selection(
        &self,
        supported_ops: &[&str],
    ) -> Result<BackendSelection, RuntimeError> {
        let Some(backend) = self.selected_backend() else {
            return Err(RuntimeError::Backend(format!(
                "amd_userspace backend unavailable: {}",
                self.health().message
            )));
        };
        Ok(BackendSelection::direct_with_policy(
            self.backend_name(),
            Some(backend.descriptor.clone()),
            supported_ops
                .iter()
                .map(|label| String::from(*label))
                .collect(),
            ServedProductBackendPolicy::fallback_to_compatible_backend(
                BackendDegradedPolicy::AllowSameBackend,
            ),
        )
        .with_runtime_resources(self.runtime_resources()))
    }

    /// Returns an explicit fallback selection when AMD userspace cannot execute the requested path.
    pub fn fallback_selection<B>(
        &self,
        fallback_backend: &B,
        supported_ops: &[&str],
    ) -> Result<BackendSelection, RuntimeError>
    where
        B: DeviceDiscovery + ?Sized,
    {
        match &self.state {
            AmdUserspaceBackendState::Available(_) => self.backend_selection(supported_ops),
            AmdUserspaceBackendState::Unavailable(health) => {
                Ok(BackendSelection::fallback_with_policy(
                    self.backend_name(),
                    fallback_backend.backend_name(),
                    fallback_backend.discover_devices()?.into_iter().next(),
                    supported_ops
                        .iter()
                        .map(|label| String::from(*label))
                        .collect(),
                    ServedProductBackendPolicy::fallback_to_compatible_backend(
                        BackendDegradedPolicy::AllowSameBackend,
                    ),
                    format!("amd_userspace backend unavailable: {}", health.message),
                )
                .with_runtime_resources(fallback_backend.runtime_resources())
                .with_backend_extensions(fallback_backend.extension_support()))
            }
        }
    }

    /// Allocates an AMD userspace staging buffer for the provided tensor specification.
    pub fn allocate_buffer(
        &mut self,
        spec: &TensorSpec,
    ) -> Result<AmdUserspaceBuffer, RuntimeError> {
        let Some(backend) = self.selected_backend() else {
            return Err(RuntimeError::Backend(self.health().message));
        };
        if spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "amd_userspace staging surface only supports F32 buffers, actual {:?}",
                spec.dtype()
            )));
        }
        if spec.device().kind() != DeviceKind::AmdUserspace {
            return Err(RuntimeError::Backend(format!(
                "amd_userspace allocator requires an AMD userspace tensor spec, actual device kind {}",
                spec.device().kind()
            )));
        }
        if spec.device().ordinal() != backend.descriptor.device.ordinal() {
            return Err(RuntimeError::Backend(format!(
                "amd_userspace allocator requires device ordinal {}, actual {}",
                backend.descriptor.device.ordinal(),
                spec.device().ordinal()
            )));
        }
        backend.allocate(spec)
    }

    /// Begins an explicit AMD userspace submission.
    pub fn begin_submission(&self) -> Result<AmdUserspaceSubmission, RuntimeError> {
        if self.selected_backend().is_none() {
            return Err(RuntimeError::Backend(self.health().message));
        }
        Ok(AmdUserspaceSubmission {
            encoded_operations: 0,
            status: AmdUserspaceCommandStatus::Submitted,
        })
    }

    fn selected_backend(&self) -> Option<&AvailableAmdUserspaceBackend> {
        match &self.state {
            AmdUserspaceBackendState::Available(backend) => Some(backend),
            AmdUserspaceBackendState::Unavailable(_) => None,
        }
    }
}

impl DeviceDiscovery for AmdUserspaceBackend {
    fn backend_name(&self) -> BackendName {
        "amd_userspace"
    }

    fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError> {
        self.discovery_report().map(|report| report.devices)
    }

    fn health(&self) -> RuntimeHealth {
        match &self.state {
            AmdUserspaceBackendState::Available(_) => match self.discovery_report() {
                Ok(report) => report.health,
                Err(error) => RuntimeHealth {
                    status: HealthStatus::Degraded,
                    message: format!("amd_userspace discovery failed: {error}"),
                },
            },
            AmdUserspaceBackendState::Unavailable(health) => health.clone(),
        }
    }

    fn runtime_resources(&self) -> Option<BackendRuntimeResources> {
        AmdUserspaceBackend::runtime_resources(self)
    }
}

impl Allocator for AmdUserspaceBackend {
    type Buffer = AmdUserspaceBuffer;

    fn allocate(&mut self, spec: &TensorSpec) -> Result<Self::Buffer, RuntimeError> {
        self.allocate_buffer(spec)
    }
}

impl AvailableAmdUserspaceBackend {
    fn allocate(&self, spec: &TensorSpec) -> Result<AmdUserspaceBuffer, RuntimeError> {
        let byte_len = spec
            .storage_size()
            .checked_mul(size_of_dtype(spec.dtype()))
            .ok_or_else(|| {
                RuntimeError::Backend(format!(
                    "amd_userspace buffer size overflow for tensor storage size {}",
                    spec.storage_size()
                ))
            })?;
        Ok(AmdUserspaceBuffer {
            spec: spec.clone(),
            byte_len,
            memory_space: AmdUserspaceMemorySpace::HostVisibleUserspace,
            host_visible: true,
            storage: Arc::new(Mutex::new(vec![0; byte_len])),
        })
    }
}

fn amd_allocator_pool_policy() -> AllocatorPoolPolicy {
    AllocatorPoolPolicy::exact_tensor_spec(AMD_POOL_MAX_CACHED_BUFFERS, AMD_POOL_MAX_CACHED_BYTES)
}

fn size_of_dtype(dtype: DType) -> usize {
    match dtype {
        DType::F32 => std::mem::size_of::<f32>(),
        _ => 0,
    }
}

fn parse_opt_in_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "enable" | "enabled"
    )
}

fn opt_in_enabled() -> bool {
    env::var(OPT_IN_ENV)
        .ok()
        .as_deref()
        .map(parse_opt_in_value)
        .unwrap_or(false)
}

fn userspace_health(
    device_count: usize,
    opt_in: AmdOptInStatus,
    amdgpu_loaded: bool,
) -> RuntimeHealth {
    match (opt_in, device_count, amdgpu_loaded) {
        (AmdOptInStatus::Disabled, _, _) => RuntimeHealth {
            status: HealthStatus::Offline,
            message: format!(
                "amd_userspace disabled: set {OPT_IN_ENV}=1 only on dedicated hosts prepared for userspace mode"
            ),
        },
        (AmdOptInStatus::Enabled, 0, _) => RuntimeHealth {
            status: HealthStatus::Offline,
            message: String::from("amd_userspace opted in but no AMD DRM devices were detected"),
        },
        (AmdOptInStatus::Enabled, count, true) => RuntimeHealth {
            status: HealthStatus::Degraded,
            message: format!(
                "amd_userspace opted in on {count} AMD device(s) but the amdgpu kernel module is still loaded"
            ),
        },
        (AmdOptInStatus::Enabled, count, false) => RuntimeHealth {
            status: HealthStatus::Ready,
            message: format!(
                "amd_userspace opted in on {count} AMD device(s) with kernel-driver handoff detected"
            ),
        },
        (AmdOptInStatus::NotRequired, _, _) => RuntimeHealth {
            status: HealthStatus::Offline,
            message: String::from("amd_userspace should always report explicit opt-in state"),
        },
    }
}

fn risk_profile() -> AmdRiskProfile {
    AmdRiskProfile {
        level: AmdRiskLevel::Elevated,
        requires_explicit_opt_in: true,
        may_unbind_kernel_driver: true,
        warnings: vec![
            String::from("userspace mode should only be enabled on dedicated hosts"),
            String::from("userspace mode may require unloading or rebinding amdgpu"),
        ],
    }
}

fn recovery_profile(driver_binding: AmdDriverBinding) -> AmdRecoveryProfile {
    AmdRecoveryProfile {
        driver_binding,
        expected_actions: vec![
            AmdRecoveryAction::ProcessRestart,
            AmdRecoveryAction::RebindKernelDriver,
            AmdRecoveryAction::RebootHost,
        ],
    }
}

fn is_card_entry(name: &str) -> bool {
    name.strip_prefix("card")
        .map(|suffix| {
            !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit())
        })
        .unwrap_or(false)
}

fn read_trimmed(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_hex_u32(path: &Path) -> Option<u32> {
    let value = read_trimmed(path)?;
    let value = value.trim_start_matches("0x");
    u32::from_str_radix(value, 16).ok()
}

fn read_u64(path: &Path) -> Option<u64> {
    read_trimmed(path)?.parse::<u64>().ok()
}

fn pci_bdf(device_dir: &Path) -> Option<String> {
    fs::canonicalize(device_dir).ok().and_then(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    })
}

fn amdgpu_loaded() -> bool {
    Path::new(AMDGPU_MODULE).exists()
}

fn driver_binding(amdgpu_loaded: bool) -> AmdDriverBinding {
    if amdgpu_loaded {
        AmdDriverBinding::KernelAmdgpu
    } else {
        AmdDriverBinding::UserspaceClaimed
    }
}

fn probe_device(
    path: &Path,
    ordinal: u16,
    opt_in: AmdOptInStatus,
    amdgpu_loaded: bool,
) -> Option<DeviceDescriptor> {
    let device_dir = path.join("device");
    if read_hex_u32(&device_dir.join("vendor"))? != AMD_VENDOR_ID {
        return None;
    }

    let pci_bdf = pci_bdf(&device_dir);
    let architecture = read_trimmed(&device_dir.join("gfx_target_version"));
    let vram_bytes = read_u64(&device_dir.join("mem_info_vram_total"));
    let visible_vram_bytes = read_u64(&device_dir.join("mem_info_vis_vram_total"));
    let device_name = read_trimmed(&device_dir.join("product_name")).or_else(|| {
        read_trimmed(&device_dir.join("device")).map(|device_id| format!("AMD GPU {device_id}"))
    });
    let binding = driver_binding(amdgpu_loaded);

    let mut feature_flags = Vec::new();
    match opt_in {
        AmdOptInStatus::Enabled => {
            feature_flags.push(String::from("userspace_opt_in_enabled"));
            if !amdgpu_loaded {
                feature_flags.push(String::from("execution_substrate_candidate"));
            }
        }
        AmdOptInStatus::Disabled => {
            feature_flags.push(String::from("userspace_opt_in_disabled"));
        }
        AmdOptInStatus::NotRequired => {}
    }
    if amdgpu_loaded {
        feature_flags.push(String::from("amdgpu_module_loaded"));
    } else {
        feature_flags.push(String::from("amdgpu_module_unloaded"));
    }

    Some(DeviceDescriptor {
        backend: String::from("amd_userspace"),
        device: Device::new(
            DeviceKind::AmdUserspace,
            ordinal,
            Some(format!("amd_userspace:{ordinal}")),
        ),
        device_name,
        supported_dtypes: vec![DType::F32],
        supported_quantization: Vec::new(),
        memory_capacity_bytes: vram_bytes.or(visible_vram_bytes),
        unified_memory: Some(false),
        feature_flags,
        amd_metadata: Some(AmdDeviceMetadata {
            mode: AmdRuntimeMode::Userspace,
            topology: AmdTopologyInfo {
                architecture,
                pci_bdf,
                xcc_count: None,
                shader_engine_count: None,
                compute_unit_count: None,
                vram_bytes,
                visible_vram_bytes,
            },
            risk: risk_profile(),
            recovery: recovery_profile(binding),
        }),
        nvidia_metadata: None,
    })
}

fn discover_devices_linux(
    opt_in: AmdOptInStatus,
    amdgpu_loaded: bool,
) -> Result<Vec<DeviceDescriptor>, RuntimeError> {
    let root = Path::new(DRM_ROOT);
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut devices = Vec::new();
    let entries = fs::read_dir(root)
        .map_err(|error| RuntimeError::Backend(format!("failed to read {DRM_ROOT}: {error}")))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            RuntimeError::Backend(format!(
                "failed to inspect AMD DRM entry in {DRM_ROOT}: {error}"
            ))
        })?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !is_card_entry(name.as_ref()) {
            continue;
        }
        if let Some(device) =
            probe_device(&entry.path(), devices.len() as u16, opt_in, amdgpu_loaded)
        {
            devices.push(device);
        }
    }

    Ok(devices)
}

mod platform {
    use super::*;

    #[cfg(target_os = "linux")]
    pub(super) fn discovery_report() -> Result<AmdBackendReport, RuntimeError> {
        let opt_in = if opt_in_enabled() {
            AmdOptInStatus::Enabled
        } else {
            AmdOptInStatus::Disabled
        };
        let amdgpu_loaded = amdgpu_loaded();
        let devices = discover_devices_linux(opt_in, amdgpu_loaded)?;
        let health = userspace_health(devices.len(), opt_in, amdgpu_loaded);
        Ok(AmdBackendReport {
            mode: AmdRuntimeMode::Userspace,
            opt_in,
            devices,
            health,
        })
    }

    #[cfg(not(target_os = "linux"))]
    pub(super) fn discovery_report() -> Result<AmdBackendReport, RuntimeError> {
        Ok(AmdBackendReport {
            mode: AmdRuntimeMode::Userspace,
            opt_in: AmdOptInStatus::Disabled,
            devices: Vec::new(),
            health: RuntimeHealth {
                status: HealthStatus::Offline,
                message: format!(
                    "amd_userspace requires Linux and explicit opt-in via {OPT_IN_ENV}=1"
                ),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use psionic_backend_cpu::CpuBackend;
    use psionic_core::{DType, Shape, TensorSpec};
    use psionic_runtime::{
        Allocator, BackendSelectionState, DeviceDiscovery, ServedProductBackendPolicy,
        ValidationCoverage, validation_reference_for_backend_probe,
    };

    use super::{
        AMD_POOL_MAX_CACHED_BUFFERS, AmdOptInStatus, AmdRuntimeMode, AmdUserspaceBackend,
        AmdUserspaceCommandStatus, AmdUserspaceCommandWait, AmdUserspaceMemorySpace,
        EXECUTION_SUBSTRATE_SUPPORTED_OPS, HealthStatus, parse_opt_in_value, userspace_health,
    };

    #[test]
    fn opt_in_parser_accepts_common_true_values() {
        for value in ["1", "true", "TRUE", "yes", "on", "enabled"] {
            assert!(
                parse_opt_in_value(value),
                "{value} should enable userspace mode"
            );
        }
        assert!(!parse_opt_in_value("0"));
        assert!(!parse_opt_in_value("no"));
    }

    #[test]
    fn userspace_health_is_offline_when_disabled() {
        let health = userspace_health(1, AmdOptInStatus::Disabled, true);
        assert_eq!(health.status, HealthStatus::Offline);
    }

    #[test]
    fn userspace_health_is_degraded_when_opted_in_but_amdgpu_is_loaded() {
        let health = userspace_health(1, AmdOptInStatus::Enabled, true);
        assert_eq!(health.status, HealthStatus::Degraded);
        assert!(health.message.contains("amdgpu"));
    }

    #[test]
    fn userspace_health_is_ready_when_opted_in_and_driver_handoff_detected() {
        let health = userspace_health(2, AmdOptInStatus::Enabled, false);
        assert_eq!(health.status, HealthStatus::Ready);
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn amd_userspace_reports_offline_outside_linux() -> Result<(), psionic_runtime::RuntimeError> {
        let backend = AmdUserspaceBackend::new();
        let report = backend.discovery_report()?;
        assert_eq!(report.mode, AmdRuntimeMode::Userspace);
        assert!(report.devices.is_empty());
        assert_eq!(report.health.status, HealthStatus::Offline);
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn amd_userspace_report_is_self_consistent_on_linux()
    -> Result<(), psionic_runtime::RuntimeError> {
        let backend = AmdUserspaceBackend::new();
        let report = backend.discovery_report()?;
        let validation = validation_reference_for_backend_probe(backend.backend_name());
        assert_eq!(report.mode, AmdRuntimeMode::Userspace);
        assert_eq!(validation.claim_id, "amd_userspace.refusal");
        assert_eq!(validation.coverage, ValidationCoverage::ExplicitRefusal);
        match report.opt_in {
            AmdOptInStatus::Disabled => assert_eq!(report.health.status, HealthStatus::Offline),
            AmdOptInStatus::Enabled => match report.health.status {
                HealthStatus::Ready | HealthStatus::Degraded | HealthStatus::Offline => {}
            },
            AmdOptInStatus::NotRequired => panic!("userspace backend must report explicit opt-in"),
        }
        Ok(())
    }

    #[test]
    fn amd_userspace_runtime_resources_are_explicit_when_execution_substrate_is_available() {
        let backend = AmdUserspaceBackend::new();
        if let Some(descriptor) = backend.selected_device() {
            let resources = backend
                .runtime_resources()
                .expect("available amd_userspace backend should surface runtime resources");
            assert_eq!(
                resources.allocator_pool.policy.max_cached_buffers,
                AMD_POOL_MAX_CACHED_BUFFERS
            );
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
    fn amd_userspace_backend_selection_reports_direct_execution_or_explicit_cpu_fallback()
    -> Result<(), psionic_runtime::RuntimeError> {
        let backend = AmdUserspaceBackend::new();
        if backend.selected_device().is_some() {
            let selection = backend.backend_selection(EXECUTION_SUBSTRATE_SUPPORTED_OPS)?;
            assert_eq!(selection.requested_backend, "amd_userspace");
            assert_eq!(selection.effective_backend, "amd_userspace");
            assert_eq!(selection.selection_state, BackendSelectionState::Direct);
            assert!(selection.runtime_resources.is_some());
            assert_eq!(
                selection.policy,
                ServedProductBackendPolicy::fallback_to_compatible_backend(
                    psionic_runtime::BackendDegradedPolicy::AllowSameBackend
                )
            );
            return Ok(());
        }

        let cpu = CpuBackend::new();
        let selection = backend.fallback_selection(&cpu, EXECUTION_SUBSTRATE_SUPPORTED_OPS)?;
        assert_eq!(selection.requested_backend, "amd_userspace");
        assert_eq!(selection.effective_backend, "cpu");
        assert_eq!(
            selection.selection_state,
            BackendSelectionState::CrossBackendFallback
        );
        assert!(selection.runtime_resources.is_some());
        assert!(selection.fallback_reason.is_some());
        Ok(())
    }

    #[test]
    fn amd_userspace_allocates_and_submits_copy_when_available()
    -> Result<(), psionic_runtime::RuntimeError> {
        let mut backend = AmdUserspaceBackend::new();
        let Some(device) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let spec = TensorSpec::new(Shape::new(vec![4]), DType::F32, device.device.clone());
        let mut left = backend.allocate(&spec)?;
        let right = backend.allocate(&spec)?;
        assert_eq!(
            left.memory_space(),
            AmdUserspaceMemorySpace::HostVisibleUserspace
        );
        assert!(left.host_visible());
        left.write_f32(&[1.0, 2.0, 3.0, 4.0])?;

        let mut submission = backend.begin_submission()?;
        submission.copy_buffer(&left, &right)?;
        let report = submission.commit(AmdUserspaceCommandWait::Completed)?;
        assert_eq!(report.status, AmdUserspaceCommandStatus::Completed);
        assert_eq!(report.encoded_operations, 1);
        assert_eq!(right.read_f32()?, vec![1.0, 2.0, 3.0, 4.0]);
        Ok(())
    }

    #[test]
    fn amd_userspace_input_buffer_populates_f32_values_when_available()
    -> Result<(), psionic_runtime::RuntimeError> {
        let mut backend = AmdUserspaceBackend::new();
        if backend.selected_device().is_none() {
            return Ok(());
        }

        let buffer = backend.input_buffer(Shape::new(vec![2]), vec![5.0, 6.0])?;
        assert_eq!(buffer.read_f32()?, vec![5.0, 6.0]);
        Ok(())
    }
}
