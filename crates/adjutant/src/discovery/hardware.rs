//! Hardware discovery - CPU, RAM, GPU detection.

use crate::manifest::{GpuDevice, HardwareManifest};
use sysinfo::System;

/// Discover hardware capabilities.
pub async fn discover_hardware() -> anyhow::Result<HardwareManifest> {
    let mut sys = System::new_all();
    sys.refresh_all();

    // CPU info
    let cpu_cores = sys.cpus().len() as u32;
    let cpu_model = sys
        .cpus()
        .first()
        .map(|cpu| cpu.brand().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    // RAM info
    let ram_bytes = sys.total_memory();
    // available_memory() returns 0 on macOS, use total - used instead
    let ram_available = ram_bytes.saturating_sub(sys.used_memory());

    // GPU detection - platform specific
    let gpus = detect_gpus();

    Ok(HardwareManifest {
        cpu_cores,
        cpu_model,
        ram_bytes,
        ram_available,
        gpus,
    })
}

/// Detect GPU devices.
fn detect_gpus() -> Vec<GpuDevice> {
    let mut gpus = Vec::new();

    // macOS: Check for Metal support
    #[cfg(target_os = "macos")]
    {
        // On Apple Silicon, the GPU is integrated
        if std::env::consts::ARCH == "aarch64" {
            gpus.push(GpuDevice {
                name: "Apple Silicon GPU".to_string(),
                backend: "Metal".to_string(),
                available: true,
            });
        }
    }

    // Linux: Check for NVIDIA
    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/dev/nvidia0").exists() {
            gpus.push(GpuDevice {
                name: "NVIDIA GPU".to_string(),
                backend: "CUDA".to_string(),
                available: true,
            });
        }
    }

    gpus
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_discover_hardware() {
        let hw = discover_hardware().await.unwrap();
        assert!(hw.cpu_cores > 0);
        assert!(hw.ram_bytes > 0);
    }
}
