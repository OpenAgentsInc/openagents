//! HTTP client capability service for agent environments
//!
//! Provides HTTP request/response capabilities as a filesystem interface.
//! Uses a request queue pattern where actual HTTP I/O is handled externally.
//!
//! # File Layout
//!
//! ```text
//! /cap/http/
//! ├── request           # Write request JSON → queued for execution
//! ├── pending/          # Pending requests
//! │   └── {id}.json     # Request details
//! ├── responses/        # Completed responses
//! │   └── {id}.json     # Response with status, headers, body
//! └── status            # Service status
//! ```
//!
//! # Design
//!
//! HttpFs uses a queue-based design:
//! - Agent writes request JSON to `/request`
//! - Request is queued in `/pending/`
//! - External HTTP executor processes pending requests
//! - Responses appear in `/responses/`
//!
//! # Example
//!
//! ```rust,ignore
//! use oanix::services::HttpFs;
//!
//! let http = HttpFs::new();
//!
//! // Submit a request
//! write_file(&http, "/request", r#"{
//!     "method": "GET",
//!     "url": "https://api.example.com/data",
//!     "headers": {"Authorization": "Bearer token"}
//! }"#);
//!
//! // External executor processes and adds response
//! // http.complete_request(id, response);
//!
//! // Agent reads response
//! let response = read_file(&http, "/responses/req-0.json");
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::error::FsError;
use crate::service::{DirEntry, FileHandle, FileService, Metadata, OpenFlags};

/// HTTP request method
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
        }
    }
}

impl Default for HttpMethod {
    fn default() -> Self {
        HttpMethod::Get
    }
}

/// HTTP request
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HttpRequest {
    /// Request ID (assigned by HttpFs)
    #[serde(default)]
    pub id: String,
    /// HTTP method
    #[serde(default)]
    pub method: HttpMethod,
    /// Request URL
    pub url: String,
    /// Request headers
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Request body (for POST, PUT, PATCH)
    #[serde(default)]
    pub body: Option<String>,
    /// Timeout in seconds
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    /// Timestamp when request was created
    #[serde(default)]
    pub created_at: u64,
}

/// HTTP response
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HttpResponse {
    /// Request ID this response is for
    pub request_id: String,
    /// HTTP status code
    pub status: u16,
    /// Status text
    pub status_text: String,
    /// Response headers
    pub headers: HashMap<String, String>,
    /// Response body
    pub body: String,
    /// Time taken in milliseconds
    pub duration_ms: u64,
    /// Timestamp when response was received
    pub completed_at: u64,
}

/// Request state
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RequestState {
    Pending,
    InProgress,
    Completed,
    Failed,
}

/// HTTP client capability service
pub struct HttpFs {
    /// Pending requests (waiting to be executed)
    pending: Arc<RwLock<HashMap<String, HttpRequest>>>,
    /// Completed responses
    responses: Arc<RwLock<HashMap<String, HttpResponse>>>,
    /// Failed requests with error messages
    failures: Arc<RwLock<HashMap<String, String>>>,
    /// Request ID counter
    next_id: AtomicU64,
    /// Default timeout in seconds
    default_timeout: u64,
}

impl HttpFs {
    /// Create a new HttpFs with default settings
    pub fn new() -> Self {
        Self {
            pending: Arc::new(RwLock::new(HashMap::new())),
            responses: Arc::new(RwLock::new(HashMap::new())),
            failures: Arc::new(RwLock::new(HashMap::new())),
            next_id: AtomicU64::new(0),
            default_timeout: 30,
        }
    }

    /// Create with custom default timeout
    pub fn with_timeout(timeout_secs: u64) -> Self {
        Self {
            pending: Arc::new(RwLock::new(HashMap::new())),
            responses: Arc::new(RwLock::new(HashMap::new())),
            failures: Arc::new(RwLock::new(HashMap::new())),
            next_id: AtomicU64::new(0),
            default_timeout: timeout_secs,
        }
    }

