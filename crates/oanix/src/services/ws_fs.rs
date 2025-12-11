//! WebSocket capability service for agent environments
//!
//! Provides WebSocket connection management as a filesystem interface.
//! Uses a buffer-based design where actual network I/O is handled externally.
//!
//! # File Layout
//!
//! ```text
//! /cap/ws/
//! ├── control           # Write commands: {"open": "wss://..."} or {"close": "conn-id"}
//! ├── status            # Overall service status
//! └── conns/
//!     └── {id}/
//!         ├── in        # Read incoming messages (FIFO queue)
//!         ├── out       # Write outgoing messages (queued for send)
//!         ├── status    # Connection state (connecting/open/closing/closed/error)
//!         └── url       # The WebSocket URL (read-only)
//! ```
//!
//! # Design
//!
//! WsFs uses a buffer-based design consistent with Plan 9 philosophy:
//! - Agent writes to `/conns/{id}/out` to queue messages
//! - Agent reads from `/conns/{id}/in` to receive messages
//! - External transport connector handles actual WebSocket I/O
//!
//! This keeps WsFs testable and portable (works in WASM too).
//!
//! # Example
//!
//! ```rust,ignore
//! use oanix::services::WsFs;
//! use oanix::service::{FileService, OpenFlags};
//!
//! let ws = WsFs::new();
//!
//! // Open a connection via control file
//! write_file(&ws, "/control", r#"{"open": "wss://relay.example.com"}"#);
//!
//! // List connections
//! let conns = ws.readdir("/conns")?;
//!
//! // Write to connection
//! write_file(&ws, "/conns/conn-0/out", "Hello, WebSocket!");
//!
//! // External connector would:
//! // 1. Read pending messages from outbox
//! // 2. Send to actual WebSocket
//! // 3. Receive from WebSocket
//! // 4. Add to inbox via ws.receive_message(conn_id, msg)
//!
//! // Agent reads incoming messages
//! let msg = read_file(&ws, "/conns/conn-0/in");
//! ```

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::error::FsError;
use crate::service::{DirEntry, FileHandle, FileService, Metadata, OpenFlags};

/// WebSocket connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WsState {
    /// Connection is being established
    Connecting,
    /// Connection is open and ready
    Open,
    /// Connection is closing
    Closing,
    /// Connection is closed
    Closed,
    /// Connection encountered an error
    Error,
}

impl WsState {
    pub fn as_str(&self) -> &'static str {
        match self {
            WsState::Connecting => "connecting",
            WsState::Open => "open",
            WsState::Closing => "closing",
            WsState::Closed => "closed",
            WsState::Error => "error",
        }
    }
}

/// A WebSocket connection
#[derive(Debug)]
pub struct WsConnection {
    /// Connection ID
    pub id: String,
    /// WebSocket URL
    pub url: String,
    /// Connection state
    pub state: WsState,
    /// Error message if state is Error
    pub error: Option<String>,
    /// Outgoing message queue (agent writes here)
    pub outbox: VecDeque<Vec<u8>>,
    /// Incoming message queue (transport writes here)
    pub inbox: VecDeque<Vec<u8>>,
    /// Timestamp when connection was opened
    pub opened_at: u64,
}

impl WsConnection {
    fn new(id: String, url: String) -> Self {
        Self {
            id,
            url,
            state: WsState::Connecting,
            error: None,
            outbox: VecDeque::new(),
            inbox: VecDeque::new(),
            opened_at: now(),
        }
    }
}

/// WebSocket capability service
///
/// Provides WebSocket connection management through a filesystem interface.
/// Actual network I/O is handled by an external transport connector.
pub struct WsFs {
    /// Active connections
    connections: Arc<RwLock<HashMap<String, WsConnection>>>,
    /// Connection ID counter
    next_id: AtomicU64,
    /// Maximum connections allowed
    max_connections: usize,
}

