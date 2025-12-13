//! Structured logging filesystem
//!
//! A FileService that provides append-only log files and structured event logging.
//! Perfect for capturing agent execution output and trajectory data.

use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::error::FsError;
use crate::service::{DirEntry, FileHandle, FileService, Metadata, OpenFlags};

/// Log severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::Info
    }
}

/// A structured log event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    /// Timestamp (Unix epoch seconds)
    pub timestamp: u64,
    /// Severity level
    pub level: LogLevel,
    /// Log message
    pub message: String,
    /// Optional structured data
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl LogEvent {
    /// Create a new log event with the current timestamp
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        LogEvent {
            timestamp: crate::services::mem_fs_now(),
            level,
            message: message.into(),
            data: None,
        }
    }

    /// Create a new log event with data
    pub fn with_data(level: LogLevel, message: impl Into<String>, data: serde_json::Value) -> Self {
        LogEvent {
            timestamp: crate::services::mem_fs_now(),
            level,
            message: message.into(),
            data: Some(data),
        }
    }

    /// Create an info event
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Info, message)
    }

    /// Create a debug event
    pub fn debug(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Debug, message)
    }

    /// Create a warning event
    pub fn warn(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Warn, message)
    }

    /// Create an error event
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(LogLevel::Error, message)
    }
}

/// Structured logging filesystem
///
/// Provides a file-based interface to logs:
///
/// ```text
/// /
/// ├── stdout.log     # Standard output (append-only write, full read)
/// ├── stderr.log     # Standard error (append-only write, full read)
/// └── events.jsonl   # Structured events in JSON Lines format
/// ```
///
/// # Example
///
/// ```rust
/// use oanix::services::{LogsFs, LogEvent, LogLevel};
/// use oanix::service::{FileService, OpenFlags};
///
/// let logs = LogsFs::new();
///
/// // Write to stdout programmatically
/// logs.write_stdout(b"Starting task...\n");
///
/// // Log a structured event
/// logs.log_event(LogEvent::info("Task started"));
///
/// // Or write via file interface
/// let mut handle = logs.open("/stdout.log", OpenFlags {
///     write: true,
///     append: true,
///     ..Default::default()
/// }).unwrap();
/// ```
pub struct LogsFs {
    stdout: Arc<RwLock<Vec<u8>>>,
    stderr: Arc<RwLock<Vec<u8>>>,
    events: Arc<RwLock<Vec<LogEvent>>>,
}

impl LogsFs {
    /// Create a new empty LogsFs
    pub fn new() -> Self {
        LogsFs {
            stdout: Arc::new(RwLock::new(Vec::new())),
            stderr: Arc::new(RwLock::new(Vec::new())),
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Write data to stdout
    pub fn write_stdout(&self, data: &[u8]) {
        self.stdout.write().unwrap().extend_from_slice(data);
    }

    /// Write data to stderr
    pub fn write_stderr(&self, data: &[u8]) {
        self.stderr.write().unwrap().extend_from_slice(data);
    }

    /// Log a structured event
    pub fn log_event(&self, event: LogEvent) {
        self.events.write().unwrap().push(event);
    }

    /// Log an info message
    pub fn info(&self, message: impl Into<String>) {
        self.log_event(LogEvent::info(message));
    }

    /// Log a debug message
    pub fn debug(&self, message: impl Into<String>) {
        self.log_event(LogEvent::debug(message));
    }

    /// Log a warning message
    pub fn warn(&self, message: impl Into<String>) {
        self.log_event(LogEvent::warn(message));
    }

    /// Log an error message
    pub fn error(&self, message: impl Into<String>) {
        self.log_event(LogEvent::error(message));
    }

    /// Get all stdout content
    pub fn stdout(&self) -> Vec<u8> {
        self.stdout.read().unwrap().clone()
    }

    /// Get all stderr content
    pub fn stderr(&self) -> Vec<u8> {
        self.stderr.read().unwrap().clone()
    }

    /// Get all events
    pub fn events(&self) -> Vec<LogEvent> {
        self.events.read().unwrap().clone()
    }

    /// Get events as JSONL (JSON Lines) format
    pub fn events_jsonl(&self) -> Vec<u8> {
        let events = self.events.read().unwrap();
        let mut output = Vec::new();
        for event in events.iter() {
            if let Ok(line) = serde_json::to_vec(event) {
                output.extend_from_slice(&line);
                output.push(b'\n');
            }
        }
        output
    }

    /// Clear all logs
    pub fn clear(&self) {
        self.stdout.write().unwrap().clear();
        self.stderr.write().unwrap().clear();
        self.events.write().unwrap().clear();
    }
}

impl Default for LogsFs {
    fn default() -> Self {
        Self::new()
    }
}

impl FileService for LogsFs {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError> {
        let path = path.trim_start_matches('/');

        match path {
            "stdout.log" => Ok(Box::new(LogFileHandle {
                buffer: self.stdout.clone(),
                position: 0,
                flags,
            })),
            "stderr.log" => Ok(Box::new(LogFileHandle {
                buffer: self.stderr.clone(),
                position: 0,
                flags,
            })),
            "events.jsonl" => {
                if flags.write {
                    Ok(Box::new(EventsWriteHandle {
                        events: self.events.clone(),
                        write_buffer: Vec::new(),
                    }))
                } else {
                    // Return current events as JSONL
                    let content = self.events_jsonl();
                    Ok(Box::new(EventsReadHandle {
                        content,
                        position: 0,
                    }))
                }
            }
            "" => Err(FsError::NotAFile("/".to_string())),
            _ => Err(FsError::NotFound(path.to_string())),
        }
    }

    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        let path = path.trim_start_matches('/').trim_end_matches('/');

        if !path.is_empty() {
            return Err(FsError::NotFound(path.to_string()));
        }

        Ok(vec![
            DirEntry {
                name: "events.jsonl".to_string(),
                is_dir: false,
                size: self.events_jsonl().len() as u64,
            },
            DirEntry {
                name: "stderr.log".to_string(),
                is_dir: false,
                size: self.stderr.read().unwrap().len() as u64,
            },
            DirEntry {
                name: "stdout.log".to_string(),
                is_dir: false,
                size: self.stdout.read().unwrap().len() as u64,
            },
        ])
    }

    fn stat(&self, path: &str) -> Result<Metadata, FsError> {
        let path = path.trim_start_matches('/');

        match path {
            "" => Ok(Metadata {
                is_dir: true,
                size: 3,
                modified: 0,
                readonly: false,
            }),
            "stdout.log" => Ok(Metadata {
                is_dir: false,
                size: self.stdout.read().unwrap().len() as u64,
                modified: 0,
                readonly: false,
            }),
            "stderr.log" => Ok(Metadata {
                is_dir: false,
                size: self.stderr.read().unwrap().len() as u64,
                modified: 0,
                readonly: false,
            }),
            "events.jsonl" => Ok(Metadata {
                is_dir: false,
                size: self.events_jsonl().len() as u64,
                modified: 0,
                readonly: false,
            }),
            _ => Err(FsError::NotFound(path.to_string())),
        }
    }

    fn mkdir(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied(
            "LogsFs has fixed structure".into(),
        ))
    }

