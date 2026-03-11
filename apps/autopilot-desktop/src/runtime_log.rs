use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, mpsc};

use chrono::Utc;
use serde_json::{Value, json};
use tracing::Level;
use wgpui::components::sections::TerminalStream;

const ENV_AUTOPILOT_LOG_DIR: &str = "OPENAGENTS_AUTOPILOT_LOG_DIR";
const DEFAULT_MAX_SESSION_FILES: usize = 12;

static SESSION_LOG_WRITER: OnceLock<SessionLogWriter> = OnceLock::new();

#[derive(Clone)]
struct SessionLogWriter {
    tx: mpsc::Sender<SessionLogCommand>,
    session_id: String,
}

enum SessionLogCommand {
    Entry(Value),
    #[cfg(test)]
    Flush(mpsc::Sender<()>),
}

#[derive(Clone, Debug)]
struct SessionLogConfig {
    base_dir: PathBuf,
    max_session_files: usize,
    session_id: String,
}

pub(crate) fn init_default_session_logging() {
    let _ = session_log_writer();
}

pub(crate) fn record_tracing_event(
    level: &Level,
    target: &str,
    message: Option<&str>,
    fields: serde_json::Map<String, Value>,
    line: &str,
) {
    let target = target.trim();
    let line = line.trim();
    if target.is_empty() || line.is_empty() {
        return;
    }

    session_log_writer().record_entry(json!({
        "source": "tracing",
        "target": target,
        "level": level.as_str(),
        "message": message.map(str::trim).filter(|value| !value.is_empty()),
        "fields": fields,
        "line": line,
    }));
}

pub(crate) fn record_mission_control_line(
    stream: TerminalStream,
    rendered_line: impl Into<String>,
    dedupe_key: Option<&str>,
) {
    let rendered_line = rendered_line.into();
    let line = rendered_line.trim();
    if line.is_empty() {
        return;
    }

    session_log_writer().record_entry(json!({
        "source": "mission_control",
        "stream": terminal_stream_label(&stream),
        "line": line,
        "dedupe_key": dedupe_key.map(str::to_string),
    }));
}

#[cfg(test)]
fn session_log_writer_for_tests(config: SessionLogConfig) -> SessionLogWriter {
    SessionLogWriter::spawn(config)
}

fn session_log_writer() -> &'static SessionLogWriter {
    SESSION_LOG_WRITER.get_or_init(|| SessionLogWriter::spawn(default_session_log_config()))
}

impl SessionLogWriter {
    fn spawn(config: SessionLogConfig) -> Self {
        let SessionLogConfig {
            base_dir,
            max_session_files,
            session_id,
        } = config;
        let session_dir = base_dir.join("sessions");
        let session_path = session_dir.join(format!("{session_id}.jsonl"));
        let latest_path = base_dir.join("latest.jsonl");
        let (tx, rx) = mpsc::channel();
        let thread_session_id = session_id.clone();
        let thread_base_dir = base_dir.clone();
        let thread_session_path = session_path.clone();
        let thread_latest_path = latest_path.clone();
        let thread_name = format!("autopilot-session-log-{thread_session_id}");
        let spawn_result = std::thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                run_session_log_writer(
                    rx,
                    thread_base_dir,
                    session_dir,
                    thread_session_path,
                    thread_latest_path,
                    thread_session_id,
                    max_session_files,
                );
            });
        if let Err(error) = spawn_result {
            eprintln!("Autopilot session log writer failed to start: {error}");
        }

        Self { tx, session_id }
    }

    fn record_entry(&self, entry: Value) {
        let payload = with_session_metadata(entry, self.session_id.as_str());
        let _ = self.tx.send(SessionLogCommand::Entry(payload));
    }

    #[cfg(test)]
    fn flush(&self) {
        let (tx, rx) = mpsc::channel();
        let _ = self.tx.send(SessionLogCommand::Flush(tx));
        let _ = rx.recv_timeout(std::time::Duration::from_secs(2));
    }
}

fn with_session_metadata(mut entry: Value, session_id: &str) -> Value {
    if let Value::Object(object) = &mut entry {
        object.insert(
            "timestamp_ms".to_string(),
            Value::from(current_timestamp_ms()),
        );
        object.insert(
            "session_id".to_string(),
            Value::String(session_id.to_string()),
        );
    }
    entry
}