impl WsFs {
    /// Create a new WsFs with default settings
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            next_id: AtomicU64::new(0),
            max_connections: 16,
        }
    }

    /// Create with custom max connections
    pub fn with_max_connections(max: usize) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            next_id: AtomicU64::new(0),
            max_connections: max,
        }
    }

    /// Open a new WebSocket connection
    ///
    /// Returns the connection ID. The connection starts in `Connecting` state.
    /// External transport should call `set_connected()` when actually connected.
    pub fn open_connection(&self, url: impl Into<String>) -> Result<String, FsError> {
        let mut conns = self.connections.write().unwrap();

        if conns.len() >= self.max_connections {
            return Err(FsError::Io("max connections reached".into()));
        }

        let id = format!("conn-{}", self.next_id.fetch_add(1, Ordering::SeqCst));
        let conn = WsConnection::new(id.clone(), url.into());
        conns.insert(id.clone(), conn);

        Ok(id)
    }

    /// Close a connection
    ///
    /// Sets state to `Closing`. External transport should call `set_closed()`
    /// after the actual close completes.
    pub fn close_connection(&self, id: &str) -> Result<(), FsError> {
        let mut conns = self.connections.write().unwrap();
        let conn = conns
            .get_mut(id)
            .ok_or_else(|| FsError::NotFound(format!("connection {}", id)))?;

        conn.state = WsState::Closing;
        Ok(())
    }

    /// Mark connection as connected (called by transport)
    pub fn set_connected(&self, id: &str) -> Result<(), FsError> {
        let mut conns = self.connections.write().unwrap();
        let conn = conns
            .get_mut(id)
            .ok_or_else(|| FsError::NotFound(format!("connection {}", id)))?;

        conn.state = WsState::Open;
        Ok(())
    }

    /// Mark connection as closed (called by transport)
    pub fn set_closed(&self, id: &str) -> Result<(), FsError> {
        let mut conns = self.connections.write().unwrap();
        let conn = conns
            .get_mut(id)
            .ok_or_else(|| FsError::NotFound(format!("connection {}", id)))?;

        conn.state = WsState::Closed;
        Ok(())
    }

    /// Mark connection as error (called by transport)
    pub fn set_error(&self, id: &str, error: impl Into<String>) -> Result<(), FsError> {
        let mut conns = self.connections.write().unwrap();
        let conn = conns
            .get_mut(id)
            .ok_or_else(|| FsError::NotFound(format!("connection {}", id)))?;

        conn.state = WsState::Error;
        conn.error = Some(error.into());
        Ok(())
    }

    /// Remove a closed connection
    pub fn remove_connection(&self, id: &str) -> Option<WsConnection> {
        let mut conns = self.connections.write().unwrap();
        conns.remove(id)
    }

    /// Queue a message for sending (called when agent writes to /out)
    pub fn send_message(&self, id: &str, data: Vec<u8>) -> Result<(), FsError> {
        let mut conns = self.connections.write().unwrap();
        let conn = conns
            .get_mut(id)
            .ok_or_else(|| FsError::NotFound(format!("connection {}", id)))?;

        if conn.state != WsState::Open && conn.state != WsState::Connecting {
            return Err(FsError::Io(format!(
                "cannot send on {} connection",
                conn.state.as_str()
            )));
        }

        conn.outbox.push_back(data);
        Ok(())
    }

    /// Get pending outgoing messages (called by transport)
    pub fn drain_outbox(&self, id: &str) -> Result<Vec<Vec<u8>>, FsError> {
        let mut conns = self.connections.write().unwrap();
        let conn = conns
            .get_mut(id)
            .ok_or_else(|| FsError::NotFound(format!("connection {}", id)))?;

        let messages: Vec<Vec<u8>> = conn.outbox.drain(..).collect();
        Ok(messages)
    }

    /// Add a received message to inbox (called by transport)
    pub fn receive_message(&self, id: &str, data: Vec<u8>) -> Result<(), FsError> {
        let mut conns = self.connections.write().unwrap();
        let conn = conns
            .get_mut(id)
            .ok_or_else(|| FsError::NotFound(format!("connection {}", id)))?;

        conn.inbox.push_back(data);
        Ok(())
    }

    /// Read next message from inbox (FIFO)
    pub fn read_message(&self, id: &str) -> Result<Option<Vec<u8>>, FsError> {
        let mut conns = self.connections.write().unwrap();
        let conn = conns
            .get_mut(id)
            .ok_or_else(|| FsError::NotFound(format!("connection {}", id)))?;

        Ok(conn.inbox.pop_front())
    }

    /// Peek at inbox without removing
    pub fn peek_inbox(&self, id: &str) -> Result<Vec<Vec<u8>>, FsError> {
        let conns = self.connections.read().unwrap();
        let conn = conns
            .get(id)
            .ok_or_else(|| FsError::NotFound(format!("connection {}", id)))?;

        Ok(conn.inbox.iter().cloned().collect())
    }

    /// Get connection info
    pub fn get_connection(&self, id: &str) -> Option<ConnectionInfo> {
        let conns = self.connections.read().unwrap();
        conns.get(id).map(|c| ConnectionInfo {
            id: c.id.clone(),
            url: c.url.clone(),
            state: c.state,
            error: c.error.clone(),
            outbox_count: c.outbox.len(),
            inbox_count: c.inbox.len(),
            opened_at: c.opened_at,
        })
    }

    /// List all connection IDs
    pub fn list_connections(&self) -> Vec<String> {
        let conns = self.connections.read().unwrap();
        conns.keys().cloned().collect()
    }

    /// Get count of active connections
    pub fn connection_count(&self) -> usize {
        let conns = self.connections.read().unwrap();
        conns.len()
    }

    /// List connections in Connecting state (need to be opened by executor)
    ///
    /// Returns (connection_id, url) pairs for connections waiting to be opened.
    pub fn connecting_connections(&self) -> Vec<(String, String)> {
        let conns = self.connections.read().unwrap();
        conns
            .values()
            .filter(|c| c.state == WsState::Connecting)
            .map(|c| (c.id.clone(), c.url.clone()))
            .collect()
    }

    /// List connections in Closing state (need to be closed by executor)
    ///
    /// Returns connection IDs for connections waiting to be closed.
    pub fn closing_connections(&self) -> Vec<String> {
        let conns = self.connections.read().unwrap();
        conns
            .values()
            .filter(|c| c.state == WsState::Closing)
            .map(|c| c.id.clone())
            .collect()
    }

    /// Peek at outbox without draining (for inspection)
    pub fn peek_outbox(&self, id: &str) -> Result<Vec<Vec<u8>>, FsError> {
        let conns = self.connections.read().unwrap();
        let conn = conns
            .get(id)
            .ok_or_else(|| FsError::NotFound(format!("connection {}", id)))?;
        Ok(conn.outbox.iter().cloned().collect())
    }
}

