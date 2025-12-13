//! Nostr capability service for agent environments
//!
//! Provides Nostr event signing and NIP-90 DVM capabilities as a filesystem interface.
//!
//! # File Layout
//!
//! ```text
//! /cap/nostr/
//! ├── identity/
//! │   ├── pubkey       # Public key hex (read-only)
//! │   └── npub         # Bech32 npub (read-only)
//! ├── outbox/          # Events queued for sending
//! │   └── {id}.json    # Individual signed events
//! ├── inbox/           # Received events
//! │   └── {id}.json    # Individual events
//! ├── submit           # Write event template → signed event in outbox
//! ├── request          # Write NIP-90 job request → signed event in outbox
//! └── status           # Service status
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use oanix::services::NostrFs;
//! use oanix::service::{FileService, OpenFlags};
//!
//! // Create with a secret key
//! let secret_key = [0u8; 32]; // Use a real key!
//! let nostr = NostrFs::new(secret_key);
//!
//! // Read identity
//! let pubkey = read_file(&nostr, "/identity/pubkey");
//!
//! // Submit a NIP-90 job request
//! let request = r#"{"kind":5050,"input":"What is 2+2?","params":{}}"#;
//! write_file(&nostr, "/request", request.as_bytes());
//!
//! // Read the queued event from outbox
//! let events = nostr.readdir("/outbox");
//! ```

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::error::FsError;
use crate::service::{DirEntry, FileHandle, FileService, Metadata, OpenFlags};

// Re-export nostr types for convenience
pub use nostr::{
    Event, EventTemplate, JobInput, JobRequest, finalize_event, get_public_key_hex,
    public_key_to_npub,
};

/// NIP-01 filter for subscriptions
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Filter {
    /// Event IDs to match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ids: Option<Vec<String>>,
    /// Author public keys to match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
    /// Event kinds to match
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<u16>>,
    /// Event e-tag references to match
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "#e")]
    pub e_tags: Option<Vec<String>>,
    /// Event p-tag references to match
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "#p")]
    pub p_tags: Option<Vec<String>>,
    /// Events created after this timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<u64>,
    /// Events created before this timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<u64>,
    /// Maximum number of events to return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
}

impl Filter {
    /// Create a new empty filter
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by event IDs
    pub fn ids(mut self, ids: Vec<String>) -> Self {
        self.ids = Some(ids);
        self
    }

    /// Filter by authors
    pub fn authors(mut self, authors: Vec<String>) -> Self {
        self.authors = Some(authors);
        self
    }

    /// Filter by event kinds
    pub fn kinds(mut self, kinds: Vec<u16>) -> Self {
        self.kinds = Some(kinds);
        self
    }

    /// Filter by e-tags (event references)
    pub fn e_tags(mut self, tags: Vec<String>) -> Self {
        self.e_tags = Some(tags);
        self
    }

    /// Filter by p-tags (pubkey references)
    pub fn p_tags(mut self, tags: Vec<String>) -> Self {
        self.p_tags = Some(tags);
        self
    }

    /// Events since timestamp
    pub fn since(mut self, timestamp: u64) -> Self {
        self.since = Some(timestamp);
        self
    }

    /// Events until timestamp
    pub fn until(mut self, timestamp: u64) -> Self {
        self.until = Some(timestamp);
        self
    }