    fn remove(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied(
            "LogsFs has fixed structure".into(),
        ))
    }

    fn rename(&self, _from: &str, _to: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied(
            "LogsFs has fixed structure".into(),
        ))
    }
}

/// File handle for stdout/stderr logs
struct LogFileHandle {
    buffer: Arc<RwLock<Vec<u8>>>,
    position: usize,
    flags: OpenFlags,
}

impl FileHandle for LogFileHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, FsError> {
        if !self.flags.read {
            return Err(FsError::PermissionDenied(
                "file not opened for reading".into(),
            ));
        }

        let data = self.buffer.read().unwrap();
        if self.position >= data.len() {
            return Ok(0);
        }

        let available = &data[self.position..];
        let to_read = buf.len().min(available.len());
        buf[..to_read].copy_from_slice(&available[..to_read]);
        self.position += to_read;
        Ok(to_read)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, FsError> {
        if !self.flags.write {
            return Err(FsError::PermissionDenied(
                "file not opened for writing".into(),
            ));
        }

        // Logs are always append-only
        let mut data = self.buffer.write().unwrap();
        data.extend_from_slice(buf);
        self.position = data.len();
        Ok(buf.len())
    }

    fn seek(&mut self, pos: u64) -> Result<(), FsError> {
        self.position = pos as usize;
        Ok(())
    }

    fn position(&self) -> u64 {
        self.position as u64
    }

    fn flush(&mut self) -> Result<(), FsError> {
        Ok(())
    }
}

/// Read handle for events.jsonl
struct EventsReadHandle {
    content: Vec<u8>,
    position: usize,
}

impl FileHandle for EventsReadHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, FsError> {
        if self.position >= self.content.len() {
            return Ok(0);
        }

        let available = &self.content[self.position..];
        let to_read = buf.len().min(available.len());
        buf[..to_read].copy_from_slice(&available[..to_read]);
        self.position += to_read;
        Ok(to_read)
    }

    fn write(&mut self, _buf: &[u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied(
            "events.jsonl opened for reading".into(),
        ))
    }

    fn seek(&mut self, pos: u64) -> Result<(), FsError> {
        self.position = pos as usize;
        Ok(())
    }

    fn position(&self) -> u64 {
        self.position as u64
    }

    fn flush(&mut self) -> Result<(), FsError> {
        Ok(())
    }
}

/// Write handle for events.jsonl
struct EventsWriteHandle {
    events: Arc<RwLock<Vec<LogEvent>>>,
    write_buffer: Vec<u8>,
}

