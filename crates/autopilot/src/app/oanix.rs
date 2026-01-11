use oanix::manifest::{GpuDevice, InferenceBackend, RelayStatus};

pub(crate) fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * KB;
    const GB: f64 = 1024.0 * MB;
    const TB: f64 = 1024.0 * GB;

    let value = bytes as f64;
    if value >= TB {
        format!("{:.1} TB", value / TB)
    } else if value >= GB {
        format!("{:.1} GB", value / GB)
    } else if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{} B", bytes)
    }
}

pub(crate) fn format_bool(value: bool) -> &'static str {
    if value {
        "Yes"
    } else {
        "No"
    }
}

pub(crate) fn format_latency(latency_ms: Option<u32>) -> String {
    match latency_ms {
        Some(value) => format!("{} ms", value),
        None => "-".to_string(),
    }
}

pub(crate) fn format_gpu_summary(gpu: &GpuDevice) -> String {
    format!(
        "{} ({}, {})",
        gpu.name,
        gpu.backend,
        if gpu.available { "ready" } else { "offline" }
    )
}

pub(crate) fn format_backend_summary(backend: &InferenceBackend) -> String {
    let readiness = if backend.ready { "ready" } else { "offline" };
    let model_count = backend.models.len();
    if let Some(endpoint) = &backend.endpoint {
        format!(
            "{} - {} • {} models • {}",
            backend.name, readiness, model_count, endpoint
        )
    } else {
        format!(
            "{} - {} • {} models",
            backend.name, readiness, model_count
        )
    }
}

pub(crate) fn format_relay_summary(relay: &RelayStatus) -> String {
    let status = if relay.connected { "connected" } else { "offline" };
    let latency = format_latency(relay.latency_ms);
    format!("{} - {} • {}", relay.url, status, latency)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oanix::manifest::{GpuDevice, InferenceBackend, RelayStatus};

    #[test]
    fn formats_bytes_across_units() {
        assert_eq!(format_bytes(42), "42 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    }

    #[test]
    fn formats_latency_and_bool() {
        assert_eq!(format_latency(Some(12)), "12 ms");
        assert_eq!(format_latency(None), "-");
        assert_eq!(format_bool(true), "Yes");
        assert_eq!(format_bool(false), "No");
    }

    #[test]
    fn formats_backend_gpu_and_relay_summaries() {
        let backend = InferenceBackend {
            id: "codex".to_string(),
            name: "Codex CLI".to_string(),
            endpoint: None,
            models: vec!["gpt-4o".to_string()],
            ready: true,
        };
        assert!(format_backend_summary(&backend).contains("Codex"));

        let gpu = GpuDevice {
            name: "M2".to_string(),
            backend: "Metal".to_string(),
            available: true,
        };
        assert!(format_gpu_summary(&gpu).contains("Metal"));

        let relay = RelayStatus {
            url: "wss://relay.example".to_string(),
            connected: false,
            latency_ms: None,
        };
        assert!(format_relay_summary(&relay).contains("offline"));
    }
}