    /// Submit a new HTTP request
    ///
    /// Returns the request ID. Use `get_response()` or read from
    /// `/responses/{id}.json` to get the result.
    pub fn submit_request(&self, mut request: HttpRequest) -> String {
        let id = format!("req-{}", self.next_id.fetch_add(1, Ordering::SeqCst));
        request.id = id.clone();
        request.created_at = now();
        if request.timeout_secs.is_none() {
            request.timeout_secs = Some(self.default_timeout);
        }

        let mut pending = self.pending.write().unwrap();
        pending.insert(id.clone(), request);
        id
    }

    /// Get a pending request (for executor to process)
    pub fn get_pending(&self, id: &str) -> Option<HttpRequest> {
        let pending = self.pending.read().unwrap();
        pending.get(id).cloned()
    }

    /// List all pending request IDs
    pub fn list_pending(&self) -> Vec<String> {
        let pending = self.pending.read().unwrap();
        pending.keys().cloned().collect()
    }

    /// Take a pending request (removes from pending)
    pub fn take_pending(&self, id: &str) -> Option<HttpRequest> {
        let mut pending = self.pending.write().unwrap();
        pending.remove(id)
    }

    /// Complete a request with a response (called by executor)
    pub fn complete_request(&self, response: HttpResponse) {
        let request_id = response.request_id.clone();

        // Remove from pending
        {
            let mut pending = self.pending.write().unwrap();
            pending.remove(&request_id);
        }

        // Add to responses
        {
            let mut responses = self.responses.write().unwrap();
            responses.insert(request_id, response);
        }
    }

    /// Mark a request as failed (called by executor)
    pub fn fail_request(&self, id: &str, error: impl Into<String>) {
        // Remove from pending
        {
            let mut pending = self.pending.write().unwrap();
            pending.remove(id);
        }

        // Add to failures
        {
            let mut failures = self.failures.write().unwrap();
            failures.insert(id.to_string(), error.into());
        }
    }

    /// Get a response for a request
    pub fn get_response(&self, id: &str) -> Option<HttpResponse> {
        let responses = self.responses.read().unwrap();
        responses.get(id).cloned()
    }

    /// Get failure message for a request
    pub fn get_failure(&self, id: &str) -> Option<String> {
        let failures = self.failures.read().unwrap();
        failures.get(id).cloned()
    }

    /// Get request state
    pub fn get_state(&self, id: &str) -> Option<RequestState> {
        if self.pending.read().unwrap().contains_key(id) {
            return Some(RequestState::Pending);
        }
        if self.responses.read().unwrap().contains_key(id) {
            return Some(RequestState::Completed);
        }
        if self.failures.read().unwrap().contains_key(id) {
            return Some(RequestState::Failed);
        }
        None
    }

    /// List all response IDs
    pub fn list_responses(&self) -> Vec<String> {
        let responses = self.responses.read().unwrap();
        responses.keys().cloned().collect()
    }

    /// Clear a response (after agent has read it)
    pub fn clear_response(&self, id: &str) {
        let mut responses = self.responses.write().unwrap();
        responses.remove(id);
    }

    /// Clear all completed responses
    pub fn clear_all_responses(&self) {
        let mut responses = self.responses.write().unwrap();
        responses.clear();
    }

    /// Get counts for status
    pub fn counts(&self) -> (usize, usize, usize) {
        (
            self.pending.read().unwrap().len(),
            self.responses.read().unwrap().len(),
            self.failures.read().unwrap().len(),
        )
    }

    /// Check if there are any pending requests
    ///
    /// This is more efficient than `list_pending().is_empty()` as it doesn't
    /// allocate a vector.
    pub fn has_pending(&self) -> bool {
        !self.pending.read().unwrap().is_empty()
    }

    /// Take a batch of pending requests (for executor efficiency)
    ///
    /// Takes up to `limit` pending requests atomically. Returns the requests
    /// that were removed from the pending queue.
    pub fn take_pending_batch(&self, limit: usize) -> Vec<HttpRequest> {
        let mut pending = self.pending.write().unwrap();
        let ids: Vec<String> = pending.keys().take(limit).cloned().collect();
        ids.into_iter()
            .filter_map(|id| pending.remove(&id))
            .collect()
    }
}