impl Default for WsFs {
    fn default() -> Self {
        Self::new()
    }
}

/// Connection info for status reporting
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConnectionInfo {
    pub id: String,
    pub url: String,
    pub state: WsState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub outbox_count: usize,
    pub inbox_count: usize,
    pub opened_at: u64,
}

impl FileService for WsFs {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError> {
        let path = path.trim_start_matches('/');
        let parts: Vec<&str> = path.split('/').collect();

        match parts.as_slice() {
            // Control file - write commands
            ["control"] => {
                if !flags.write {
                    return Err(FsError::PermissionDenied("control is write-only".into()));
                }
                Ok(Box::new(ControlHandle {
                    ws: WsFsRef {
                        connections: Arc::clone(&self.connections),
                        next_id: &self.next_id as *const AtomicU64,
                        max_connections: self.max_connections,
                    },
                    buffer: Vec::new(),
                }))
            }

            // Overall status
            ["status"] => {
                if flags.write {
                    return Err(FsError::ReadOnly);
                }
                let conns = self.connections.read().unwrap();
                let status = serde_json::json!({
                    "connection_count": conns.len(),
                    "max_connections": self.max_connections,
                    "connections": conns.keys().collect::<Vec<_>>()
                });
                let json = serde_json::to_string_pretty(&status)
                    .map_err(|e| FsError::Io(e.to_string()))?;
                Ok(Box::new(StaticHandle::new(json.into_bytes())))
            }

            // Connection URL
            ["conns", conn_id, "url"] => {
                if flags.write {
                    return Err(FsError::ReadOnly);
                }
                let conns = self.connections.read().unwrap();
                let conn = conns
                    .get(*conn_id)
                    .ok_or_else(|| FsError::NotFound(path.to_string()))?;
                Ok(Box::new(StaticHandle::new(conn.url.as_bytes().to_vec())))
            }

            // Connection status
            ["conns", conn_id, "status"] => {
                if flags.write {
                    return Err(FsError::ReadOnly);
                }
                let info = self
                    .get_connection(conn_id)
                    .ok_or_else(|| FsError::NotFound(path.to_string()))?;
                let json = serde_json::to_string_pretty(&info)
                    .map_err(|e| FsError::Io(e.to_string()))?;
                Ok(Box::new(StaticHandle::new(json.into_bytes())))
            }

            // Connection outbox (write)
            ["conns", conn_id, "out"] => {
                if !flags.write {
                    return Err(FsError::PermissionDenied("out is write-only".into()));
                }
                Ok(Box::new(OutHandle {
                    connections: Arc::clone(&self.connections),
                    conn_id: conn_id.to_string(),
                    buffer: Vec::new(),
                }))
            }

            // Connection inbox (read)
            ["conns", conn_id, "in"] => {
                if flags.write {
                    return Err(FsError::ReadOnly);
                }
                // Read next message from inbox
                let msg = self.read_message(conn_id)?;
                match msg {
                    Some(data) => Ok(Box::new(StaticHandle::new(data))),
                    None => Ok(Box::new(StaticHandle::new(Vec::new()))), // Empty if no messages
                }
            }

            _ => Err(FsError::NotFound(path.to_string())),
        }
    }

    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        let path = path.trim_start_matches('/');

