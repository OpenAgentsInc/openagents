//! AMD KFD backend discovery and readiness reporting for Mox.

use std::{fs, path::Path};

use mox_core::{DType, Device, DeviceKind};
use mox_runtime::{
    AmdBackendReport, AmdDeviceMetadata, AmdDriverBinding, AmdOptInStatus, AmdRecoveryAction,
    AmdRecoveryProfile, AmdRiskLevel, AmdRiskProfile, AmdRuntimeMode, AmdTopologyInfo, BackendName,
    DeviceDescriptor, DeviceDiscovery, HealthStatus, RuntimeError, RuntimeHealth,
};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "AMD KFD backend discovery and readiness";

const AMD_VENDOR_ID: u32 = 0x1002;
const KFD_DEVICE_NODE: &str = "/dev/kfd";
const DRM_ROOT: &str = "/sys/class/drm";

/// Discovery-only AMD KFD backend.
#[derive(Clone, Debug, Default)]
pub struct AmdKfdBackend;

impl AmdKfdBackend {
    /// Creates an AMD KFD backend probe.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Returns the backend-local AMD discovery report.
    pub fn discovery_report(&self) -> Result<AmdBackendReport, RuntimeError> {
        platform::discovery_report()
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
        match self.discovery_report() {
            Ok(report) => report.health,
            Err(error) => RuntimeHealth {
                status: HealthStatus::Degraded,
                message: format!("amd_kfd discovery failed: {error}"),
            },
        }
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
    use mox_runtime::{AmdOptInStatus, AmdRuntimeMode};

    use super::{kfd_health, AmdKfdBackend, HealthStatus};

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
    fn amd_kfd_reports_offline_outside_linux() -> Result<(), mox_runtime::RuntimeError> {
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
    fn amd_kfd_report_is_self_consistent_on_linux() -> Result<(), mox_runtime::RuntimeError> {
        let backend = AmdKfdBackend::new();
        let report = backend.discovery_report()?;
        assert_eq!(report.mode, AmdRuntimeMode::Kfd);
        assert_eq!(report.opt_in, AmdOptInStatus::NotRequired);
        match report.health.status {
            HealthStatus::Ready => assert!(!report.devices.is_empty()),
            HealthStatus::Degraded => assert!(!report.devices.is_empty()),
            HealthStatus::Offline => {}
        }
        Ok(())
    }
}