impl Default for HttpFs {
    fn default() -> Self {
        Self::new()
    }
}

impl FileService for HttpFs {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError> {
        let path = path.trim_start_matches('/');
        let parts: Vec<&str> = path.split('/').collect();

        match parts.as_slice() {
            // Submit request (write-only)
            ["request"] => {
                if !flags.write {
                    return Err(FsError::PermissionDenied("request is write-only".into()));
                }
                Ok(Box::new(RequestHandle {
                    pending: Arc::clone(&self.pending),
                    next_id: &self.next_id as *const AtomicU64,
                    default_timeout: self.default_timeout,
                    buffer: Vec::new(),
                }))
            }

            // Service status
            ["status"] => {
                if flags.write {
                    return Err(FsError::ReadOnly);
                }
                let (pending, completed, failed) = self.counts();
                let status = serde_json::json!({
                    "pending_count": pending,
                    "completed_count": completed,
                    "failed_count": failed,
                    "default_timeout_secs": self.default_timeout
                });
                let json = serde_json::to_string_pretty(&status)
                    .map_err(|e| FsError::Io(e.to_string()))?;
                Ok(Box::new(StaticHandle::new(json.into_bytes())))
            }

            // Pending request details
            ["pending", filename] => {
                if flags.write {
                    return Err(FsError::ReadOnly);
                }
                let id = filename.trim_end_matches(".json");
                let pending = self.pending.read().unwrap();
                let request = pending
                    .get(id)
                    .ok_or_else(|| FsError::NotFound(path.to_string()))?;
                let json = serde_json::to_string_pretty(request)
                    .map_err(|e| FsError::Io(e.to_string()))?;
                Ok(Box::new(StaticHandle::new(json.into_bytes())))
            }

            // Response details
            ["responses", filename] => {
                if flags.write {
                    return Err(FsError::ReadOnly);
                }
                let id = filename.trim_end_matches(".json");

                // Check for response
                {
                    let responses = self.responses.read().unwrap();
                    if let Some(response) = responses.get(id) {
                        let json = serde_json::to_string_pretty(response)
                            .map_err(|e| FsError::Io(e.to_string()))?;
                        return Ok(Box::new(StaticHandle::new(json.into_bytes())));
                    }
                }

                // Check for failure
                {
                    let failures = self.failures.read().unwrap();
                    if let Some(error) = failures.get(id) {
                        let json = serde_json::json!({
                            "request_id": id,
                            "error": error,
                            "status": "failed"
                        });
                        let json_str = serde_json::to_string_pretty(&json)
                            .map_err(|e| FsError::Io(e.to_string()))?;
                        return Ok(Box::new(StaticHandle::new(json_str.into_bytes())));
                    }
                }

                Err(FsError::NotFound(path.to_string()))
            }

            _ => Err(FsError::NotFound(path.to_string())),
        }
    }

    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        let path = path.trim_start_matches('/');