        match path {
            "" | "." => Ok(vec![
                DirEntry {
                    name: "control".into(),
                    is_dir: false,
                    size: 0,
                },
                DirEntry {
                    name: "status".into(),
                    is_dir: false,
                    size: 0,
                },
                DirEntry {
                    name: "conns".into(),
                    is_dir: true,
                    size: 0,
                },
            ]),
            "conns" => {
                let conns = self.connections.read().unwrap();
                Ok(conns
                    .keys()
                    .map(|id| DirEntry {
                        name: id.clone(),
                        is_dir: true,
                        size: 0,
                    })
                    .collect())
            }
            p if p.starts_with("conns/") => {
                let conn_id = p.strip_prefix("conns/").unwrap();
                // Check if this is a valid connection
                let conns = self.connections.read().unwrap();
                if !conns.contains_key(conn_id) {
                    return Err(FsError::NotFound(path.to_string()));
                }
                Ok(vec![
                    DirEntry {
                        name: "in".into(),
                        is_dir: false,
                        size: 0,
                    },
                    DirEntry {
                        name: "out".into(),
                        is_dir: false,
                        size: 0,
                    },
                    DirEntry {
                        name: "status".into(),
                        is_dir: false,
                        size: 0,
                    },
                    DirEntry {
                        name: "url".into(),
                        is_dir: false,
                        size: 0,
                    },
                ])
            }
            _ => Err(FsError::NotADirectory(path.to_string())),
        }
    }

    fn stat(&self, path: &str) -> Result<Metadata, FsError> {
        let path = path.trim_start_matches('/');

        let is_dir = match path {
            "" | "." | "conns" => true,
            p if p.starts_with("conns/") && !p.contains('/') => {
                // Check if connection exists
                let conn_id = p.strip_prefix("conns/").unwrap();
                let conns = self.connections.read().unwrap();
                if !conns.contains_key(conn_id) {
                    return Err(FsError::NotFound(path.to_string()));
                }
                true
            }
            "control" | "status" => false,
            p if p.starts_with("conns/") => {
                // conns/{id}/{file}
                let parts: Vec<&str> = p.split('/').collect();
                if parts.len() == 3 {
                    let conn_id = parts[1];
                    let conns = self.connections.read().unwrap();
                    if !conns.contains_key(conn_id) {
                        return Err(FsError::NotFound(path.to_string()));
                    }
                    false
                } else {
                    return Err(FsError::NotFound(path.to_string()));
                }
            }
            _ => return Err(FsError::NotFound(path.to_string())),
        };

        Ok(Metadata {
            is_dir,
            size: 0,
            modified: now(),
            readonly: path != "control" && !path.ends_with("/out"),
        })
    }

    fn mkdir(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied(
            "use /control to create connections".into(),
        ))
    }

    fn remove(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied(
            "use /control to close connections".into(),
        ))
    }

    fn rename(&self, _from: &str, _to: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied("connections cannot be renamed".into()))
    }
}

