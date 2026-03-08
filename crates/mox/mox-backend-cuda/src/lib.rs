//! CUDA backend architecture and truthful readiness surface for Mox.

use mox_runtime::{
    BackendName, DeviceDescriptor, DeviceDiscovery, HealthStatus, RuntimeError, RuntimeHealth,
};

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "CUDA backend architecture and truthful readiness";

const CUDA_ARCHITECTURE_MESSAGE: &str =
    "cuda backend architecture is present but NVIDIA discovery and execution are not landed yet";

/// CUDA backend placeholder that stays explicit about its pre-discovery state.
#[derive(Clone, Debug, Default)]
pub struct CudaBackend;

impl CudaBackend {
    /// Creates a CUDA backend probe.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Returns the current truthful backend health.
    #[must_use]
    pub fn architecture_health(&self) -> RuntimeHealth {
        RuntimeHealth {
            status: HealthStatus::Offline,
            message: String::from(CUDA_ARCHITECTURE_MESSAGE),
        }
    }
}

impl DeviceDiscovery for CudaBackend {
    fn backend_name(&self) -> BackendName {
        "cuda"
    }

    fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError> {
        Ok(Vec::new())
    }

    fn health(&self) -> RuntimeHealth {
        self.architecture_health()
    }
}

#[cfg(test)]
mod tests {
    use super::CudaBackend;
    use mox_runtime::{DeviceDiscovery, HealthStatus};

    #[test]
    fn cuda_backend_reports_explicit_offline_architecture_state() {
        let backend = CudaBackend::new();
        assert_eq!(backend.backend_name(), "cuda");
        assert!(backend.discover_devices().unwrap_or_default().is_empty());
        let health = backend.health();
        assert_eq!(health.status, HealthStatus::Offline);
        assert_eq!(
            health.message,
            "cuda backend architecture is present but NVIDIA discovery and execution are not landed yet"
        );
    }
}