        match path {
            "" | "." => Ok(vec![
                DirEntry {
                    name: "request".into(),
                    is_dir: false,
                    size: 0,
                },
                DirEntry {
                    name: "pending".into(),
                    is_dir: true,
                    size: 0,
                },
                DirEntry {
                    name: "responses".into(),
                    is_dir: true,
                    size: 0,
                },
                DirEntry {
                    name: "status".into(),
                    is_dir: false,
                    size: 0,
                },
            ]),
            "pending" => {
                let pending = self.pending.read().unwrap();
                Ok(pending
                    .keys()
                    .map(|id| DirEntry {
                        name: format!("{}.json", id),
                        is_dir: false,
                        size: 0,
                    })
                    .collect())
            }
            "responses" => {
                let responses = self.responses.read().unwrap();
                let failures = self.failures.read().unwrap();

                let mut entries: Vec<DirEntry> = responses
                    .keys()
                    .map(|id| DirEntry {
                        name: format!("{}.json", id),
                        is_dir: false,
                        size: 0,
                    })
                    .collect();

                // Also include failures in responses directory
                for id in failures.keys() {
                    entries.push(DirEntry {
                        name: format!("{}.json", id),
                        is_dir: false,
                        size: 0,
                    });
                }

                Ok(entries)
            }
            _ => Err(FsError::NotADirectory(path.to_string())),
        }
    }

    fn stat(&self, path: &str) -> Result<Metadata, FsError> {
        let path = path.trim_start_matches('/');

        let is_dir = match path {
            "" | "." | "pending" | "responses" => true,
            "request" | "status" => false,
            p if p.starts_with("pending/") || p.starts_with("responses/") => false,
            _ => return Err(FsError::NotFound(path.to_string())),
        };

        Ok(Metadata {
            is_dir,
            size: 0,
            modified: now(),
            readonly: path != "request",
        })
    }

    fn mkdir(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied(
            "directories are managed automatically".into(),
        ))
    }

    fn remove(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied(
            "use clear_response() to remove responses".into(),
        ))
    }

    fn rename(&self, _from: &str, _to: &str) -> Result<(), FsError> {
        Err(FsError::PermissionDenied(
            "requests cannot be renamed".into(),
        ))
    }
}

/// Request submission handle
struct RequestHandle {
    pending: Arc<RwLock<HashMap<String, HttpRequest>>>,
    next_id: *const AtomicU64,
    default_timeout: u64,
    buffer: Vec<u8>,
}

// Safety: RequestHandle only used within HttpFs
// The raw pointer is only used to read an AtomicU64 which is thread-safe
unsafe impl Send for RequestHandle {}
unsafe impl Sync for RequestHandle {}

