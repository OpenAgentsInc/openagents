//! DaemonStatus component.
//!
//! Shows daemon/worker status in a compact card positioned bottom-left.
//! Displays worker status, PID, uptime, restarts, memory usage.
//! Includes buttons to start/stop daemon and restart worker.

use maud::{Markup, html};

/// Daemon status display.
#[derive(Default)]
pub struct DaemonStatus {
    /// Whether connected to daemon socket
    pub connected: bool,
    /// Worker status (running, stopped, restarting, failed)
    pub worker_status: String,
    /// Worker process ID
    pub worker_pid: Option<u32>,
    /// Worker uptime in seconds
    pub uptime_seconds: u64,
    /// Total restart count
    pub total_restarts: u64,
    /// Consecutive failures
    pub consecutive_failures: u32,
    /// Available memory in bytes
    pub memory_available_bytes: u64,
    /// Total memory in bytes
    pub memory_total_bytes: u64,
    /// Error message if any
    pub error: Option<String>,
}

impl DaemonStatus {
    /// Create a disconnected status.
    pub fn disconnected() -> Self {
        Self {
            connected: false,
            worker_status: "disconnected".to_string(),
            ..Default::default()
        }
    }

    /// Create a connected status with worker info.
    pub fn connected() -> Self {
        Self {
            connected: true,
            ..Default::default()
        }
    }

    pub fn worker_status(mut self, status: impl Into<String>) -> Self {
        self.worker_status = status.into();
        self
    }

    pub fn worker_pid(mut self, pid: u32) -> Self {
        self.worker_pid = Some(pid);
        self
    }

    pub fn uptime(mut self, seconds: u64) -> Self {
        self.uptime_seconds = seconds;
        self
    }

    pub fn restarts(mut self, total: u64, consecutive: u32) -> Self {
        self.total_restarts = total;
        self.consecutive_failures = consecutive;
        self
    }

    pub fn memory(mut self, available: u64, total: u64) -> Self {
        self.memory_available_bytes = available;
        self.memory_total_bytes = total;
        self
    }

    pub fn error(mut self, msg: impl Into<String>) -> Self {
        self.error = Some(msg.into());
        self
    }

    /// Render the component for positioning (call this for the full positioned version).
    /// Updates are pushed via WebSocket OOB swaps to #daemon-status-content.
    /// Positioned above the Claude status panel on the right side.
    pub fn build_positioned(self) -> Markup {
        html! {
            div
                id="daemon-status"
                style="position: fixed; bottom: 22rem; right: 1rem;"
            {
                div id="daemon-status-content" {
                    (self.build())
                }
            }
        }
    }