    /// Limit number of results
    pub fn limit(mut self, limit: u64) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Nostr capability service
///
/// Provides Nostr event signing and NIP-90 DVM capabilities to agents.
/// Events are signed with the provided secret key and queued in the outbox.
/// External systems can read from the outbox and write to the inbox.
pub struct NostrFs {
    /// Secret key for signing (32 bytes)
    secret_key: [u8; 32],
    /// Public key hex (cached)
    pubkey_hex: String,
    /// Bech32 npub (cached)
    npub: String,
    /// Outbox: signed events waiting to be sent
    outbox: Arc<RwLock<HashMap<String, Event>>>,
    /// Inbox: received events
    inbox: Arc<RwLock<HashMap<String, Event>>>,
    /// Preferred relays
    relays: Arc<RwLock<Vec<String>>>,
    /// Active subscriptions: sub_id -> filters
    subscriptions: Arc<RwLock<HashMap<String, Vec<Filter>>>>,
    /// Track which events have been sent to which relays
    sent_events: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl NostrFs {
    /// Create a new NostrFs with the given secret key
    pub fn new(secret_key: [u8; 32]) -> Result<Self, FsError> {
        let pubkey_hex = get_public_key_hex(&secret_key)
            .map_err(|e| FsError::Io(format!("invalid secret key: {}", e)))?;

        // Convert pubkey to bytes for npub
        let pubkey_bytes = hex::decode(&pubkey_hex)
            .map_err(|e| FsError::Io(format!("invalid pubkey hex: {}", e)))?;
        let mut pubkey_arr = [0u8; 32];
        pubkey_arr.copy_from_slice(&pubkey_bytes);

        let npub = public_key_to_npub(&pubkey_arr)
            .map_err(|e| FsError::Io(format!("failed to create npub: {}", e)))?;

        Ok(Self {
            secret_key,
            pubkey_hex,
            npub,
            outbox: Arc::new(RwLock::new(HashMap::new())),
            inbox: Arc::new(RwLock::new(HashMap::new())),
            relays: Arc::new(RwLock::new(Vec::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            sent_events: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create with a randomly generated secret key
    pub fn generate() -> Result<Self, FsError> {
        let secret_key = nostr::generate_secret_key();
        Self::new(secret_key)
    }

    /// Get the public key hex
    pub fn pubkey(&self) -> &str {
        &self.pubkey_hex
    }

    /// Get the npub (bech32)
    pub fn npub(&self) -> &str {
        &self.npub
    }

    /// Get the secret key
    pub fn secret_key(&self) -> &[u8; 32] {
        &self.secret_key
    }

    /// Add a relay to the preferred list
    pub fn add_relay(&self, relay: impl Into<String>) {
        let mut relays = self.relays.write().unwrap();
        relays.push(relay.into());
    }

    /// Get preferred relays
    pub fn relays(&self) -> Vec<String> {
        self.relays.read().unwrap().clone()
    }

    /// Sign and queue an event template
    pub fn sign_event(&self, template: EventTemplate) -> Result<Event, FsError> {
        let event = finalize_event(&template, &self.secret_key)
            .map_err(|e| FsError::Io(format!("failed to sign event: {}", e)))?;

        // Add to outbox
        let mut outbox = self.outbox.write().unwrap();
        outbox.insert(event.id.clone(), event.clone());

        Ok(event)
    }

    /// Create and sign a NIP-90 job request
    pub fn create_job_request(
        &self,
        kind: u16,
        input: impl Into<String>,
        params: HashMap<String, String>,
    ) -> Result<Event, FsError> {
        let mut request = JobRequest::new(kind)
            .map_err(|e| FsError::Io(format!("invalid job kind: {}", e)))?
            .add_input(JobInput::text(input));

        // Add params
        for (key, value) in params {
            request = request.add_param(key, value);
        }

        // Add relays
        for relay in self.relays.read().unwrap().iter() {
            request = request.add_relay(relay);
        }

        // Create event template
        let template = EventTemplate {
            created_at: now(),
            kind: request.kind,
            tags: request.to_tags(),
            content: request.content.clone(),
        };

        self.sign_event(template)
    }

    /// Get all events in the outbox
    pub fn outbox_events(&self) -> Vec<Event> {
        self.outbox.read().unwrap().values().cloned().collect()
    }

    /// Remove an event from the outbox (after sending)
    pub fn remove_from_outbox(&self, event_id: &str) -> Option<Event> {
        self.outbox.write().unwrap().remove(event_id)
    }

    /// Clear all events from the outbox
    pub fn clear_outbox(&self) {
        self.outbox.write().unwrap().clear();
    }

    /// Add an event to the inbox (received from relay)
    pub fn add_to_inbox(&self, event: Event) {
        let mut inbox = self.inbox.write().unwrap();
        inbox.insert(event.id.clone(), event);
    }

    /// Get all events in the inbox
    pub fn inbox_events(&self) -> Vec<Event> {
        self.inbox.read().unwrap().values().cloned().collect()
    }

    /// Get a specific event from the inbox
    pub fn get_inbox_event(&self, event_id: &str) -> Option<Event> {
        self.inbox.read().unwrap().get(event_id).cloned()
    }

    /// Clear all events from the inbox
    pub fn clear_inbox(&self) {
        self.inbox.write().unwrap().clear();
    }

    /// Add a subscription with filters (for executor to manage)
    ///
    /// The executor will send a REQ message to relays with these filters.
    pub fn add_subscription(&self, sub_id: String, filters: Vec<Filter>) {
        let mut subs = self.subscriptions.write().unwrap();
        subs.insert(sub_id, filters);
    }

    /// Get all active subscriptions
    ///
    /// Returns (subscription_id, filters) pairs for the executor to manage.
    pub fn subscriptions(&self) -> Vec<(String, Vec<Filter>)> {
        let subs = self.subscriptions.read().unwrap();
        subs.iter()
            .map(|(id, filters)| (id.clone(), filters.clone()))
            .collect()
    }

    /// Remove a subscription
    pub fn remove_subscription(&self, sub_id: &str) {
        let mut subs = self.subscriptions.write().unwrap();
        subs.remove(sub_id);
    }

    /// Clear all subscriptions
    pub fn clear_subscriptions(&self) {
        self.subscriptions.write().unwrap().clear();
    }

    /// Mark an event as sent to a relay (for tracking)
    ///
    /// The executor calls this after successfully sending an event.
    pub fn mark_sent(&self, event_id: &str, relay: &str) {
        let mut sent = self.sent_events.write().unwrap();
        sent.entry(event_id.to_string())
            .or_default()
            .push(relay.to_string());
    }

    /// Get relays an event was sent to
    pub fn sent_to(&self, event_id: &str) -> Vec<String> {
        let sent = self.sent_events.read().unwrap();
        sent.get(event_id).cloned().unwrap_or_default()
    }

    /// Clear sent tracking for an event
    pub fn clear_sent(&self, event_id: &str) {
        let mut sent = self.sent_events.write().unwrap();
        sent.remove(event_id);
    }
}

impl FileService for NostrFs {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<Box<dyn FileHandle>, FsError> {
        let path = path.trim_start_matches('/');
        let parts: Vec<&str> = path.split('/').collect();

        match parts.as_slice() {
            // Identity files (read-only)
            ["identity", "pubkey"] => {
                if flags.write {
                    return Err(FsError::ReadOnly);
                }
                Ok(Box::new(StaticHandle::new(
                    self.pubkey_hex.as_bytes().to_vec(),
                )))
            }
            ["identity", "npub"] => {
                if flags.write {
                    return Err(FsError::ReadOnly);
                }
                Ok(Box::new(StaticHandle::new(self.npub.as_bytes().to_vec())))
            }

            // Status (read-only)
            ["status"] => {
                if flags.write {
                    return Err(FsError::ReadOnly);
                }
                let status = serde_json::json!({
                    "status": "ready",
                    "pubkey": self.pubkey_hex,
                    "outbox_count": self.outbox.read().unwrap().len(),
                    "inbox_count": self.inbox.read().unwrap().len(),
                    "relays": self.relays()
                });
                let json = serde_json::to_string_pretty(&status)
                    .map_err(|e| FsError::Io(e.to_string()))?;
                Ok(Box::new(StaticHandle::new(json.into_bytes())))
            }

            // Submit event template (write-only)
            ["submit"] => {
                if !flags.write {
                    return Err(FsError::PermissionDenied("submit is write-only".into()));
                }
                Ok(Box::new(SubmitHandle {
                    nostr: NostrFsRef {
                        secret_key: self.secret_key,
                        outbox: Arc::clone(&self.outbox),
                    },
                    buffer: Vec::new(),
                    mode: SubmitMode::Event,
                }))
            }

            // Submit NIP-90 job request (write-only)
            ["request"] => {
                if !flags.write {
                    return Err(FsError::PermissionDenied("request is write-only".into()));
                }
                Ok(Box::new(SubmitHandle {
                    nostr: NostrFsRef {
                        secret_key: self.secret_key,
                        outbox: Arc::clone(&self.outbox),
                    },
                    buffer: Vec::new(),
                    mode: SubmitMode::JobRequest,
                }))
            }

            // Outbox events
            ["outbox", event_id] => {
                let outbox = self.outbox.read().unwrap();
                let event = outbox
                    .get(*event_id)
                    .ok_or_else(|| FsError::NotFound(path.to_string()))?;
                let json =
                    serde_json::to_string_pretty(event).map_err(|e| FsError::Io(e.to_string()))?;
                Ok(Box::new(StaticHandle::new(json.into_bytes())))
            }

            // Inbox events
            ["inbox", event_id] => {
                let inbox = self.inbox.read().unwrap();
                let event = inbox
                    .get(*event_id)
                    .ok_or_else(|| FsError::NotFound(path.to_string()))?;
                let json =
                    serde_json::to_string_pretty(event).map_err(|e| FsError::Io(e.to_string()))?;
                Ok(Box::new(StaticHandle::new(json.into_bytes())))
            }

            _ => Err(FsError::NotFound(path.to_string())),
        }
    }

    fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        let path = path.trim_start_matches('/');

        match path {
            "" | "." => Ok(vec![
                DirEntry {
                    name: "identity".into(),
                    is_dir: true,
                    size: 0,
                },
                DirEntry {
                    name: "outbox".into(),
                    is_dir: true,
                    size: 0,
                },
                DirEntry {
                    name: "inbox".into(),
                    is_dir: true,
                    size: 0,
                },
                DirEntry {
                    name: "submit".into(),
                    is_dir: false,
                    size: 0,
                },
                DirEntry {
                    name: "request".into(),
                    is_dir: false,
                    size: 0,
                },
                DirEntry {
                    name: "status".into(),
                    is_dir: false,
                    size: 0,
                },
            ]),
            "identity" => Ok(vec![
                DirEntry {
                    name: "pubkey".into(),
                    is_dir: false,
                    size: self.pubkey_hex.len() as u64,
                },
                DirEntry {
                    name: "npub".into(),
                    is_dir: false,
                    size: self.npub.len() as u64,
                },
            ]),
            "outbox" => {
                let outbox = self.outbox.read().unwrap();
                Ok(outbox
                    .keys()
                    .map(|id| DirEntry {
                        name: format!("{}.json", id),
                        is_dir: false,
                        size: 0,
                    })
                    .collect())
            }
            "inbox" => {
                let inbox = self.inbox.read().unwrap();
                Ok(inbox
                    .keys()
                    .map(|id| DirEntry {
                        name: format!("{}.json", id),
                        is_dir: false,
                        size: 0,
                    })
                    .collect())
            }
            _ => Err(FsError::NotADirectory(path.to_string())),
        }
    }

    fn stat(&self, path: &str) -> Result<Metadata, FsError> {
        let path = path.trim_start_matches('/');

        match path {
            "" | "." | "identity" | "outbox" | "inbox" => Ok(Metadata {
                is_dir: true,
                size: 0,
                modified: now(),
                readonly: true,
            }),
            "identity/pubkey" => Ok(Metadata {
                is_dir: false,
                size: self.pubkey_hex.len() as u64,
                modified: now(),
                readonly: true,
            }),
            "identity/npub" => Ok(Metadata {
                is_dir: false,
                size: self.npub.len() as u64,
                modified: now(),
                readonly: true,
            }),
            "submit" | "request" => Ok(Metadata {
                is_dir: false,
                size: 0,
                modified: now(),
                readonly: false,
            }),
            "status" => Ok(Metadata {
                is_dir: false,
                size: 0,
                modified: now(),
                readonly: true,
            }),
            p if p.starts_with("outbox/") || p.starts_with("inbox/") => Ok(Metadata {
                is_dir: false,
                size: 0,
                modified: now(),
                readonly: true,
            }),
            _ => Err(FsError::NotFound(path.to_string())),
        }
    }

    fn mkdir(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::ReadOnly)
    }

    fn remove(&self, _path: &str) -> Result<(), FsError> {
        Err(FsError::ReadOnly)
    }

    fn rename(&self, _from: &str, _to: &str) -> Result<(), FsError> {
        Err(FsError::ReadOnly)
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

/// Submit mode for the submit handle
#[derive(Clone, Copy)]
enum SubmitMode {
    Event,
    JobRequest,
}

/// Reference to NostrFs data for handles
struct NostrFsRef {
    secret_key: [u8; 32],
    outbox: Arc<RwLock<HashMap<String, Event>>>,
}

/// Write handle for /submit and /request
struct SubmitHandle {
    nostr: NostrFsRef,
    buffer: Vec<u8>,
    mode: SubmitMode,
}

impl FileHandle for SubmitHandle {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, FsError> {
        Err(FsError::PermissionDenied("submit is write-only".into()))
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

        let event = match self.mode {
            SubmitMode::Event => {
                // Parse as event template
                let template: EventTemplateJson = serde_json::from_str(&json_str)
                    .map_err(|e| FsError::Io(format!("invalid event template: {}", e)))?;

                let event_template = EventTemplate {
                    created_at: template.created_at.unwrap_or_else(now),
                    kind: template.kind,
                    tags: template.tags.unwrap_or_default(),
                    content: template.content,
                };

                finalize_event(&event_template, &self.nostr.secret_key)
                    .map_err(|e| FsError::Io(format!("failed to sign event: {}", e)))?
            }
            SubmitMode::JobRequest => {
                // Parse as job request
                let req: JobRequestJson = serde_json::from_str(&json_str)
                    .map_err(|e| FsError::Io(format!("invalid job request: {}", e)))?;

                let mut request = JobRequest::new(req.kind)
                    .map_err(|e| FsError::Io(format!("invalid job kind: {}", e)))?
                    .add_input(JobInput::text(req.input));

                // Add params
                if let Some(params) = req.params {
                    for (key, value) in params {
                        request = request.add_param(key, value);
                    }
                }

                // Add relays
                if let Some(relays) = req.relays {
                    for relay in relays {
                        request = request.add_relay(relay);
                    }
                }

                // Add bid
                if let Some(bid) = req.bid {
                    request = request.with_bid(bid);
                }

                let template = EventTemplate {
                    created_at: now(),
                    kind: request.kind,
                    tags: request.to_tags(),
                    content: request.content.clone(),
                };

                finalize_event(&template, &self.nostr.secret_key)
                    .map_err(|e| FsError::Io(format!("failed to sign event: {}", e)))?
            }
        };

        // Add to outbox
        let mut outbox = self.nostr.outbox.write().unwrap();
        outbox.insert(event.id.clone(), event);

        self.buffer.clear();
        Ok(())
    }
}

/// JSON format for event template input
#[derive(serde::Deserialize)]
struct EventTemplateJson {
    kind: u16,
    content: String,
    #[serde(default)]
    tags: Option<Vec<Vec<String>>>,
    #[serde(default)]
    created_at: Option<u64>,
}

/// JSON format for job request input
#[derive(serde::Deserialize)]
struct JobRequestJson {
    kind: u16,
    input: String,
    #[serde(default)]
    params: Option<HashMap<String, String>>,
    #[serde(default)]
    relays: Option<Vec<String>>,
    #[serde(default)]
    bid: Option<u64>,
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
    use nostr::KIND_JOB_TEXT_GENERATION;

    // Test key (same as in nostr crate tests)
    fn test_secret_key() -> [u8; 32] {
        let hex = "d217c1ff2f8a65c3e3a1740db3b9f58b8c848bb45e26d00ed4714e4a0f4ceecf";
        let bytes = hex::decode(hex).unwrap();
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        key
    }

    #[test]
    fn test_nostr_fs_creation() {
        let nostr = NostrFs::new(test_secret_key()).unwrap();
        assert!(!nostr.pubkey().is_empty());
        assert!(nostr.npub().starts_with("npub1"));
    }

    #[test]
    fn test_nostr_fs_generate() {
        let nostr = NostrFs::generate().unwrap();
        assert!(!nostr.pubkey().is_empty());
        assert!(nostr.npub().starts_with("npub1"));
    }

    #[test]
    fn test_read_identity() {
        let nostr = NostrFs::new(test_secret_key()).unwrap();

        // Read pubkey
        let mut handle = nostr
            .open("/identity/pubkey", OpenFlags::read_only())
            .unwrap();
        let mut buf = vec![0u8; 1024];
        let n = handle.read(&mut buf).unwrap();
        let pubkey = String::from_utf8(buf[..n].to_vec()).unwrap();
        assert_eq!(pubkey, nostr.pubkey());

        // Read npub
        let mut handle = nostr
            .open("/identity/npub", OpenFlags::read_only())
            .unwrap();
        let n = handle.read(&mut buf).unwrap();
        let npub = String::from_utf8(buf[..n].to_vec()).unwrap();
        assert!(npub.starts_with("npub1"));
    }

    #[test]
    fn test_read_status() {
        let nostr = NostrFs::new(test_secret_key()).unwrap();
        nostr.add_relay("wss://relay.example.com");

        let mut handle = nostr.open("/status", OpenFlags::read_only()).unwrap();
        let mut buf = vec![0u8; 4096];
        let n = handle.read(&mut buf).unwrap();
        let status: serde_json::Value = serde_json::from_slice(&buf[..n]).unwrap();

        assert_eq!(status["status"], "ready");
        assert!(status["relays"].as_array().unwrap().len() > 0);
    }

    #[test]
    fn test_sign_event() {
        let nostr = NostrFs::new(test_secret_key()).unwrap();

        let template = EventTemplate {
            created_at: 1617932115,
            kind: 1,
            tags: vec![],
            content: "Hello from OANIX!".to_string(),
        };

        let event = nostr.sign_event(template).unwrap();
        assert!(!event.id.is_empty());
        assert!(!event.sig.is_empty());
        assert_eq!(event.kind, 1);
        assert_eq!(event.content, "Hello from OANIX!");

        // Should be in outbox
        assert_eq!(nostr.outbox_events().len(), 1);
    }

    #[test]
    fn test_create_job_request() {
        let nostr = NostrFs::new(test_secret_key()).unwrap();
        nostr.add_relay("wss://relay.damus.io");

        let mut params = HashMap::new();
        params.insert("model".to_string(), "gpt-4".to_string());

        let event = nostr
            .create_job_request(KIND_JOB_TEXT_GENERATION, "What is 2+2?", params)
            .unwrap();

        assert_eq!(event.kind, 5050); // Text generation
        assert!(
            event
                .tags
                .iter()
                .any(|t| t[0] == "i" && t[1] == "What is 2+2?")
        );
        assert!(
            event
                .tags
                .iter()
                .any(|t| t[0] == "param" && t[1] == "model")
        );

        // Should be in outbox
        assert_eq!(nostr.outbox_events().len(), 1);
    }

    #[test]
    fn test_submit_event_via_file() {
        let nostr = NostrFs::new(test_secret_key()).unwrap();

        // Write event template to /submit
        let template_json = r#"{"kind":1,"content":"Hello via file!"}"#;
        let mut handle = nostr.open("/submit", OpenFlags::write_only()).unwrap();
        handle.write(template_json.as_bytes()).unwrap();
        handle.flush().unwrap();

        // Should be in outbox
        let events = nostr.outbox_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, 1);
        assert_eq!(events[0].content, "Hello via file!");
    }

    #[test]
    fn test_submit_job_request_via_file() {
        let nostr = NostrFs::new(test_secret_key()).unwrap();

        // Write job request to /request
        let request_json = r#"{"kind":5050,"input":"Tell me a joke","params":{"style":"funny"}}"#;
        let mut handle = nostr.open("/request", OpenFlags::write_only()).unwrap();
        handle.write(request_json.as_bytes()).unwrap();
        handle.flush().unwrap();

        // Should be in outbox
        let events = nostr.outbox_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, 5050);
    }

    #[test]
    fn test_outbox_operations() {
        let nostr = NostrFs::new(test_secret_key()).unwrap();

        // Sign an event
        let template = EventTemplate {
            created_at: 1617932115,
            kind: 1,
            tags: vec![],
            content: "Test".to_string(),
        };
        let event = nostr.sign_event(template).unwrap();
        let event_id = event.id.clone();

        // List outbox
        let entries = nostr.readdir("/outbox").unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].name.contains(&event_id));

        // Read event from outbox
        let mut handle = nostr
            .open(&format!("/outbox/{}", event_id), OpenFlags::read_only())
            .unwrap();
        let mut buf = vec![0u8; 4096];
        let n = handle.read(&mut buf).unwrap();
        let read_event: Event = serde_json::from_slice(&buf[..n]).unwrap();
        assert_eq!(read_event.id, event_id);

        // Remove from outbox
        nostr.remove_from_outbox(&event_id);
        assert!(nostr.outbox_events().is_empty());
    }

    #[test]
    fn test_inbox_operations() {
        let nostr = NostrFs::new(test_secret_key()).unwrap();

        // Create a mock event
        let event = Event {
            id: "abc123".to_string(),
            pubkey: "def456".repeat(4),
            created_at: 1617932115,
            kind: 1,
            tags: vec![],
            content: "Incoming message".to_string(),
            sig: "sig".repeat(32),
        };

        // Add to inbox
        nostr.add_to_inbox(event.clone());

        // List inbox
        let entries = nostr.readdir("/inbox").unwrap();
        assert_eq!(entries.len(), 1);

        // Read event from inbox
        let read_event = nostr.get_inbox_event("abc123").unwrap();
        assert_eq!(read_event.content, "Incoming message");

        // Clear inbox
        nostr.clear_inbox();
        assert!(nostr.inbox_events().is_empty());
    }

    #[test]
    fn test_readdir_root() {
        let nostr = NostrFs::new(test_secret_key()).unwrap();
        let entries = nostr.readdir("/").unwrap();

        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"identity"));
        assert!(names.contains(&"outbox"));
        assert!(names.contains(&"inbox"));
        assert!(names.contains(&"submit"));
        assert!(names.contains(&"request"));
        assert!(names.contains(&"status"));
    }

    #[test]
    fn test_identity_readonly() {
        let nostr = NostrFs::new(test_secret_key()).unwrap();

        // Try to write to identity
        let result = nostr.open(
            "/identity/pubkey",
            OpenFlags {
                write: true,
                ..Default::default()
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_relays() {
        let nostr = NostrFs::new(test_secret_key()).unwrap();

        assert!(nostr.relays().is_empty());

        nostr.add_relay("wss://relay1.com");
        nostr.add_relay("wss://relay2.com");

        assert_eq!(nostr.relays().len(), 2);
        assert!(nostr.relays().contains(&"wss://relay1.com".to_string()));
    }
}
