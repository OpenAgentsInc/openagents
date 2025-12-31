use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct DaemonStatus {
    pub connected: bool,
    pub worker_status: String,
    pub worker_pid: Option<u32>,
    pub uptime_seconds: u64,
    pub total_restarts: u64,
    pub consecutive_failures: u32,
    pub memory_available_bytes: u64,
    pub memory_total_bytes: u64,
    pub error: Option<String>,
}

pub trait DaemonClient {
    fn status(&self) -> DaemonStatus;
}

pub struct UnixDaemonClient {
    socket_path: PathBuf,
    timeout: Duration,
}

impl UnixDaemonClient {
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            timeout: Duration::from_secs(2),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }

    fn default_socket_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".autopilot")
            .join("autopilotd.sock")
    }
}

impl Default for UnixDaemonClient {
    fn default() -> Self {
        Self::new(Self::default_socket_path())
    }
}

impl DaemonClient for UnixDaemonClient {
    fn status(&self) -> DaemonStatus {
        #[cfg(not(unix))]
        {
            return DaemonStatus {
                connected: false,
                worker_status: "unsupported".to_string(),
                error: Some("Daemon client requires unix sockets".to_string()),
                ..DaemonStatus::default()
            };
        }

        #[cfg(unix)]
        {
            use std::io::{Read, Write};
            use std::os::unix::net::UnixStream;

            let mut stream = match UnixStream::connect(&self.socket_path) {
                Ok(stream) => stream,
                Err(err) => {
                    return DaemonStatus {
                        connected: false,
                        worker_status: "disconnected".to_string(),
                        error: Some(format!("Connect failed: {}", err)),
                        ..DaemonStatus::default()
                    };
                }
            };

            let _ = stream.set_read_timeout(Some(self.timeout));
            let _ = stream.set_write_timeout(Some(self.timeout));

            let request = serde_json::json!({ "type": "Status" });
            let request_bytes = match serde_json::to_vec(&request) {
                Ok(bytes) => bytes,
                Err(err) => {
                    return DaemonStatus {
                        connected: false,
                        worker_status: "error".to_string(),
                        error: Some(format!("Encode failed: {}", err)),
                        ..DaemonStatus::default()
                    };
                }
            };

            if let Err(err) = stream.write_all(&request_bytes) {
                return DaemonStatus {
                    connected: false,
                    worker_status: "error".to_string(),
                    error: Some(format!("Write failed: {}", err)),
                    ..DaemonStatus::default()
                };
            }

            let mut buf = vec![0u8; 8192];
            let read_len = match stream.read(&mut buf) {
                Ok(len) => len,
                Err(err) => {
                    return DaemonStatus {
                        connected: false,
                        worker_status: "error".to_string(),
                        error: Some(format!("Read failed: {}", err)),
                        ..DaemonStatus::default()
                    };
                }
            };

            let response: serde_json::Value = match serde_json::from_slice(&buf[..read_len]) {
                Ok(value) => value,
                Err(err) => {
                    return DaemonStatus {
                        connected: false,
                        worker_status: "error".to_string(),
                        error: Some(format!("Parse failed: {}", err)),
                        ..DaemonStatus::default()
                    };
                }
            };

            let data = response.get("data");
            let worker_status = data
                .and_then(|d| d.get("worker_status"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            DaemonStatus {
                connected: data.is_some(),
                worker_status,
                worker_pid: data
                    .and_then(|d| d.get("worker_pid"))
                    .and_then(|v| v.as_u64())
                    .map(|pid| pid as u32),
                uptime_seconds: data
                    .and_then(|d| d.get("uptime_seconds"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                total_restarts: data
                    .and_then(|d| d.get("total_restarts"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                consecutive_failures: data
                    .and_then(|d| d.get("consecutive_failures"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                memory_available_bytes: data
                    .and_then(|d| d.get("memory_available_bytes"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                memory_total_bytes: data
                    .and_then(|d| d.get("memory_total_bytes"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                error: if data.is_some() {
                    None
                } else {
                    Some("Missing data".to_string())
                },
            }
        }
    }
}
