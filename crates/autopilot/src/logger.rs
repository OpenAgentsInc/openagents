use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{File, OpenOptions, create_dir_all};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub session_id: String,
    pub phase: String,
    pub event_type: String,
    pub data: Value,
}

pub struct SessionLogger {
    writer: Arc<Mutex<BufWriter<File>>>,
    session_id: String,
    pub log_path: PathBuf,
}

impl SessionLogger {
    pub fn new(session_id: &str) -> std::io::Result<Self> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let now = Local::now();
        let date_dir = now.format("%Y%m%d").to_string();

        let sessions_dir = PathBuf::from(&home)
            .join(".openagents/sessions")
            .join(&date_dir);

        create_dir_all(&sessions_dir)?;

        let log_path = sessions_dir.join(format!("{}.jsonl", session_id));

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;

        let writer = Arc::new(Mutex::new(BufWriter::new(file)));

        Ok(Self {
            writer,
            session_id: session_id.to_string(),
            log_path,
        })
    }

    pub fn log(&self, phase: &str, event_type: &str, data: Value) {
        let entry = LogEntry {
            timestamp: Local::now().to_rfc3339(),
            session_id: self.session_id.clone(),
            phase: phase.to_string(),
            event_type: event_type.to_string(),
            data,
        };

        if let Ok(json) = serde_json::to_string(&entry) {
            if let Ok(mut writer) = self.writer.lock() {
                let _ = writeln!(writer, "{}", json);
                let _ = writer.flush();
            }
        }
    }

    pub fn log_raw(&self, phase: &str, raw_json: &str) {
        if let Ok(data) = serde_json::from_str::<Value>(raw_json) {
            self.log(phase, "raw_message", data);
        } else {
            self.log(phase, "raw_message", serde_json::json!({ "raw": raw_json }));
        }
    }

    pub fn log_assistant(&self, phase: &str, message: &Value) {
        self.log(phase, "assistant", message.clone());
    }

    pub fn log_user(&self, phase: &str, message: &Value) {
        self.log(phase, "user", message.clone());
    }

    pub fn log_tool_use(&self, phase: &str, tool_name: &str, input: &Value) {
        self.log(
            phase,
            "tool_use",
            serde_json::json!({
                "tool": tool_name,
                "input": input
            }),
        );
    }

    pub fn log_tool_result(&self, phase: &str, tool_name: &str, result: &Value) {
        self.log(
            phase,
            "tool_result",
            serde_json::json!({
                "tool": tool_name,
                "result": result
            }),
        );
    }

    pub fn log_result(&self, phase: &str, result: &Value) {
        self.log(phase, "result", result.clone());
    }

    pub fn log_error(&self, phase: &str, error: &str) {
        self.log(phase, "error", serde_json::json!({ "error": error }));
    }

    pub fn log_phase_start(&self, phase: &str) {
        self.log(phase, "phase_start", serde_json::json!({ "phase": phase }));
    }

    pub fn log_phase_end(&self, phase: &str, summary: &str) {
        self.log(
            phase,
            "phase_end",
            serde_json::json!({
                "phase": phase,
                "summary": summary
            }),
        );
    }
}

impl Clone for SessionLogger {
    fn clone(&self) -> Self {
        Self {
            writer: self.writer.clone(),
            session_id: self.session_id.clone(),
            log_path: self.log_path.clone(),
        }
    }
}

pub fn generate_session_id() -> String {
    let now = Local::now();
    format!("{}-{:08x}", now.format("%H%M%S"), rand::random::<u32>())
}

mod rand {
    pub fn random<T>() -> T
    where
        T: From<u32>,
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        T::from(nanos)
    }
}