/// Reference to WsFs for handles
struct WsFsRef {
    connections: Arc<RwLock<HashMap<String, WsConnection>>>,
    next_id: *const AtomicU64,
    max_connections: usize,
}

// Safety: WsFsRef is only used within WsFs which manages the lifetime
unsafe impl Send for WsFsRef {}
unsafe impl Sync for WsFsRef {}

/// Control file handle for open/close commands
struct ControlHandle {
    ws: WsFsRef,
    buffer: Vec<u8>,
}

impl FileHandle for ControlHandle {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied("control is write-only".into()))
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, FsError> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: u64) -> Result<(), FsError> {
        Ok(())
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> Result<(), FsError> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let json_str = String::from_utf8(self.buffer.clone())
            .map_err(|e| FsError::Io(format!("invalid UTF-8: {}", e)))?;

        let cmd: ControlCommand = serde_json::from_str(&json_str)
            .map_err(|e| FsError::Io(format!("invalid command: {}", e)))?;

        match cmd {
            ControlCommand::Open { url } => {
                let mut conns = self.ws.connections.write().unwrap();
                if conns.len() >= self.ws.max_connections {
                    return Err(FsError::Io("max connections reached".into()));
                }

                // Safety: we're only reading the atomic
                let next_id = unsafe { &*self.ws.next_id };
                let id = format!("conn-{}", next_id.fetch_add(1, Ordering::SeqCst));
                let conn = WsConnection::new(id.clone(), url);
                conns.insert(id, conn);
            }
            ControlCommand::Close { id } => {
                let mut conns = self.ws.connections.write().unwrap();
                let conn = conns
                    .get_mut(&id)
                    .ok_or_else(|| FsError::NotFound(format!("connection {}", id)))?;
                conn.state = WsState::Closing;
            }
        }

        self.buffer.clear();
        Ok(())
    }
}

/// Control commands
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum ControlCommand {
    Open { url: String },
    Close { id: String },
}

/// Output handle for writing to connection outbox
struct OutHandle {
    connections: Arc<RwLock<HashMap<String, WsConnection>>>,
    conn_id: String,
    buffer: Vec<u8>,
}

impl FileHandle for OutHandle {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied("out is write-only".into()))
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, FsError> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: u64) -> Result<(), FsError> {
        Ok(())
    }

    fn position(&self) -> u64 {
        self.buffer.len() as u64
    }

    fn flush(&mut self) -> Result<(), FsError> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let mut conns = self.connections.write().unwrap();
        let conn = conns
            .get_mut(&self.conn_id)
            .ok_or_else(|| FsError::NotFound(format!("connection {}", self.conn_id)))?;

        if conn.state != WsState::Open && conn.state != WsState::Connecting {
            return Err(FsError::Io(format!(
                "cannot send on {} connection",
                conn.state.as_str()
            )));
        }

        conn.outbox.push_back(self.buffer.clone());
        self.buffer.clear();
        Ok(())
    }
}

/// Static read-only file handle
struct StaticHandle {
    data: Vec<u8>,
    position: usize,
}

impl StaticHandle {
    fn new(data: Vec<u8>) -> Self {
        Self { data, position: 0 }
    }
}