    /// Render just the card (for embedding or storybook).
    pub fn build(self) -> Markup {
        html! {
            div
                class="daemon-status-card"
                style="
                    background: #111;
                    border: 1px solid #333;
                    padding: 0.75rem 1rem;
                    font-family: 'Vera Mono', ui-monospace, monospace;
                    font-size: 0.65rem;
                    min-width: 220px;
                "
            {
                // Header row
                div style="display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.5rem;" {
                    // Status dot
                    span style={
                        "width: 6px; height: 6px; display: inline-block; "
                        "background: " (self.status_color()) ";"
                    } {}
                    span style="color: #888; text-transform: uppercase; letter-spacing: 0.05em;" {
                        "DAEMON"
                    }
                }

                @if !self.connected {
                    // Not connected state
                    div style="color: #666; margin-bottom: 0.5rem;" {
                        "Not connected"
                    }
                    @if let Some(ref err) = self.error {
                        div style="color: #a44; font-size: 0.55rem; margin-bottom: 0.5rem;" {
                            (err)
                        }
                    }
                    // Start button
                    div style="margin-top: 0.5rem;" {
                        button
                            style="
                                background: #222;
                                border: 1px solid #444;
                                color: #aaa;
                                padding: 0.35rem 0.75rem;
                                font-family: inherit;
                                font-size: 0.6rem;
                                cursor: pointer;
                                width: 100%;
                            "
                            hx-post="/api/daemon/start"
                            hx-swap="none"
                        {
                            "Start Daemon"
                        }
                    }
                } @else {
                    // Connected state - show worker info
                    div style="margin-bottom: 0.35rem;" {
                        span style="color: #555;" { "Worker: " }
                        span style={"color: " (self.worker_status_color()) ";"} {
                            (self.worker_status.as_str())
                        }
                    }

                    @if let Some(pid) = self.worker_pid {
                        div style="color: #555; margin-bottom: 0.25rem;" {
                            "PID: "
                            span style="color: #888;" { (pid) }
                        }
                    }

                    @if self.uptime_seconds > 0 {
                        div style="color: #555; margin-bottom: 0.25rem;" {
                            "Uptime: "
                            span style="color: #888;" { (format_duration(self.uptime_seconds)) }
                        }
                    }

                    div style="color: #555; margin-bottom: 0.35rem;" {
                        "Restarts: "
                        span style="color: #888;" { (self.total_restarts) }
                        @if self.consecutive_failures > 0 {
                            span style="color: #a44;" {
                                " (" (self.consecutive_failures) " consecutive)"
                            }
                        }
                    }

                    // Memory bar
                    @if self.memory_total_bytes > 0 {
                        div style="margin-top: 0.5rem;" {
                            div style="display: flex; justify-content: space-between; color: #555; margin-bottom: 0.2rem;" {
                                span { "Memory" }
                                span { (format_bytes(self.memory_available_bytes)) " free" }
                            }
                            div style="height: 4px; background: #333; width: 100%;" {
                                div style={
                                    "height: 100%; transition: width 0.3s; "
                                    "width: " (self.memory_used_percent()) "%; "
                                    "background: " (self.memory_bar_color()) ";"
                                } {}
                            }
                            div style="display: flex; justify-content: space-between; color: #444; font-size: 0.55rem; margin-top: 0.15rem;" {
                                span { (format_bytes(self.memory_total_bytes - self.memory_available_bytes)) " used" }
                                span { (format_bytes(self.memory_total_bytes)) " total" }
                            }
                        }
                    }

                    // Control buttons
                    div style="display: flex; gap: 0.5rem; margin-top: 0.75rem; border-top: 1px solid #333; padding-top: 0.5rem;" {
                        button
                            style="
                                flex: 1;
                                background: #222;
                                border: 1px solid #444;
                                color: #aaa;
                                padding: 0.35rem 0.5rem;
                                font-family: inherit;
                                font-size: 0.55rem;
                                cursor: pointer;
                            "
                            hx-post="/api/daemon/stop"
                            hx-swap="none"
                            hx-confirm="Stop the daemon? This will also stop the worker."
                        {
                            "Stop"
                        }
                        button
                            style="
                                flex: 1;
                                background: #222;
                                border: 1px solid #444;
                                color: #aaa;
                                padding: 0.35rem 0.5rem;
                                font-family: inherit;
                                font-size: 0.55rem;
                                cursor: pointer;
                            "
                            hx-post="/api/daemon/restart-worker"
                            hx-swap="none"
                        {
                            "Restart Worker"
                        }
                    }
                }
            }
        }
    }

    /// Get status indicator color
    fn status_color(&self) -> &'static str {
        if !self.connected {
            "#555" // gray - disconnected
        } else {
            match self.worker_status.as_str() {
                "running" => "#00A645", // green
                "restarting" | "starting" => "#FFB800", // yellow
                "stopped" => "#FF6600", // orange
                "failed" => "#FF0000", // red
                _ => "#555", // gray - unknown
            }
        }
    }

    /// Get worker status text color
    fn worker_status_color(&self) -> &'static str {
        match self.worker_status.as_str() {
            "running" => "#4a9",
            "restarting" | "starting" => "#aa4",
            "stopped" => "#a94",
            "failed" => "#a44",
            _ => "#888",
        }
    }

    /// Calculate memory used percentage
    fn memory_used_percent(&self) -> f64 {
        if self.memory_total_bytes == 0 {
            0.0
        } else {
            let used = self.memory_total_bytes - self.memory_available_bytes;
            (used as f64 / self.memory_total_bytes as f64) * 100.0
        }
    }

    /// Get memory bar color based on available memory
    fn memory_bar_color(&self) -> &'static str {
        let available_gb = self.memory_available_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        if available_gb > 4.0 {
            "#00A645" // green - plenty available
        } else if available_gb > 2.0 {
            "#FFB800" // yellow - moderate
        } else if available_gb > 1.0 {
            "#FF6600" // orange - low
        } else {
            "#FF0000" // red - critical
        }
    }
}

/// Format bytes as human-readable string
fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1}GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.0}MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.0}KB", bytes as f64 / 1024.0)
    } else {
        format!("{}B", bytes)
    }
}

/// Format duration from seconds
fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        let mins = secs / 60;
        let s = secs % 60;
        format!("{}m{}s", mins, s)
    } else if secs < 86400 {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        format!("{}h{}m", hours, mins)
    } else {
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        format!("{}d{}h", days, hours)
    }
}
