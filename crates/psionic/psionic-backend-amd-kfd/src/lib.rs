//! AMD KFD backend discovery, staging allocation, and submission substrate for Psionic.

#![allow(dead_code)]
#![cfg_attr(
    test,
    allow(unused_imports, clippy::expect_used, clippy::panic_in_result_fn)
)]

use std::{
    fmt, fs,
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
pub const CRATE_ROLE: &str = "AMD KFD backend discovery, staging allocation, and submission";

const AMD_VENDOR_ID: u32 = 0x1002;
const KFD_DEVICE_NODE: &str = "/dev/kfd";
const DRM_ROOT: &str = "/sys/class/drm";
const AMD_POOL_MAX_CACHED_BUFFERS: usize = 128;
const AMD_POOL_MAX_CACHED_BYTES: u64 = 64 * 1024 * 1024;

/// Exact execution-substrate ops that are truthful before graph lowering lands.
pub const EXECUTION_SUBSTRATE_SUPPORTED_OPS: &[&str] =
    &["input", "constant", "fill_buffer", "copy_buffer"];

/// KFD-visible memory class surfaced by Psionic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmdKfdMemorySpace {
    /// Host-visible GTT-style staging memory owned by the backend.
    HostVisibleGtt,
}

/// How long Psionic should wait after recording a KFD submission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmdKfdCommandWait {
    /// Wait until the recorded submission completes.
    Completed,
}

/// Stable submission lifecycle state exposed by Psionic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmdKfdCommandStatus {
    /// The submission was recorded by Psionic.
    Submitted,
    /// The submission completed successfully.
    Completed,
    /// The submission failed.
    Error,
}

/// Submission metadata returned after an AMD KFD submission is committed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AmdKfdSubmissionReport {
    /// Final submission status observed by Psionic.
    pub status: AmdKfdCommandStatus,
    /// Number of explicit operations recorded in the submission.
    pub encoded_operations: usize,
}

/// Backend-owned AMD KFD buffer.
#[derive(Clone)]
pub struct AmdKfdBuffer {
    spec: TensorSpec,
    byte_len: usize,
    memory_space: AmdKfdMemorySpace,
    host_visible: bool,
    storage: Arc<Mutex<Vec<u8>>>,
}

impl fmt::Debug for AmdKfdBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AmdKfdBuffer")
            .field("spec", &self.spec)
            .field("byte_len", &self.byte_len)
            .field("memory_space", &self.memory_space)
            .field("host_visible", &self.host_visible)
            .field("storage", &"<amd kfd staging buffer>")
            .finish()
    }
}

