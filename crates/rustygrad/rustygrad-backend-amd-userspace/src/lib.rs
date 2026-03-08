//! AMD userspace backend discovery, opt-in gating, and readiness reporting for Rustygrad.

use std::{env, fs, path::Path};

use rustygrad_core::{DType, Device, DeviceKind};
use rustygrad_runtime::{
    AmdBackendReport, AmdDeviceMetadata, AmdDriverBinding, AmdOptInStatus, AmdRecoveryAction,
    AmdRecoveryProfile, AmdRiskLevel, AmdRiskProfile, AmdRuntimeMode, AmdTopologyInfo, BackendName,
    DeviceDescriptor, DeviceDiscovery, HealthStatus, RuntimeError, RuntimeHealth,
};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "AMD userspace backend discovery, opt-in, and readiness";

const AMD_VENDOR_ID: u32 = 0x1002;
const DRM_ROOT: &str = "/sys/class/drm";
const AMDGPU_MODULE: &str = "/sys/module/amdgpu";
const OPT_IN_ENV: &str = "RUSTYGRAD_AMD_USERSPACE_ENABLE";

/// Discovery-only AMD userspace backend.
#[derive(Clone, Debug, Default)]
pub struct AmdUserspaceBackend;

impl AmdUserspaceBackend {
    /// Creates an AMD userspace backend probe.
    #[must_use]
    pub const fn new() -> Self {
        Self
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
}

impl DeviceDiscovery for AmdUserspaceBackend {
    fn backend_name(&self) -> BackendName {
        "amd_userspace"
    }

    fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError> {
        self.discovery_report().map(|report| report.devices)
    }

    fn health(&self) -> RuntimeHealth {
        match self.discovery_report() {
            Ok(report) => report.health,
            Err(error) => RuntimeHealth {
                status: HealthStatus::Degraded,
                message: format!("amd_userspace discovery failed: {error}"),
            },
        }
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
        AmdOptInStatus::Enabled => feature_flags.push(String::from("userspace_opt_in_enabled")),
        AmdOptInStatus::Disabled => feature_flags.push(String::from("userspace_opt_in_disabled")),
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
        let opt_in = if opt_in_enabled() {
            AmdOptInStatus::Enabled
        } else {
            AmdOptInStatus::Disabled
        };
        Ok(AmdBackendReport {
            mode: AmdRuntimeMode::Userspace,
            opt_in,
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
    use rustygrad_runtime::{AmdOptInStatus, AmdRuntimeMode};

    use super::{parse_opt_in_value, userspace_health, AmdUserspaceBackend, HealthStatus};

    #[test]
    fn userspace_opt_in_parser_recognizes_enabled_values() {
        for value in ["1", "true", "yes", "on", "enabled"] {
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
    fn amd_userspace_reports_offline_outside_linux() -> Result<(), rustygrad_runtime::RuntimeError>
    {
        let backend = AmdUserspaceBackend::new();
        let report = backend.discovery_report()?;
        assert_eq!(report.mode, AmdRuntimeMode::Userspace);
        assert!(report.devices.is_empty());
        assert_eq!(report.health.status, HealthStatus::Offline);
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn amd_userspace_report_is_self_consistent_on_linux(
    ) -> Result<(), rustygrad_runtime::RuntimeError> {
        let backend = AmdUserspaceBackend::new();
        let report = backend.discovery_report()?;
        assert_eq!(report.mode, AmdRuntimeMode::Userspace);
        match report.opt_in {
            AmdOptInStatus::Disabled => assert_eq!(report.health.status, HealthStatus::Offline),
            AmdOptInStatus::Enabled => match report.health.status {
                HealthStatus::Ready | HealthStatus::Degraded | HealthStatus::Offline => {}
            },
            AmdOptInStatus::NotRequired => panic!("userspace backend must report explicit opt-in"),
        }
        Ok(())
    }
}