fn run_session_log_writer(
    rx: mpsc::Receiver<SessionLogCommand>,
    base_dir: PathBuf,
    session_dir: PathBuf,
    session_path: PathBuf,
    latest_path: PathBuf,
    session_id: String,
    max_session_files: usize,
) {
    let mut file = initialize_session_log_file(
        base_dir.as_path(),
        session_dir.as_path(),
        session_path.as_path(),
        latest_path.as_path(),
        max_session_files,
    );

    if let Some(file_handle) = file.as_mut() {
        let started = json!({
            "timestamp_ms": current_timestamp_ms(),
            "session_id": session_id,
            "source": "session",
            "event": "started",
            "pid": std::process::id(),
            "version": env!("CARGO_PKG_VERSION"),
            "log_dir": base_dir.display().to_string(),
            "session_path": session_path.display().to_string(),
        });
        if let Err(error) = write_jsonl_entry(file_handle, &started) {
            eprintln!(
                "Autopilot session log entry write failed for {}: {}",
                session_path.display(),
                error
            );
            file = None;
        }
    }

    while let Ok(command) = rx.recv() {
        match command {
            SessionLogCommand::Entry(entry) => {
                let Some(file_handle) = file.as_mut() else {
                    continue;
                };
                if let Err(error) = write_jsonl_entry(file_handle, &entry) {
                    eprintln!(
                        "Autopilot session log entry write failed for {}: {}",
                        session_path.display(),
                        error
                    );
                    file = None;
                }
            }
            #[cfg(test)]
            SessionLogCommand::Flush(flush_tx) => {
                if let Some(file_handle) = file.as_mut() {
                    let _ = file_handle.flush();
                }
                let _ = flush_tx.send(());
            }
        }
    }
}

fn initialize_session_log_file(
    base_dir: &Path,
    session_dir: &Path,
    session_path: &Path,
    latest_path: &Path,
    max_session_files: usize,
) -> Option<File> {
    if let Err(error) = fs::create_dir_all(base_dir) {
        eprintln!(
            "Autopilot session log directory initialization failed for {}: {}",
            base_dir.display(),
            error
        );
        return None;
    }
    if let Err(error) = fs::create_dir_all(session_dir) {
        eprintln!(
            "Autopilot session log directory initialization failed for {}: {}",
            session_dir.display(),
            error
        );
        return None;
    }
    if let Err(error) = prune_old_session_logs(session_dir, max_session_files) {
        eprintln!(
            "Autopilot session log retention cleanup failed for {}: {}",
            session_dir.display(),
            error
        );
    }

    let file = match OpenOptions::new()
        .create(true)
        .append(true)
        .open(session_path)
    {
        Ok(file) => file,
        Err(error) => {
            eprintln!(
                "Autopilot session log file open failed for {}: {}",
                session_path.display(),
                error
            );
            return None;
        }
    };
    if let Err(error) = refresh_latest_alias(session_path, latest_path) {
        eprintln!(
            "Autopilot latest session log alias update failed for {}: {}",
            latest_path.display(),
            error
        );
    }
    Some(file)
}

fn write_jsonl_entry(file: &mut File, entry: &Value) -> Result<(), String> {
    let line = serde_json::to_string(entry)
        .map_err(|error| format!("serialize session log entry: {error}"))?;
    file.write_all(line.as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .map_err(|error| format!("append session log line: {error}"))?;
    file.flush()
        .map_err(|error| format!("flush session log file: {error}"))?;
    Ok(())
}

fn refresh_latest_alias(session_path: &Path, latest_path: &Path) -> Result<(), String> {
    match fs::remove_file(latest_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "remove existing latest alias {}: {error}",
                latest_path.display()
            ));
        }
    }

    if fs::hard_link(session_path, latest_path).is_ok() {
        return Ok(());
    }

    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink(session_path, latest_path).is_ok() {
            return Ok(());
        }
    }

    fs::copy(session_path, latest_path)
        .map(|_| ())
        .map_err(|error| format!("copy latest alias {}: {error}", latest_path.display()))
}

fn prune_old_session_logs(session_dir: &Path, max_session_files: usize) -> Result<(), String> {
    let mut files = fs::read_dir(session_dir)
        .map_err(|error| {
            format!(
                "read session log directory {}: {error}",
                session_dir.display()
            )
        })?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_type()
                .map(|kind| kind.is_file())
                .unwrap_or(false)
        })
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("jsonl"))
        .collect::<Vec<_>>();
    if files.len() <= max_session_files {
        return Ok(());
    }

    files.sort_by_key(|entry| entry.file_name());
    let remove_count = files.len().saturating_sub(max_session_files);
    for entry in files.into_iter().take(remove_count) {
        fs::remove_file(entry.path()).map_err(|error| {
            format!(
                "remove stale session log {}: {error}",
                entry.path().display()
            )
        })?;
    }
    Ok(())
}