impl FileHandle for StaticHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, FsError> {
        let remaining = &self.data[self.position..];
        let to_read = std::cmp::min(buf.len(), remaining.len());
        buf[..to_read].copy_from_slice(&remaining[..to_read]);
        self.position += to_read;
        Ok(to_read)
    }

    fn write(&mut self, _buf: &[u8]) -> Result<usize, FsError> {
        Err(FsError::ReadOnly)
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

/// Get current Unix timestamp
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_file(fs: &dyn FileService, path: &str) -> String {
        let mut handle = fs.open(path, OpenFlags::read_only()).unwrap();
        let mut buf = vec![0u8; 4096];
        let n = handle.read(&mut buf).unwrap();
        String::from_utf8_lossy(&buf[..n]).to_string()
    }

    fn write_file(fs: &dyn FileService, path: &str, content: &str) {
        let mut handle = fs.open(path, OpenFlags::write_only()).unwrap();
        handle.write(content.as_bytes()).unwrap();
        handle.flush().unwrap();
    }

    #[test]
    fn test_ws_fs_creation() {
        let ws = WsFs::new();
        assert_eq!(ws.connection_count(), 0);
    }

    #[test]
    fn test_open_connection_programmatic() {
        let ws = WsFs::new();

        let id = ws.open_connection("wss://relay.example.com").unwrap();
        assert!(id.starts_with("conn-"));

        let info = ws.get_connection(&id).unwrap();
        assert_eq!(info.url, "wss://relay.example.com");
        assert_eq!(info.state, WsState::Connecting);
    }

    #[test]
    fn test_open_connection_via_file() {
        let ws = WsFs::new();

        // Open connection via control file
        write_file(&ws, "/control", r#"{"url": "wss://relay.example.com"}"#);

        assert_eq!(ws.connection_count(), 1);
        let ids = ws.list_connections();
        assert_eq!(ids.len(), 1);

        let info = ws.get_connection(&ids[0]).unwrap();
        assert_eq!(info.url, "wss://relay.example.com");
    }

    #[test]
    fn test_close_connection_via_file() {
        let ws = WsFs::new();

        let id = ws.open_connection("wss://example.com").unwrap();
        ws.set_connected(&id).unwrap();

        // Close via control file
        let close_cmd = format!(r#"{{"id": "{}"}}"#, id);
        write_file(&ws, "/control", &close_cmd);

        let info = ws.get_connection(&id).unwrap();
        assert_eq!(info.state, WsState::Closing);
    }

    #[test]
    fn test_connection_lifecycle() {
        let ws = WsFs::new();

        // Open
        let id = ws.open_connection("wss://example.com").unwrap();
        assert_eq!(ws.get_connection(&id).unwrap().state, WsState::Connecting);

        // Connected
        ws.set_connected(&id).unwrap();
        assert_eq!(ws.get_connection(&id).unwrap().state, WsState::Open);

        // Closing
        ws.close_connection(&id).unwrap();
        assert_eq!(ws.get_connection(&id).unwrap().state, WsState::Closing);

        // Closed
        ws.set_closed(&id).unwrap();
        assert_eq!(ws.get_connection(&id).unwrap().state, WsState::Closed);

        // Remove
        ws.remove_connection(&id);
        assert!(ws.get_connection(&id).is_none());
    }

    #[test]
    fn test_connection_error() {
        let ws = WsFs::new();

        let id = ws.open_connection("wss://example.com").unwrap();
        ws.set_error(&id, "Connection refused").unwrap();

        let info = ws.get_connection(&id).unwrap();
        assert_eq!(info.state, WsState::Error);
        assert_eq!(info.error, Some("Connection refused".to_string()));
    }

    #[test]
    fn test_send_receive_messages() {
        let ws = WsFs::new();

        let id = ws.open_connection("wss://example.com").unwrap();
        ws.set_connected(&id).unwrap();

        // Send message
        ws.send_message(&id, b"Hello, WebSocket!".to_vec()).unwrap();

        // Check outbox
        let messages = ws.drain_outbox(&id).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0], b"Hello, WebSocket!");

        // Outbox should be empty now
        let messages = ws.drain_outbox(&id).unwrap();
        assert!(messages.is_empty());

        // Receive message
        ws.receive_message(&id, b"Hello back!".to_vec()).unwrap();

        // Read from inbox
        let msg = ws.read_message(&id).unwrap();
        assert_eq!(msg, Some(b"Hello back!".to_vec()));

        // Inbox should be empty now
        let msg = ws.read_message(&id).unwrap();
        assert!(msg.is_none());
    }

    #[test]
    fn test_send_via_file() {
        let ws = WsFs::new();

        let id = ws.open_connection("wss://example.com").unwrap();
        ws.set_connected(&id).unwrap();

        // Write to /conns/{id}/out
        let path = format!("/conns/{}/out", id);
        write_file(&ws, &path, "Hello via file!");

        // Check outbox
        let messages = ws.drain_outbox(&id).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(String::from_utf8_lossy(&messages[0]), "Hello via file!");
    }

    #[test]
    fn test_receive_via_file() {
        let ws = WsFs::new();

        let id = ws.open_connection("wss://example.com").unwrap();
        ws.set_connected(&id).unwrap();

        // Add message to inbox (simulating transport)
        ws.receive_message(&id, b"Incoming message".to_vec()).unwrap();

        // Read via file
        let path = format!("/conns/{}/in", id);
        let msg = read_file(&ws, &path);
        assert_eq!(msg, "Incoming message");

        // Second read should be empty (message consumed)
        let msg = read_file(&ws, &path);
        assert!(msg.is_empty());
    }

    #[test]
    fn test_read_connection_status() {
        let ws = WsFs::new();

        let id = ws.open_connection("wss://relay.example.com").unwrap();
        ws.set_connected(&id).unwrap();

        let path = format!("/conns/{}/status", id);
        let status = read_file(&ws, &path);

        assert!(status.contains("\"state\": \"open\""));
        assert!(status.contains("relay.example.com"));
    }

    #[test]
    fn test_read_connection_url() {
        let ws = WsFs::new();

        let id = ws.open_connection("wss://my.relay.com").unwrap();

        let path = format!("/conns/{}/url", id);
        let url = read_file(&ws, &path);

        assert_eq!(url, "wss://my.relay.com");
    }

    #[test]
    fn test_readdir_root() {
        let ws = WsFs::new();
        let entries = ws.readdir("/").unwrap();

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"control"));
        assert!(names.contains(&"status"));
        assert!(names.contains(&"conns"));
    }

    #[test]
    fn test_readdir_conns() {
        let ws = WsFs::new();

        ws.open_connection("wss://a.com").unwrap();
        ws.open_connection("wss://b.com").unwrap();

        let entries = ws.readdir("/conns").unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_readdir_connection() {
        let ws = WsFs::new();

        let id = ws.open_connection("wss://example.com").unwrap();

        let path = format!("/conns/{}", id);
        let entries = ws.readdir(&path).unwrap();

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"in"));
        assert!(names.contains(&"out"));
        assert!(names.contains(&"status"));
        assert!(names.contains(&"url"));
    }

    #[test]
    fn test_read_overall_status() {
        let ws = WsFs::new();

        ws.open_connection("wss://a.com").unwrap();
        ws.open_connection("wss://b.com").unwrap();

        let status = read_file(&ws, "/status");
        assert!(status.contains("\"connection_count\": 2"));
    }

    #[test]
    fn test_max_connections() {
        let ws = WsFs::with_max_connections(2);

        ws.open_connection("wss://a.com").unwrap();
        ws.open_connection("wss://b.com").unwrap();

        // Third connection should fail
        let result = ws.open_connection("wss://c.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_cannot_send_on_closed() {
        let ws = WsFs::new();

        let id = ws.open_connection("wss://example.com").unwrap();
        ws.set_connected(&id).unwrap();
        ws.set_closed(&id).unwrap();

        let result = ws.send_message(&id, b"test".to_vec());
        assert!(result.is_err());
    }

    #[test]
    fn test_fifo_message_order() {
        let ws = WsFs::new();

        let id = ws.open_connection("wss://example.com").unwrap();
        ws.set_connected(&id).unwrap();

        // Receive multiple messages
        ws.receive_message(&id, b"first".to_vec()).unwrap();
        ws.receive_message(&id, b"second".to_vec()).unwrap();
        ws.receive_message(&id, b"third".to_vec()).unwrap();

        // Should read in FIFO order
        assert_eq!(ws.read_message(&id).unwrap(), Some(b"first".to_vec()));
        assert_eq!(ws.read_message(&id).unwrap(), Some(b"second".to_vec()));
        assert_eq!(ws.read_message(&id).unwrap(), Some(b"third".to_vec()));
        assert_eq!(ws.read_message(&id).unwrap(), None);
    }
}