impl FileHandle for RequestHandle {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied("request is write-only".into()))
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

        let mut request: HttpRequest = serde_json::from_str(&json_str)
            .map_err(|e| FsError::Io(format!("invalid request: {}", e)))?;

        // Assign ID and timestamp
        // Safety: we're only reading the atomic
        let next_id = unsafe { &*self.next_id };
        let id = format!("req-{}", next_id.fetch_add(1, Ordering::SeqCst));
        request.id = id.clone();
        request.created_at = now();
        if request.timeout_secs.is_none() {
            request.timeout_secs = Some(self.default_timeout);
        }

        // Add to pending
        {
            let mut pending = self.pending.write().unwrap();
            pending.insert(id, request);
        }

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
        let mut buf = vec![0u8; 8192];
        let n = handle.read(&mut buf).unwrap();
        String::from_utf8_lossy(&buf[..n]).to_string()
    }

    fn write_file(fs: &dyn FileService, path: &str, content: &str) {
        let mut handle = fs.open(path, OpenFlags::write_only()).unwrap();
        handle.write(content.as_bytes()).unwrap();
        handle.flush().unwrap();
    }

    #[test]
    fn test_http_fs_creation() {
        let http = HttpFs::new();
        let (pending, completed, failed) = http.counts();
        assert_eq!(pending, 0);
        assert_eq!(completed, 0);
        assert_eq!(failed, 0);
    }

    #[test]
    fn test_submit_request_programmatic() {
        let http = HttpFs::new();

        let request = HttpRequest {
            id: String::new(),
            method: HttpMethod::Get,
            url: "https://api.example.com/data".to_string(),
            headers: HashMap::new(),
            body: None,
            timeout_secs: None,
            created_at: 0,
        };

        let id = http.submit_request(request);
        assert!(id.starts_with("req-"));

        let pending = http.get_pending(&id).unwrap();
        assert_eq!(pending.url, "https://api.example.com/data");
        assert_eq!(pending.method, HttpMethod::Get);
    }

    #[test]
    fn test_submit_request_via_file() {
        let http = HttpFs::new();

        let request_json = r#"{
            "method": "POST",
            "url": "https://api.example.com/submit",
            "headers": {"Content-Type": "application/json"},
            "body": "{\"data\": 42}"
        }"#;

        write_file(&http, "/request", request_json);

        let ids = http.list_pending();
        assert_eq!(ids.len(), 1);

        let pending = http.get_pending(&ids[0]).unwrap();
        assert_eq!(pending.method, HttpMethod::Post);
        assert_eq!(pending.url, "https://api.example.com/submit");
        assert!(pending.headers.contains_key("Content-Type"));
    }

    #[test]
    fn test_complete_request() {
        let http = HttpFs::new();

        let request = HttpRequest {
            id: String::new(),
            method: HttpMethod::Get,
            url: "https://api.example.com".to_string(),
            headers: HashMap::new(),
            body: None,
            timeout_secs: None,
            created_at: 0,
        };

        let id = http.submit_request(request);

        // Simulate executor completing the request
        let response = HttpResponse {
            request_id: id.clone(),
            status: 200,
            status_text: "OK".to_string(),
            headers: HashMap::new(),
            body: r#"{"result": "success"}"#.to_string(),
            duration_ms: 150,
            completed_at: now(),
        };

        http.complete_request(response);

        // Should no longer be pending
        assert!(http.get_pending(&id).is_none());

        // Should have response
        let resp = http.get_response(&id).unwrap();
        assert_eq!(resp.status, 200);
        assert!(resp.body.contains("success"));
    }

    #[test]
    fn test_fail_request() {
        let http = HttpFs::new();

        let request = HttpRequest {
            id: String::new(),
            method: HttpMethod::Get,
            url: "https://invalid.example.com".to_string(),
            headers: HashMap::new(),
            body: None,
            timeout_secs: None,
            created_at: 0,
        };

        let id = http.submit_request(request);
        http.fail_request(&id, "Connection refused");

        assert!(http.get_pending(&id).is_none());
        assert_eq!(http.get_state(&id), Some(RequestState::Failed));
        assert_eq!(
            http.get_failure(&id),
            Some("Connection refused".to_string())
        );
    }

    #[test]
    fn test_read_response_via_file() {
        let http = HttpFs::new();

        let id = http.submit_request(HttpRequest {
            id: String::new(),
            method: HttpMethod::Get,
            url: "https://api.example.com".to_string(),
            headers: HashMap::new(),
            body: None,
            timeout_secs: None,
            created_at: 0,
        });

        http.complete_request(HttpResponse {
            request_id: id.clone(),
            status: 200,
            status_text: "OK".to_string(),
            headers: HashMap::new(),
            body: "Hello, World!".to_string(),
            duration_ms: 100,
            completed_at: now(),
        });

        let path = format!("/responses/{}.json", id);
        let response = read_file(&http, &path);

        assert!(response.contains("\"status\": 200"));
        assert!(response.contains("Hello, World!"));
    }

    #[test]
    fn test_read_failure_via_file() {
        let http = HttpFs::new();

        let id = http.submit_request(HttpRequest {
            id: String::new(),
            method: HttpMethod::Get,
            url: "https://api.example.com".to_string(),
            headers: HashMap::new(),
            body: None,
            timeout_secs: None,
            created_at: 0,
        });

        http.fail_request(&id, "Timeout");

        let path = format!("/responses/{}.json", id);
        let response = read_file(&http, &path);

        assert!(response.contains("\"error\": \"Timeout\""));
        assert!(response.contains("\"status\": \"failed\""));
    }

    #[test]
    fn test_readdir_root() {
        let http = HttpFs::new();
        let entries = http.readdir("/").unwrap();

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"request"));
        assert!(names.contains(&"pending"));
        assert!(names.contains(&"responses"));
        assert!(names.contains(&"status"));
    }

    #[test]
    fn test_readdir_pending() {
        let http = HttpFs::new();

        http.submit_request(HttpRequest {
            id: String::new(),
            method: HttpMethod::Get,
            url: "https://a.com".to_string(),
            headers: HashMap::new(),
            body: None,
            timeout_secs: None,
            created_at: 0,
        });

        http.submit_request(HttpRequest {
            id: String::new(),
            method: HttpMethod::Get,
            url: "https://b.com".to_string(),
            headers: HashMap::new(),
            body: None,
            timeout_secs: None,
            created_at: 0,
        });

        let entries = http.readdir("/pending").unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_read_status() {
        let http = HttpFs::new();

        http.submit_request(HttpRequest {
            id: String::new(),
            method: HttpMethod::Get,
            url: "https://a.com".to_string(),
            headers: HashMap::new(),
            body: None,
            timeout_secs: None,
            created_at: 0,
        });

        let status = read_file(&http, "/status");
        assert!(status.contains("\"pending_count\": 1"));
    }

    #[test]
    fn test_request_state_transitions() {
        let http = HttpFs::new();

        let id = http.submit_request(HttpRequest {
            id: String::new(),
            method: HttpMethod::Get,
            url: "https://api.example.com".to_string(),
            headers: HashMap::new(),
            body: None,
            timeout_secs: None,
            created_at: 0,
        });

        // Initially pending
        assert_eq!(http.get_state(&id), Some(RequestState::Pending));

        // Complete it
        http.complete_request(HttpResponse {
            request_id: id.clone(),
            status: 200,
            status_text: "OK".to_string(),
            headers: HashMap::new(),
            body: "done".to_string(),
            duration_ms: 50,
            completed_at: now(),
        });

        // Now completed
        assert_eq!(http.get_state(&id), Some(RequestState::Completed));
    }

    #[test]
    fn test_take_pending() {
        let http = HttpFs::new();

        let id = http.submit_request(HttpRequest {
            id: String::new(),
            method: HttpMethod::Get,
            url: "https://api.example.com".to_string(),
            headers: HashMap::new(),
            body: None,
            timeout_secs: None,
            created_at: 0,
        });

        // Take removes from pending
        let request = http.take_pending(&id).unwrap();
        assert_eq!(request.url, "https://api.example.com");

        // No longer pending
        assert!(http.get_pending(&id).is_none());
        assert!(http.get_state(&id).is_none());
    }

    #[test]
    fn test_default_timeout() {
        let http = HttpFs::with_timeout(60);

        let id = http.submit_request(HttpRequest {
            id: String::new(),
            method: HttpMethod::Get,
            url: "https://api.example.com".to_string(),
            headers: HashMap::new(),
            body: None,
            timeout_secs: None, // Should get default
            created_at: 0,
        });

        let request = http.get_pending(&id).unwrap();
        assert_eq!(request.timeout_secs, Some(60));
    }

    #[test]
    fn test_clear_response() {
        let http = HttpFs::new();

        let id = http.submit_request(HttpRequest {
            id: String::new(),
            method: HttpMethod::Get,
            url: "https://api.example.com".to_string(),
            headers: HashMap::new(),
            body: None,
            timeout_secs: None,
            created_at: 0,
        });

        http.complete_request(HttpResponse {
            request_id: id.clone(),
            status: 200,
            status_text: "OK".to_string(),
            headers: HashMap::new(),
            body: "done".to_string(),
            duration_ms: 50,
            completed_at: now(),
        });

        assert!(http.get_response(&id).is_some());

        http.clear_response(&id);

        assert!(http.get_response(&id).is_none());
    }

    #[test]
    fn test_http_methods() {
        for (method_str, method) in [
            ("GET", HttpMethod::Get),
            ("POST", HttpMethod::Post),
            ("PUT", HttpMethod::Put),
            ("PATCH", HttpMethod::Patch),
            ("DELETE", HttpMethod::Delete),
        ] {
            // Create fresh HttpFs for each method to avoid HashMap ordering issues
            let http = HttpFs::new();
            let request_json = format!(
                r#"{{"method": "{}", "url": "https://example.com"}}"#,
                method_str
            );
            write_file(&http, "/request", &request_json);

            let ids = http.list_pending();
            assert_eq!(ids.len(), 1);
            let request = http.get_pending(&ids[0]).unwrap();
            assert_eq!(
                request.method, method,
                "Method {} should parse correctly",
                method_str
            );
        }
    }
}
