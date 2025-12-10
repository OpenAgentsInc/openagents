//! Auto-detection of container backends

use crate::backend::{ContainerBackend, NoOpBackend};
use crate::docker::DockerBackend;
use crate::macos::MacOSContainerBackend;
use std::sync::Arc;

/// Detect and return the best available container backend.
///
/// Priority:
/// 1. macOS Container (if on macOS 26+ with `container` CLI)
/// 2. Docker (all platforms where docker CLI is available)
/// 3. NoOp backend (no sandboxing available)
pub async fn detect_backend() -> Arc<dyn ContainerBackend> {
    // Check for platform override
    let platform_override = std::env::var("OA_SANDBOX_PLATFORM").ok();
    let platform = platform_override
        .as_deref()
        .unwrap_or(std::env::consts::OS);

    // Try macOS Container first (on Darwin)
    if platform == "macos" {
        let macos_backend = MacOSContainerBackend::new();
        if macos_backend.is_available().await {
            tracing::info!("Using macOS Container backend");
            return Arc::new(macos_backend);
        }
    }

    // Try Docker
    let docker_backend = DockerBackend::new();
    if docker_backend.is_available().await {
        tracing::info!("Using Docker backend");
        return Arc::new(docker_backend);
    }

    // Fallback to no-op
    tracing::warn!("No container runtime available");
    Arc::new(NoOpBackend)
}

/// Check if any container backend is available
pub async fn is_container_available() -> bool {
    let backend = detect_backend().await;
    backend.is_available().await
}

/// Get the name of the available container backend
pub async fn get_backend_name() -> &'static str {
    let backend = detect_backend().await;
    backend.name()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detect_backend() {
        // This test just verifies the detection runs without panicking
        let backend = detect_backend().await;
        let name = backend.name();
        assert!(!name.is_empty());
    }
}
