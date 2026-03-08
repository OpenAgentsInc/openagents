//! CUDA backend discovery and readiness reporting for Mox.

use std::{io::ErrorKind, process::Command};

use mox_core::{DType, Device, DeviceKind};
use mox_runtime::{
    BackendName, DeviceDescriptor, DeviceDiscovery, HealthStatus, NvidiaBackendReport,
    NvidiaDeviceMetadata, NvidiaRecoveryAction, NvidiaRecoveryProfile, NvidiaRiskLevel,
    NvidiaRiskProfile, NvidiaTopologyInfo, RuntimeError, RuntimeHealth,
};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "CUDA backend discovery and truthful readiness";

const NVIDIA_SMI_BINARY: &str = "nvidia-smi";
const INVENTORY_QUERY: &str = concat!(
    "index,name,pci.bus_id,memory.total,compute_cap,display_attached,",
    "mig.mode.current,persistence_mode,addressing_mode"
);
const OFFLINE_NO_DRIVER_MESSAGE: &str =
    "cuda backend unavailable: nvidia-smi is not installed or the NVIDIA driver is not reachable";

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

/// CUDA backend probe backed by `nvidia-smi` discovery.
#[derive(Clone, Debug, Default)]
pub struct CudaBackend;

impl CudaBackend {
    /// Creates a CUDA backend probe.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Returns the backend-local NVIDIA discovery report.
    pub fn discovery_report(&self) -> Result<NvidiaBackendReport, RuntimeError> {
        match query_inventory() {
            Ok(rows) => {
                let devices = rows
                    .into_iter()
                    .map(NvidiaInventoryRow::into_device_descriptor)
                    .collect::<Result<Vec<_>, _>>()?;
                let health = cuda_health(&devices);
                Ok(NvidiaBackendReport { devices, health })
            }
            Err(error) if error.kind == NvidiaQueryErrorKind::NotInstalled => {
                Ok(NvidiaBackendReport {
                    devices: Vec::new(),
                    health: RuntimeHealth {
                        status: HealthStatus::Offline,
                        message: String::from(OFFLINE_NO_DRIVER_MESSAGE),
                    },
                })
            }
            Err(error) => Ok(NvidiaBackendReport {
                devices: Vec::new(),
                health: RuntimeHealth {
                    status: HealthStatus::Offline,
                    message: error.message,
                },
            }),
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
        match self.discovery_report() {
            Ok(report) => report.health,
            Err(error) => RuntimeHealth {
                status: HealthStatus::Degraded,
                message: format!("cuda discovery failed: {error}"),
            },
        }
    }
}

impl NvidiaInventoryRow {
    fn into_device_descriptor(self) -> Result<DeviceDescriptor, RuntimeError> {
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

        Ok(DeviceDescriptor {
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
        })
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

#[cfg(test)]
mod tests {
    use super::{
        CudaBackend, HealthStatus, architecture_from_compute_capability, cuda_health,
        parse_inventory_row, parse_mig_profile, recovery_profile, risk_profile,
    };
    use mox_runtime::{DeviceDiscovery, NvidiaRecoveryAction, NvidiaRiskLevel};

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
    fn cuda_health_is_degraded_when_a_discovered_device_is_elevated() {
        let devices = vec![
            parse_inventory_row(
                "0, NVIDIA GeForce RTX 4080, 00000000:01:00.0, 16376, 8.9, Yes, [N/A], Disabled, HMM",
            )
            .expect("inventory row should parse")
            .into_device_descriptor()
            .expect("device descriptor should build"),
        ];
        let health = cuda_health(&devices);
        assert_eq!(health.status, HealthStatus::Degraded);
        assert!(health.message.contains("display-attached"));
    }

    #[test]
    fn cuda_backend_report_is_self_consistent() -> Result<(), mox_runtime::RuntimeError> {
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
}