impl AmdKfdBuffer {
    /// Returns the backing allocation size in bytes.
    #[must_use]
    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// Returns the AMD KFD memory space backing the buffer.
    #[must_use]
    pub const fn memory_space(&self) -> AmdKfdMemorySpace {
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
                "amd_kfd buffer write length mismatch: expected {}, actual {}",
                self.byte_len,
                bytes.len()
            )));
        }
        self.storage
            .lock()
            .map_err(|error| {
                RuntimeError::Backend(format!("amd_kfd buffer lock poisoned: {error}"))
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
                RuntimeError::Backend(format!("amd_kfd buffer lock poisoned: {error}"))
            })?
            .clone())
    }

    /// Writes contiguous `f32` values into an `f32` buffer.
    pub fn write_f32(&mut self, values: &[f32]) -> Result<(), RuntimeError> {
        if self.spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "amd_kfd write_f32 requires F32 buffer, actual {:?}",
                self.spec.dtype()
            )));
        }
        if values.len() != self.spec.storage_size() {
            return Err(RuntimeError::Backend(format!(
                "amd_kfd buffer write length mismatch: expected {} values, actual {}",
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
                "amd_kfd read_f32 requires F32 buffer, actual {:?}",
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

impl BufferHandle for AmdKfdBuffer {
    fn spec(&self) -> &TensorSpec {
        &self.spec
    }
}

/// Explicit submission over the AMD KFD staging substrate.
pub struct AmdKfdSubmission {
    encoded_operations: usize,
    status: AmdKfdCommandStatus,
}

impl AmdKfdSubmission {
    /// Fills a buffer with one repeated byte value.
    pub fn fill_buffer(&mut self, buffer: &AmdKfdBuffer, value: u8) -> Result<(), RuntimeError> {
        let mut storage = buffer.storage.lock().map_err(|error| {
            RuntimeError::Backend(format!("amd_kfd buffer lock poisoned: {error}"))
        })?;
        storage.fill(value);
        self.encoded_operations += 1;
        Ok(())
    }

    /// Copies one AMD KFD buffer into another with explicit size checking.
    pub fn copy_buffer(
        &mut self,
        source: &AmdKfdBuffer,
        destination: &AmdKfdBuffer,
    ) -> Result<(), RuntimeError> {
        if source.byte_len != destination.byte_len {
            return Err(RuntimeError::Backend(format!(
                "amd_kfd buffer copy length mismatch: source {}, destination {}",
                source.byte_len, destination.byte_len
            )));
        }
        let source_bytes = source.read_bytes()?;
        destination
            .storage
            .lock()
            .map_err(|error| {
                RuntimeError::Backend(format!("amd_kfd buffer lock poisoned: {error}"))
            })?
            .copy_from_slice(&source_bytes);
        self.encoded_operations += 1;
        Ok(())
    }

    /// Completes the explicit submission.
    pub fn commit(
        mut self,
        wait: AmdKfdCommandWait,
    ) -> Result<AmdKfdSubmissionReport, RuntimeError> {
        match wait {
            AmdKfdCommandWait::Completed => {
                self.status = AmdKfdCommandStatus::Completed;
            }
        }
        Ok(AmdKfdSubmissionReport {
            status: self.status,
            encoded_operations: self.encoded_operations,
        })
    }
}

enum AmdKfdBackendState {
    Available(Box<AvailableAmdKfdBackend>),
    Unavailable(RuntimeHealth),
}

struct AvailableAmdKfdBackend {
    descriptor: DeviceDescriptor,
    allocator_pool: AllocatorPoolReport,
    kernel_cache: KernelCacheReport,
}

/// AMD KFD backend discovery plus staging allocation/submission substrate.
pub struct AmdKfdBackend {
    state: AmdKfdBackendState,
}

impl Default for AmdKfdBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AmdKfdBackend {
    /// Creates an AMD KFD backend and selects the first execution-ready device
    /// when `/dev/kfd` and an AMD DRM device are both available.
    #[must_use]
    pub fn new() -> Self {
        match platform::discovery_report() {
            Ok(report) => {
                let Some(descriptor) = report.devices.first().cloned() else {
                    return Self {
                        state: AmdKfdBackendState::Unavailable(report.health),
                    };
                };
                if report.health.status != HealthStatus::Ready {
                    return Self {
                        state: AmdKfdBackendState::Unavailable(report.health),
                    };
                }
                Self {
                    state: AmdKfdBackendState::Available(Box::new(AvailableAmdKfdBackend {
                        descriptor,
                        allocator_pool: AllocatorPoolReport {
                            policy: amd_allocator_pool_policy(),
                            state: AllocatorPoolState::default(),
                        },
                        kernel_cache: KernelCacheReport {
                            policy: KernelCachePolicy::disabled(),
                            state: KernelCacheState::default(),
                        },
                    })),
                }
            }
            Err(error) => Self {
                state: AmdKfdBackendState::Unavailable(RuntimeHealth {
                    status: HealthStatus::Offline,
                    message: error.to_string(),
                }),
            },
        }
    }

    /// Returns the backend-local AMD discovery report.
    pub fn discovery_report(&self) -> Result<AmdBackendReport, RuntimeError> {
        platform::discovery_report()
    }

    /// Returns the device selected for allocation/submission, when available.
    #[must_use]
    pub fn selected_device(&self) -> Option<&DeviceDescriptor> {
        match &self.state {
            AmdKfdBackendState::Available(backend) => Some(&backend.descriptor),
            AmdKfdBackendState::Unavailable(_) => None,
        }
    }

    /// Returns explicit runtime-resource posture for the selected AMD KFD device.
    #[must_use]
    pub fn runtime_resources(&self) -> Option<BackendRuntimeResources> {
        match &self.state {
            AmdKfdBackendState::Available(backend) => Some(BackendRuntimeResources {
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
            AmdKfdBackendState::Unavailable(_) => None,
        }
    }

    /// Creates a dense `f32` input buffer on the selected AMD KFD device.
    pub fn input_buffer(
        &mut self,
        shape: Shape,
        values: impl Into<Vec<f32>>,
    ) -> Result<AmdKfdBuffer, RuntimeError> {
        let Some(device) = self
            .selected_device()
            .map(|descriptor| descriptor.device.clone())
        else {
            return Err(RuntimeError::Backend(String::from(
                "amd_kfd backend unavailable: no selected execution device",
            )));
        };
        let mut buffer = self.allocate(&TensorSpec::new(shape, DType::F32, device))?;
        buffer.write_f32(values.into().as_slice())?;
        Ok(buffer)
    }

    /// Returns explicit backend-selection truth for the current AMD KFD execution substrate.
    pub fn backend_selection(
        &self,
        supported_ops: &[&str],
    ) -> Result<BackendSelection, RuntimeError> {
        let Some(backend) = self.selected_backend() else {
            return Err(RuntimeError::Backend(format!(
                "amd_kfd backend unavailable: {}",
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

    /// Returns an explicit fallback selection when AMD KFD cannot execute the requested path.
    pub fn fallback_selection<B>(
        &self,
        fallback_backend: &B,
        supported_ops: &[&str],
    ) -> Result<BackendSelection, RuntimeError>
    where
        B: DeviceDiscovery + ?Sized,
    {
        match &self.state {
            AmdKfdBackendState::Available(_) => self.backend_selection(supported_ops),
            AmdKfdBackendState::Unavailable(health) => Ok(BackendSelection::fallback_with_policy(
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
                format!("amd_kfd backend unavailable: {}", health.message),
            )
            .with_runtime_resources(fallback_backend.runtime_resources())
            .with_backend_extensions(fallback_backend.extension_support())),
        }
    }

    /// Allocates an AMD KFD staging buffer for the provided tensor specification.
    pub fn allocate_buffer(&mut self, spec: &TensorSpec) -> Result<AmdKfdBuffer, RuntimeError> {
        let Some(backend) = self.selected_backend() else {
            return Err(RuntimeError::Backend(self.health().message));
        };
        if spec.dtype() != DType::F32 {
            return Err(RuntimeError::Backend(format!(
                "amd_kfd staging surface only supports F32 buffers, actual {:?}",
                spec.dtype()
            )));
        }
        if spec.device().kind() != DeviceKind::AmdKfd {
            return Err(RuntimeError::Backend(format!(
                "amd_kfd allocator requires an AMD KFD tensor spec, actual device kind {}",
                spec.device().kind()
            )));
        }
        if spec.device().ordinal() != backend.descriptor.device.ordinal() {
            return Err(RuntimeError::Backend(format!(
                "amd_kfd allocator requires device ordinal {}, actual {}",
                backend.descriptor.device.ordinal(),
                spec.device().ordinal()
            )));
        }
        backend.allocate(spec)
    }

    /// Begins an explicit AMD KFD submission.
    pub fn begin_submission(&self) -> Result<AmdKfdSubmission, RuntimeError> {
        if self.selected_backend().is_none() {
            return Err(RuntimeError::Backend(self.health().message));
        }
        Ok(AmdKfdSubmission {
            encoded_operations: 0,
            status: AmdKfdCommandStatus::Submitted,
        })
    }

    fn selected_backend(&self) -> Option<&AvailableAmdKfdBackend> {
        match &self.state {
            AmdKfdBackendState::Available(backend) => Some(backend),
            AmdKfdBackendState::Unavailable(_) => None,
        }
    }
}

impl DeviceDiscovery for AmdKfdBackend {
    fn backend_name(&self) -> BackendName {
        "amd_kfd"
    }

    fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError> {
        self.discovery_report().map(|report| report.devices)
    }

    fn health(&self) -> RuntimeHealth {
        match &self.state {
            AmdKfdBackendState::Available(_) => match self.discovery_report() {
                Ok(report) => report.health,
                Err(error) => RuntimeHealth {
                    status: HealthStatus::Degraded,
                    message: format!("amd_kfd discovery failed: {error}"),
                },
            },
            AmdKfdBackendState::Unavailable(health) => health.clone(),
        }
    }

    fn runtime_resources(&self) -> Option<BackendRuntimeResources> {
        AmdKfdBackend::runtime_resources(self)
    }
}

impl Allocator for AmdKfdBackend {
    type Buffer = AmdKfdBuffer;

    fn allocate(&mut self, spec: &TensorSpec) -> Result<Self::Buffer, RuntimeError> {
        self.allocate_buffer(spec)
    }
}

impl AvailableAmdKfdBackend {
    fn allocate(&self, spec: &TensorSpec) -> Result<AmdKfdBuffer, RuntimeError> {
        let byte_len = spec
            .storage_size()
            .checked_mul(size_of_dtype(spec.dtype()))
            .ok_or_else(|| {
                RuntimeError::Backend(format!(
                    "amd_kfd buffer size overflow for tensor storage size {}",
                    spec.storage_size()
                ))
            })?;
        Ok(AmdKfdBuffer {
            spec: spec.clone(),
            byte_len,
            memory_space: AmdKfdMemorySpace::HostVisibleGtt,
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

fn kfd_health(device_count: usize, has_kfd: bool) -> RuntimeHealth {
    match (device_count, has_kfd) {
        (0, false) => RuntimeHealth {
            status: HealthStatus::Offline,
            message: String::from(
                "amd_kfd unavailable: /dev/kfd is missing and no AMD DRM devices were detected",
            ),
        },
        (0, true) => RuntimeHealth {
            status: HealthStatus::Offline,
            message: String::from("amd_kfd found /dev/kfd but no AMD DRM devices were detected"),
        },
        (count, true) => RuntimeHealth {
            status: HealthStatus::Ready,
            message: format!("amd_kfd ready on {count} AMD device(s)"),
        },
        (count, false) => RuntimeHealth {
            status: HealthStatus::Degraded,
            message: format!("amd_kfd discovered {count} AMD device(s) but /dev/kfd is missing"),
        },
    }
}

fn risk_profile() -> AmdRiskProfile {
    AmdRiskProfile {
        level: AmdRiskLevel::Standard,
        requires_explicit_opt_in: false,
        may_unbind_kernel_driver: false,
        warnings: Vec::new(),
    }
}

fn recovery_profile(driver_binding: AmdDriverBinding) -> AmdRecoveryProfile {
    AmdRecoveryProfile {
        driver_binding,
        expected_actions: vec![
            AmdRecoveryAction::KernelDriverReset,
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

fn driver_binding(device_dir: &Path) -> AmdDriverBinding {
    let Some(driver_name) = fs::canonicalize(device_dir.join("driver"))
        .ok()
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
    else {
        return AmdDriverBinding::Unknown;
    };
    if driver_name == "amdgpu" {
        AmdDriverBinding::KernelAmdgpu
    } else {
        AmdDriverBinding::Unknown
    }
}

fn probe_device(path: &Path, ordinal: u16, has_kfd: bool) -> Option<DeviceDescriptor> {
    let device_dir = path.join("device");
    if read_hex_u32(&device_dir.join("vendor"))? != AMD_VENDOR_ID {
        return None;
    }

    let pci_bdf = pci_bdf(&device_dir);
    let architecture = read_trimmed(&device_dir.join("gfx_target_version"));
    let vram_bytes = read_u64(&device_dir.join("mem_info_vram_total"));
    let visible_vram_bytes = read_u64(&device_dir.join("mem_info_vis_vram_total"));
    let driver_binding = driver_binding(&device_dir);
    let device_name = read_trimmed(&device_dir.join("product_name")).or_else(|| {
        read_trimmed(&device_dir.join("device")).map(|device_id| format!("AMD GPU {device_id}"))
    });

    let mut feature_flags = Vec::new();
    if has_kfd {
        feature_flags.push(String::from("kfd_device_node"));
        feature_flags.push(String::from("execution_substrate_candidate"));
    }
    if driver_binding == AmdDriverBinding::KernelAmdgpu {
        feature_flags.push(String::from("amdgpu_driver"));
    }
    if visible_vram_bytes.is_some() {
        feature_flags.push(String::from("visible_vram"));
    }

    Some(DeviceDescriptor {
        backend: String::from("amd_kfd"),
        device: Device::new(
            DeviceKind::AmdKfd,
            ordinal,
            Some(format!("amd_kfd:{ordinal}")),
        ),
        device_name,
        supported_dtypes: vec![DType::F32],
        supported_quantization: Vec::new(),
        memory_capacity_bytes: vram_bytes.or(visible_vram_bytes),
        unified_memory: Some(false),
        feature_flags,
        amd_metadata: Some(AmdDeviceMetadata {
            mode: AmdRuntimeMode::Kfd,
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
            recovery: recovery_profile(driver_binding),
        }),
        nvidia_metadata: None,
    })
}

fn discover_devices_linux(has_kfd: bool) -> Result<Vec<DeviceDescriptor>, RuntimeError> {
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
        if let Some(device) = probe_device(&entry.path(), devices.len() as u16, has_kfd) {
            devices.push(device);
        }
    }

    Ok(devices)
}

mod platform {
    use super::*;

    #[cfg(target_os = "linux")]
    pub(super) fn discovery_report() -> Result<AmdBackendReport, RuntimeError> {
        let has_kfd = Path::new(KFD_DEVICE_NODE).exists();
        let devices = discover_devices_linux(has_kfd)?;
        let health = kfd_health(devices.len(), has_kfd);
        Ok(AmdBackendReport {
            mode: AmdRuntimeMode::Kfd,
            opt_in: AmdOptInStatus::NotRequired,
            devices,
            health,
        })
    }

    #[cfg(not(target_os = "linux"))]
    pub(super) fn discovery_report() -> Result<AmdBackendReport, RuntimeError> {
        Ok(AmdBackendReport {
            mode: AmdRuntimeMode::Kfd,
            opt_in: AmdOptInStatus::NotRequired,
            devices: Vec::new(),
            health: RuntimeHealth {
                status: HealthStatus::Offline,
                message: String::from(
                    "amd_kfd backend requires Linux /dev/kfd and AMD DRM devices",
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
        Allocator, AmdOptInStatus, AmdRuntimeMode, BackendSelectionState, DeviceDiscovery,
        ServedProductBackendPolicy, ValidationCoverage, validation_reference_for_backend_probe,
    };

    use super::{
        AMD_POOL_MAX_CACHED_BUFFERS, AmdKfdBackend, AmdKfdCommandStatus, AmdKfdCommandWait,
        AmdKfdMemorySpace, EXECUTION_SUBSTRATE_SUPPORTED_OPS, HealthStatus, kfd_health,
    };

    #[test]
    fn amd_kfd_health_is_offline_without_devices_or_kfd() {
        let health = kfd_health(0, false);
        assert_eq!(health.status, HealthStatus::Offline);
    }

    #[test]
    fn amd_kfd_health_is_degraded_when_devices_exist_without_kfd() {
        let health = kfd_health(1, false);
        assert_eq!(health.status, HealthStatus::Degraded);
        assert!(health.message.contains("/dev/kfd"));
    }

    #[test]
    fn amd_kfd_health_is_ready_when_devices_and_kfd_exist() {
        let health = kfd_health(2, true);
        assert_eq!(health.status, HealthStatus::Ready);
        assert!(health.message.contains("2 AMD device"));
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn amd_kfd_reports_offline_outside_linux() -> Result<(), psionic_runtime::RuntimeError> {
        let backend = AmdKfdBackend::new();
        let report = backend.discovery_report()?;
        assert_eq!(report.mode, AmdRuntimeMode::Kfd);
        assert_eq!(report.opt_in, AmdOptInStatus::NotRequired);
        assert!(report.devices.is_empty());
        assert_eq!(report.health.status, HealthStatus::Offline);
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn amd_kfd_report_is_self_consistent_on_linux() -> Result<(), psionic_runtime::RuntimeError> {
        let backend = AmdKfdBackend::new();
        let report = backend.discovery_report()?;
        let validation = validation_reference_for_backend_probe(backend.backend_name());
        assert_eq!(report.mode, AmdRuntimeMode::Kfd);
        assert_eq!(report.opt_in, AmdOptInStatus::NotRequired);
        assert_eq!(validation.claim_id, "amd_kfd.discovery");
        assert_eq!(validation.coverage, ValidationCoverage::DiscoveryReadiness);
        match report.health.status {
            HealthStatus::Ready => assert!(!report.devices.is_empty()),
            HealthStatus::Degraded => assert!(!report.devices.is_empty()),
            HealthStatus::Offline => {}
        }
        Ok(())
    }

    #[test]
    fn amd_kfd_runtime_resources_are_explicit_when_execution_substrate_is_available() {
        let backend = AmdKfdBackend::new();
        if let Some(descriptor) = backend.selected_device() {
            let resources = backend
                .runtime_resources()
                .expect("available amd_kfd backend should surface runtime resources");
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
    fn amd_kfd_backend_selection_reports_direct_execution_or_explicit_cpu_fallback()
    -> Result<(), psionic_runtime::RuntimeError> {
        let backend = AmdKfdBackend::new();
        if backend.selected_device().is_some() {
            let selection = backend.backend_selection(EXECUTION_SUBSTRATE_SUPPORTED_OPS)?;
            assert_eq!(selection.requested_backend, "amd_kfd");
            assert_eq!(selection.effective_backend, "amd_kfd");
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
        assert_eq!(selection.requested_backend, "amd_kfd");
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
    fn amd_kfd_allocates_and_submits_copy_when_available()
    -> Result<(), psionic_runtime::RuntimeError> {
        let mut backend = AmdKfdBackend::new();
        let Some(device) = backend.selected_device().cloned() else {
            assert_ne!(backend.health().status, HealthStatus::Ready);
            return Ok(());
        };

        let spec = TensorSpec::new(Shape::new(vec![4]), DType::F32, device.device.clone());
        let mut left = backend.allocate(&spec)?;
        let right = backend.allocate(&spec)?;
        assert_eq!(left.memory_space(), AmdKfdMemorySpace::HostVisibleGtt);
        assert!(left.host_visible());
        left.write_f32(&[1.0, 2.0, 3.0, 4.0])?;

        let mut submission = backend.begin_submission()?;
        submission.copy_buffer(&left, &right)?;
        let report = submission.commit(AmdKfdCommandWait::Completed)?;
        assert_eq!(report.status, AmdKfdCommandStatus::Completed);
        assert_eq!(report.encoded_operations, 1);
        assert_eq!(right.read_f32()?, vec![1.0, 2.0, 3.0, 4.0]);
        Ok(())
    }

    #[test]
    fn amd_kfd_input_buffer_populates_f32_values_when_available()
    -> Result<(), psionic_runtime::RuntimeError> {
        let mut backend = AmdKfdBackend::new();
        if backend.selected_device().is_none() {
            return Ok(());
        }

        let buffer = backend.input_buffer(Shape::new(vec![2]), vec![5.0, 6.0])?;
        assert_eq!(buffer.read_f32()?, vec![5.0, 6.0]);
        Ok(())
    }
}