fn terminal_stream_label(stream: &TerminalStream) -> &'static str {
    match stream {
        TerminalStream::Stdout => "stdout",
        TerminalStream::Stderr => "stderr",
    }
}

fn default_session_log_config() -> SessionLogConfig {
    SessionLogConfig {
        base_dir: default_autopilot_log_dir(),
        max_session_files: DEFAULT_MAX_SESSION_FILES,
        session_id: default_session_id(),
    }
}

fn default_session_id() -> String {
    format!(
        "{}-pid{}",
        Utc::now().format("%Y%m%dT%H%M%SZ"),
        std::process::id()
    )
}

fn default_autopilot_log_dir() -> PathBuf {
    #[cfg(test)]
    let fallback_home = Some(std::env::temp_dir().join(format!(
        "openagents-autopilot-test-home-{}",
        std::process::id()
    )));
    #[cfg(not(test))]
    let fallback_home = dirs::home_dir();

    resolve_log_dir_from(std::env::var_os(ENV_AUTOPILOT_LOG_DIR), fallback_home)
}

fn resolve_log_dir_from(override_dir: Option<OsString>, fallback_home: Option<PathBuf>) -> PathBuf {
    if let Some(override_dir) = override_dir {
        let override_dir = PathBuf::from(override_dir);
        if !override_dir.as_os_str().is_empty() {
            return override_dir;
        }
    }

    fallback_home
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openagents")
        .join("logs")
        .join("autopilot")
}

fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use serde_json::Value;
    use tempfile::tempdir;

    use super::{
        DEFAULT_MAX_SESSION_FILES, SessionLogConfig, current_timestamp_ms, resolve_log_dir_from,
        session_log_writer_for_tests,
    };

    #[test]
    fn resolve_log_dir_prefers_override() {
        let home = PathBuf::from("/tmp/ignored-home");
        let dir = resolve_log_dir_from(
            Some(OsString::from("/tmp/autopilot-logs")),
            Some(home.clone()),
        );

        assert_eq!(dir, PathBuf::from("/tmp/autopilot-logs"));
        assert_eq!(
            resolve_log_dir_from(None, Some(home)),
            PathBuf::from("/tmp/ignored-home/.openagents/logs/autopilot")
        );
    }

    #[test]
    fn session_writer_appends_entries_and_updates_latest_alias() {
        let temp = tempdir().expect("create temp log dir");
        let writer = session_log_writer_for_tests(SessionLogConfig {
            base_dir: temp.path().join("logs"),
            max_session_files: DEFAULT_MAX_SESSION_FILES,
            session_id: "20260311T214500Z-pid12345".to_string(),
        });

        writer.record_entry(serde_json::json!({
            "timestamp_ms": current_timestamp_ms(),
            "source": "tracing",
            "target": "autopilot_desktop::provider",
            "level": "INFO",
            "line": "INFO autopilot_desktop::provider: Provider queued Apple FM execution",
        }));
        writer.record_entry(serde_json::json!({
            "timestamp_ms": current_timestamp_ms(),
            "source": "mission_control",
            "stream": "stdout",
            "line": "[21:45:00] Provider online. Heartbeat and relay intake are active.",
        }));
        writer.flush();

        let session_path = temp
            .path()
            .join("logs")
            .join("sessions")
            .join("20260311T214500Z-pid12345.jsonl");
        let latest_path = temp.path().join("logs").join("latest.jsonl");
        let lines = std::fs::read_to_string(&session_path)
            .expect("read session log")
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("parse session log line"))
            .collect::<Vec<_>>();

        assert!(latest_path.exists());
        assert!(
            lines.iter().any(|entry| {
                entry.get("source") == Some(&Value::String("tracing".to_string()))
                    && entry.get("target")
                        == Some(&Value::String("autopilot_desktop::provider".to_string()))
            }),
            "expected provider tracing entry in session log"
        );
        assert!(
            lines.iter().any(|entry| {
                entry.get("source") == Some(&Value::String("mission_control".to_string()))
                    && entry.get("stream") == Some(&Value::String("stdout".to_string()))
            }),
            "expected mission control entry in session log"
        );
        assert_eq!(
            std::fs::read_to_string(&latest_path).expect("read latest session log"),
            std::fs::read_to_string(&session_path).expect("read session log for comparison")
        );
    }
}