impl FileHandle for EventsWriteHandle {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied(
            "events.jsonl opened for writing".into(),
        ))
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, FsError> {
        self.write_buffer.extend_from_slice(buf);

        // Try to parse complete lines
        while let Some(newline_pos) = self.write_buffer.iter().position(|&b| b == b'\n') {
            let line = &self.write_buffer[..newline_pos];
            if let Ok(event) = serde_json::from_slice::<LogEvent>(line) {
                self.events.write().unwrap().push(event);
            }
            self.write_buffer.drain(..=newline_pos);
        }

        Ok(buf.len())
    }

    fn seek(&mut self, _pos: u64) -> Result<(), FsError> {
        // Seeking not meaningful for append
        Ok(())
    }

    fn position(&self) -> u64 {
        0
    }

    fn flush(&mut self) -> Result<(), FsError> {
        // Try to parse any remaining content as an event
        if !self.write_buffer.is_empty() {
            if let Ok(event) = serde_json::from_slice::<LogEvent>(&self.write_buffer) {
                self.events.write().unwrap().push(event);
            }
            self.write_buffer.clear();
        }
        Ok(())
    }
}

impl Drop for EventsWriteHandle {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdout_programmatic() {
        let logs = LogsFs::new();

        logs.write_stdout(b"Hello, ");
        logs.write_stdout(b"World!\n");

        let content = logs.stdout();
        assert_eq!(content, b"Hello, World!\n");
    }

    #[test]
    fn test_stderr_programmatic() {
        let logs = LogsFs::new();

        logs.write_stderr(b"Error: something failed\n");

        let content = logs.stderr();
        assert_eq!(content, b"Error: something failed\n");
    }

    #[test]
    fn test_events_programmatic() {
        let logs = LogsFs::new();

        logs.info("Starting task");
        logs.debug("Debug info");
        logs.warn("Warning message");
        logs.error("Error occurred");

        let events = logs.events();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].level, LogLevel::Info);
        assert_eq!(events[1].level, LogLevel::Debug);
        assert_eq!(events[2].level, LogLevel::Warn);
        assert_eq!(events[3].level, LogLevel::Error);
    }

    #[test]
    fn test_stdout_file_interface() {
        let logs = LogsFs::new();

        // Write via file
        {
            let mut handle = logs
                .open(
                    "/stdout.log",
                    OpenFlags {
                        write: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            handle.write(b"Line 1\n").unwrap();
            handle.write(b"Line 2\n").unwrap();
        }

        // Read via file
        {
            let mut handle = logs.open("/stdout.log", OpenFlags::read_only()).unwrap();
            let mut buf = vec![0u8; 1024];
            let n = handle.read(&mut buf).unwrap();
            assert_eq!(&buf[..n], b"Line 1\nLine 2\n");
        }
    }

    #[test]
    fn test_events_jsonl_read() {
        let logs = LogsFs::new();

        logs.log_event(LogEvent::info("Test message"));
        logs.log_event(LogEvent::with_data(
            LogLevel::Debug,
            "With data",
            serde_json::json!({"key": "value"}),
        ));

        let mut handle = logs.open("/events.jsonl", OpenFlags::read_only()).unwrap();
        let mut buf = vec![0u8; 4096];
        let n = handle.read(&mut buf).unwrap();
        let content = String::from_utf8_lossy(&buf[..n]);

        // Should be valid JSONL
        let lines: Vec<&str> = content.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);

        // Each line should be valid JSON
        for line in lines {
            let _event: LogEvent = serde_json::from_str(line).unwrap();
        }
    }

    #[test]
    fn test_events_jsonl_write() {
        let logs = LogsFs::new();

        // Write events via file
        {
            let mut handle = logs
                .open(
                    "/events.jsonl",
                    OpenFlags {
                        write: true,
                        ..Default::default()
                    },
                )
                .unwrap();

            let event1 = LogEvent::info("Event 1");
            let event2 = LogEvent::warn("Event 2");

            let mut line1 = serde_json::to_vec(&event1).unwrap();
            line1.push(b'\n');
            let mut line2 = serde_json::to_vec(&event2).unwrap();
            line2.push(b'\n');

            handle.write(&line1).unwrap();
            handle.write(&line2).unwrap();
            handle.flush().unwrap();
        }

        // Verify events were parsed
        let events = logs.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].message, "Event 1");
        assert_eq!(events[1].message, "Event 2");
    }

    #[test]
    fn test_readdir() {
        let logs = LogsFs::new();
        logs.write_stdout(b"test");

        let entries = logs.readdir("/").unwrap();
        assert_eq!(entries.len(), 3);

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"stdout.log"));
        assert!(names.contains(&"stderr.log"));
        assert!(names.contains(&"events.jsonl"));
    }

    #[test]
    fn test_stat() {
        let logs = LogsFs::new();
        logs.write_stdout(b"hello");

        let meta = logs.stat("/").unwrap();
        assert!(meta.is_dir);

        let meta = logs.stat("/stdout.log").unwrap();
        assert!(!meta.is_dir);
        assert_eq!(meta.size, 5);
    }

    #[test]
    fn test_clear() {
        let logs = LogsFs::new();

        logs.write_stdout(b"stdout content");
        logs.write_stderr(b"stderr content");
        logs.info("Event");

        assert!(!logs.stdout().is_empty());
        assert!(!logs.stderr().is_empty());
        assert!(!logs.events().is_empty());

        logs.clear();

        assert!(logs.stdout().is_empty());
        assert!(logs.stderr().is_empty());
        assert!(logs.events().is_empty());
    }
}
