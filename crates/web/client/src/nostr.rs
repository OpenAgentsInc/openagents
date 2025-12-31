use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Relay connection status
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum RelayStatus {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

impl Default for RelayStatus {
    fn default() -> Self {
        Self::Disconnected
    }
}

/// NIP-90 Job Type - matches vendata.io kinds plus standard NIP-90
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum JobType {
    TextExtraction,     // 5000, 65002
    Summarization,      // 5001, 65003
    Translation { target_lang: Option<String> }, // 5002, 5100, 65004
    TextGeneration,     // 5050
    ImageGeneration,    // 5250, 65005
    SpeechToText,       // 5300
    TextToSpeech,       // 5301
    NostrDiscovery,     // 65006
    NostrFiltering,     // 65007
    Unknown(u16),
}

impl JobType {
    /// Create JobType from a NIP-90 request kind
    pub(crate) fn from_kind(kind: u16) -> Self {
        match kind {
            5000 | 65002 => JobType::TextExtraction,
            5001 | 65003 => JobType::Summarization,
            5002 | 5100 | 65004 => JobType::Translation { target_lang: None },
            5050 => JobType::TextGeneration,
            5250 | 65005 => JobType::ImageGeneration,
            5300 => JobType::SpeechToText,
            5301 => JobType::TextToSpeech,
            65006 => JobType::NostrDiscovery,
            65007 => JobType::NostrFiltering,
            _ => JobType::Unknown(kind),
        }
    }

    /// Get the result kind for this job type
    #[allow(dead_code)]
    pub(crate) fn result_kind(request_kind: u16) -> u16 {
        if request_kind >= 5000 && request_kind < 6000 {
            request_kind + 1000
        } else if request_kind >= 65000 {
            65001 // vendata.io style result kind
        } else {
            request_kind + 1000
        }
    }

    /// Get human-readable label
    pub(crate) fn label(&self) -> &'static str {
        match self {
            JobType::TextExtraction => "Text Extraction",
            JobType::Summarization => "Summarization",
            JobType::Translation { .. } => "Translation",
            JobType::TextGeneration => "Text Generation",
            JobType::ImageGeneration => "Image Generation",
            JobType::SpeechToText => "Speech-to-Text",
            JobType::TextToSpeech => "Text-to-Speech",
            JobType::NostrDiscovery => "Nostr Discovery",
            JobType::NostrFiltering => "Nostr Filter",
            JobType::Unknown(_) => "DVM Job",
        }
    }

    /// Get short badge text for compact display
    pub(crate) fn badge(&self) -> &'static str {
        match self {
            JobType::TextExtraction => "TXT",
            JobType::Summarization => "SUM",
            JobType::Translation { .. } => "TRANS",
            JobType::TextGeneration => "GEN",
            JobType::ImageGeneration => "IMG",
            JobType::SpeechToText => "STT",
            JobType::TextToSpeech => "TTS",
            JobType::NostrDiscovery => "DSC",
            JobType::NostrFiltering => "FLT",
            JobType::Unknown(_) => "DVM",
        }
    }

    /// Get badge with target language for translation
    pub(crate) fn badge_with_lang(&self) -> String {
        match self {
            JobType::Translation { target_lang: Some(lang) } => {
                let short_lang = if lang.len() > 2 { &lang[..2] } else { lang };
                format!("→{}", short_lang.to_uppercase())
            }
            _ => self.badge().to_string(),
        }
    }
}

/// Type of NIP-90 event
#[derive(Clone, Debug)]
pub(crate) enum Nip90EventType {
    JobRequest {
        job_type: JobType,
        inputs: Vec<Nip90Input>,
    },
    JobResult {
        request_id: Option<String>,
        request_kind: Option<u16>,
    },
    JobFeedback {
        status: String,
        request_id: Option<String>,
    },
}

/// Input to a NIP-90 job request
#[derive(Clone, Debug)]
pub(crate) struct Nip90Input {
    pub(crate) value: String,
    pub(crate) input_type: Option<String>, // "url", "text", "event", "job", etc.
    pub(crate) relay: Option<String>,
    pub(crate) marker: Option<String>,
}

/// A parsed NIP-90 event for display
#[derive(Clone, Debug)]
pub(crate) struct Nip90Event {
    pub(crate) id: String,
    pub(crate) kind: u16,
    pub(crate) pubkey: String,
    pub(crate) created_at: u64,
    pub(crate) content: String,
    pub(crate) event_type: Nip90EventType,
}

impl Nip90Event {
    /// Get the job type for this event
    pub(crate) fn job_type(&self) -> Option<&JobType> {
        match &self.event_type {
            Nip90EventType::JobRequest { job_type, .. } => Some(job_type),
            _ => None,
        }
    }

    /// Get a descriptive label for the event type
    pub(crate) fn type_label(&self) -> String {
        match &self.event_type {
            Nip90EventType::JobRequest { job_type, .. } => job_type.label().to_string(),
            Nip90EventType::JobResult { request_kind: Some(k), .. } => {
                format!("{} Result", JobType::from_kind(*k).label())
            }
            Nip90EventType::JobResult { .. } => "Result".to_string(),
            Nip90EventType::JobFeedback { status, .. } => {
                match status.as_str() {
                    "processing" => "Processing".to_string(),
                    "success" => "Success".to_string(),
                    "error" => "Error".to_string(),
                    "payment-required" => "Payment Required".to_string(),
                    "partial" => "Partial".to_string(),
                    _ => format!("Status: {}", status),
                }
            }
        }
    }

    /// Get badge text for compact display
    pub(crate) fn badge(&self) -> String {
        match &self.event_type {
            Nip90EventType::JobRequest { job_type, .. } => job_type.badge_with_lang(),
            Nip90EventType::JobResult { request_kind: Some(k), .. } => {
                format!("{}✓", JobType::from_kind(*k).badge())
            }
            Nip90EventType::JobResult { .. } => "RES".to_string(),
            Nip90EventType::JobFeedback { status, .. } => {
                match status.as_str() {
                    "processing" => "...".to_string(),
                    "success" => "OK".to_string(),
                    "error" => "ERR".to_string(),
                    "payment-required" => "PAY".to_string(),
                    _ => "FB".to_string(),
                }
            }
        }
    }

    /// Get truncated pubkey for display (first 8 chars of hex)
    pub(crate) fn short_pubkey(&self) -> String {
        if self.pubkey.len() > 8 {
            format!("{}...", &self.pubkey[..8])
        } else {
            self.pubkey.clone()
        }
    }

    /// Get the best displayable content for this event
    pub(crate) fn display_content(&self, max_len: usize) -> String {
        let text = match &self.event_type {
            Nip90EventType::JobRequest { inputs, .. } => {
                // Find the best input to display (prefer text over URLs, skip event/job refs)
                let text_input = inputs.iter().find(|i| {
                    i.input_type.as_deref() != Some("event") &&
                    i.input_type.as_deref() != Some("job") &&
                    !i.value.is_empty()
                });
                if let Some(input) = text_input {
                    input.value.clone()
                } else if !self.content.is_empty() && !self.is_json_content() {
                    self.content.clone()
                } else {
                    // Show reference if that's all we have
                    inputs.first()
                        .map(|i| match i.input_type.as_deref() {
                            Some("event") => format!("event:{}", &i.value[..8.min(i.value.len())]),
                            Some("job") => format!("job:{}", &i.value[..8.min(i.value.len())]),
                            _ => i.value.clone(),
                        })
                        .unwrap_or_default()
                }
            }
            Nip90EventType::JobResult { .. } => {
                if self.is_json_content() {
                    // Try to extract something useful from JSON result
                    self.extract_json_preview()
                } else if self.is_url_content() {
                    // Show URLs (could be image results)
                    self.content.clone()
                } else if !self.content.is_empty() {
                    self.content.clone()
                } else {
                    String::new()
                }
            }
            Nip90EventType::JobFeedback { status, .. } => {
                if !self.content.is_empty() && !self.is_json_content() {
                    self.content.clone()
                } else {
                    status.clone()
                }
            }
        };

        // Clean up the text
        let text = text.replace('\n', " ").replace('\r', "").replace('\t', " ");
        let text = text.trim();

        if text.is_empty() {
            return String::new();
        }

        if text.len() > max_len {
            format!("{}...", &text[..max_len.saturating_sub(3)])
        } else {
            text.to_string()
        }
    }

    /// Check if content looks like JSON
    fn is_json_content(&self) -> bool {
        let trimmed = self.content.trim();
        trimmed.starts_with('[') || trimmed.starts_with('{')
    }

    /// Check if content is a URL
    fn is_url_content(&self) -> bool {
        self.content.starts_with("http://") || self.content.starts_with("https://")
    }

    /// Try to extract a preview from JSON content
    fn extract_json_preview(&self) -> String {
        // Try to parse as JSON array of tags (vendata.io style results)
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&self.content) {
            if let Some(arr) = parsed.as_array() {
                // Look for useful content in tag arrays
                for item in arr.iter().take(3) {
                    if let Some(tag_arr) = item.as_array() {
                        if tag_arr.len() >= 2 {
                            let tag_type = tag_arr[0].as_str().unwrap_or("");
                            let tag_value = tag_arr[1].as_str().unwrap_or("");
                            match tag_type {
                                "content" | "text" | "result" => {
                                    if !tag_value.is_empty() {
                                        return tag_value.to_string();
                                    }
                                }
                                "p" => return format!("user:{}", &tag_value[..8.min(tag_value.len())]),
                                "e" => return format!("event:{}", &tag_value[..8.min(tag_value.len())]),
                                _ => {}
                            }
                        }
                    }
                }
                // Show count if we can't extract content
                return format!("[{} items]", arr.len());
            } else if let Some(obj) = parsed.as_object() {
                // Object result - try to find text field
                for key in &["content", "text", "result", "output", "message"] {
                    if let Some(v) = obj.get(*key) {
                        if let Some(s) = v.as_str() {
                            return s.to_string();
                        }
                    }
                }
            }
        }
        String::new()
    }

    /// Get relative time string (e.g. "2m ago", "1h ago")
    pub(crate) fn relative_time(&self, now_secs: u64) -> String {
        if self.created_at > now_secs {
            return "now".to_string();
        }
        let diff = now_secs - self.created_at;
        if diff < 60 {
            format!("{}s", diff)
        } else if diff < 3600 {
            format!("{}m", diff / 60)
        } else if diff < 86400 {
            format!("{}h", diff / 3600)
        } else {
            format!("{}d", diff / 86400)
        }
    }
}

/// Parse a Nostr event from JSON
fn parse_nostr_event(json: &str) -> Option<Nip90Event> {
    // Parse the relay message: ["EVENT", "sub_id", {event}]
    let parsed: serde_json::Value = serde_json::from_str(json).ok()?;
    let arr = parsed.as_array()?;

    if arr.len() < 3 {
        return None;
    }

    let msg_type = arr[0].as_str()?;
    if msg_type != "EVENT" {
        return None;
    }

    let event = &arr[2];

    let id = event["id"].as_str()?.to_string();
    let kind = event["kind"].as_u64()? as u16;
    let pubkey = event["pubkey"].as_str()?.to_string();
    let created_at = event["created_at"].as_u64()?;
    let content = event["content"].as_str().unwrap_or("").to_string();
    let tags = event["tags"].as_array();

    // Determine event type based on kind
    let event_type = if is_job_request_kind(kind) {
        // Parse job type with potential translation target language
        let mut job_type = JobType::from_kind(kind);

        // Extract target language from param tags for translation jobs
        if matches!(job_type, JobType::Translation { .. }) {
            if let Some(tags) = tags {
                for tag in tags {
                    if let Some(arr) = tag.as_array() {
                        if arr.len() >= 3 {
                            let tag_type = arr.get(0).and_then(|v| v.as_str()).unwrap_or("");
                            let param_name = arr.get(1).and_then(|v| v.as_str()).unwrap_or("");
                            let param_value = arr.get(2).and_then(|v| v.as_str()).unwrap_or("");
                            if tag_type == "param" && (param_name == "lang" || param_name == "language") {
                                job_type = JobType::Translation { target_lang: Some(param_value.to_string()) };
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Extract all inputs from "i" tags
        let inputs = tags.map(|tags| {
            tags.iter().filter_map(|tag| {
                let arr = tag.as_array()?;
                if arr.get(0)?.as_str()? != "i" {
                    return None;
                }
                let value = arr.get(1)?.as_str()?.to_string();
                let input_type = arr.get(2).and_then(|v| v.as_str()).map(|s| s.to_string());
                let relay = arr.get(3).and_then(|v| v.as_str()).map(|s| s.to_string());
                let marker = arr.get(4).and_then(|v| v.as_str()).map(|s| s.to_string());
                Some(Nip90Input { value, input_type, relay, marker })
            }).collect::<Vec<_>>()
        }).unwrap_or_default();

        Nip90EventType::JobRequest { job_type, inputs }
    } else if is_job_result_kind(kind) {
        // Try to find request ID and infer request kind from e tag
        let (request_id, request_kind) = tags.map(|tags| {
            let mut req_id = None;
            let mut req_kind = None;
            for tag in tags {
                if let Some(arr) = tag.as_array() {
                    let tag_type = arr.get(0).and_then(|v| v.as_str()).unwrap_or("");
                    if tag_type == "e" {
                        req_id = arr.get(1).and_then(|v| v.as_str()).map(|s| s.to_string());
                    } else if tag_type == "request" {
                        // Some DVMs include the original request - extract its kind
                        if let Some(req_json) = arr.get(1).and_then(|v| v.as_str()) {
                            if let Ok(req_event) = serde_json::from_str::<serde_json::Value>(req_json) {
                                req_kind = req_event["kind"].as_u64().map(|k| k as u16);
                            }
                        }
                    }
                }
            }
            // Infer request kind from result kind if not found
            if req_kind.is_none() && kind >= 6000 && kind < 7000 {
                req_kind = Some(kind - 1000);
            }
            (req_id, req_kind)
        }).unwrap_or((None, Some(kind - 1000)));

        Nip90EventType::JobResult { request_id, request_kind }
    } else if kind == 7000 {
        // Job feedback - extract status and request ID
        let (status, request_id) = tags.map(|tags| {
            let mut status = "unknown".to_string();
            let mut req_id = None;
            for tag in tags {
                if let Some(arr) = tag.as_array() {
                    let tag_type = arr.get(0).and_then(|v| v.as_str()).unwrap_or("");
                    if tag_type == "status" {
                        status = arr.get(1).and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                    } else if tag_type == "e" {
                        req_id = arr.get(1).and_then(|v| v.as_str()).map(|s| s.to_string());
                    }
                }
            }
            (status, req_id)
        }).unwrap_or(("unknown".to_string(), None));

        Nip90EventType::JobFeedback { status, request_id }
    } else if is_vendata_kind(kind) {
        // vendata.io style kinds (65002-65007)
        let mut job_type = JobType::from_kind(kind);

        // Extract target language for translation
        if matches!(job_type, JobType::Translation { .. }) {
            if let Some(tags) = tags {
                for tag in tags {
                    if let Some(arr) = tag.as_array() {
                        if arr.len() >= 3 {
                            let tag_type = arr.get(0).and_then(|v| v.as_str()).unwrap_or("");
                            let param_name = arr.get(1).and_then(|v| v.as_str()).unwrap_or("");
                            let param_value = arr.get(2).and_then(|v| v.as_str()).unwrap_or("");
                            if tag_type == "param" && (param_name == "lang" || param_name == "language") {
                                job_type = JobType::Translation { target_lang: Some(param_value.to_string()) };
                                break;
                            }
                        }
                    }
                }
            }
        }

        let inputs = tags.map(|tags| {
            tags.iter().filter_map(|tag| {
                let arr = tag.as_array()?;
                if arr.get(0)?.as_str()? != "i" {
                    return None;
                }
                let value = arr.get(1)?.as_str()?.to_string();
                let input_type = arr.get(2).and_then(|v| v.as_str()).map(|s| s.to_string());
                let relay = arr.get(3).and_then(|v| v.as_str()).map(|s| s.to_string());
                let marker = arr.get(4).and_then(|v| v.as_str()).map(|s| s.to_string());
                Some(Nip90Input { value, input_type, relay, marker })
            }).collect::<Vec<_>>()
        }).unwrap_or_default();

        Nip90EventType::JobRequest { job_type, inputs }
    } else if kind == 65001 {
        // vendata.io style result
        let request_id = tags.and_then(|tags| {
            tags.iter().find_map(|tag| {
                let arr = tag.as_array()?;
                if arr.get(0)?.as_str()? == "e" {
                    arr.get(1)?.as_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
        });
        Nip90EventType::JobResult { request_id, request_kind: None }
    } else {
        return None;
    };

    Some(Nip90Event {
        id,
        kind,
        pubkey,
        created_at,
        content,
        event_type,
    })
}

/// Check if kind is a standard NIP-90 job request
fn is_job_request_kind(kind: u16) -> bool {
    kind >= 5000 && kind < 6000
}

/// Check if kind is a standard NIP-90 job result
fn is_job_result_kind(kind: u16) -> bool {
    kind >= 6000 && kind < 7000
}

/// Check if kind is a vendata.io style kind (65002-65007)
fn is_vendata_kind(kind: u16) -> bool {
    kind >= 65002 && kind <= 65007
}

/// Build the subscription message for NIP-90 events
fn build_nip90_subscription() -> String {
    // Subscribe to NIP-90 job requests (5000-5999), results (6000-6999), and feedback (7000)
    // Also include vendata.io style kinds (65002-65007)
    let kinds: Vec<u16> = vec![
        // Standard NIP-90 job request kinds
        5000, 5001, 5002, 5050, 5100, 5250, 5300, 5301,
        // Corresponding result kinds
        6000, 6001, 6002, 6050, 6100, 6250, 6300, 6301,
        // Feedback
        7000,
        // vendata.io style kinds
        65001, 65002, 65003, 65004, 65005, 65006, 65007,
    ];

    serde_json::json!([
        "REQ",
        "nip90-sub",
        {
            "kinds": kinds,
            "limit": 100
        }
    ]).to_string()
}

/// Build the subscription message for NIP-89 DVM announcements
fn build_nip89_subscription() -> String {
    // Subscribe to NIP-89 DVM announcements (kind 31990)
    // Filter by #k tags for job kinds we're interested in
    serde_json::json!([
        "REQ",
        "nip89-dvms",
        {
            "kinds": [31990],
            "#k": ["5000", "5001", "5002", "5050", "5100", "5250", "5300", "5301",
                   "65002", "65003", "65004", "65005", "65006", "65007"],
            "limit": 50
        }
    ]).to_string()
}

/// Handle for a connected Nostr relay
pub(crate) struct NostrRelayHandle {
    pub(crate) ws: web_sys::WebSocket,
}

impl NostrRelayHandle {
    pub(crate) fn close(self) {
        let _ = self.ws.close();
    }
}

/// State for NIP-90 events display
#[derive(Clone, Default)]
pub(crate) struct Nip90State {
    pub(crate) events: Vec<Nip90Event>,
    pub(crate) relay_status: RelayStatus,
    pub(crate) relay_url: String,
    pub(crate) scroll_offset: f32,
}

impl Nip90State {
    pub(crate) fn new() -> Self {
        Self {
            events: Vec::new(),
            relay_status: RelayStatus::Disconnected,
            relay_url: String::new(),
            scroll_offset: 0.0,
        }
    }

    /// Add an event, keeping sorted by timestamp (most recent first), capped at max_events
    pub(crate) fn add_event(&mut self, event: Nip90Event, max_events: usize) {
        // Avoid duplicates
        if self.events.iter().any(|e| e.id == event.id) {
            return;
        }

        // Find insertion point to keep sorted by created_at (descending)
        let insert_idx = self.events
            .iter()
            .position(|e| e.created_at < event.created_at)
            .unwrap_or(self.events.len());

        self.events.insert(insert_idx, event);
        // Truncate to max
        self.events.truncate(max_events);
    }

    /// Get a grouped job by request ID
    pub(crate) fn get_job(&self, request_id: &str) -> Option<DvmJob> {
        // Find the request event
        let request = self.events.iter().find(|e| {
            e.id == request_id && matches!(e.event_type, Nip90EventType::JobRequest { .. })
        })?;

        let mut job = DvmJob::new(request.clone());

        // Find all related results and feedback
        for event in &self.events {
            match &event.event_type {
                Nip90EventType::JobResult { request_id: Some(rid), .. } if rid == request_id => {
                    job.results.push(event.clone());
                }
                Nip90EventType::JobFeedback { request_id: Some(rid), .. } if rid == request_id => {
                    job.feedback.push(event.clone());
                }
                _ => {}
            }
        }

        // Sort results by time (oldest first for reading order)
        job.results.sort_by_key(|e| e.created_at);
        job.feedback.sort_by_key(|e| e.created_at);

        Some(job)
    }

    /// Get only job request events (for the feed)
    pub(crate) fn job_requests(&self) -> Vec<&Nip90Event> {
        self.events.iter()
            .filter(|e| matches!(e.event_type, Nip90EventType::JobRequest { .. }))
            .collect()
    }
}

/// Which view is currently active in the DVM section
#[derive(Clone, PartialEq, Debug, Default)]
pub(crate) enum DvmView {
    #[default]
    Feed,       // Live job stream
    Directory,  // DVM list from NIP-89
    JobDetail(String),  // Viewing specific job by ID
}

/// A grouped job with request, results, and feedback
#[derive(Clone, Debug)]
pub(crate) struct DvmJob {
    pub(crate) request: Nip90Event,
    pub(crate) results: Vec<Nip90Event>,
    pub(crate) feedback: Vec<Nip90Event>,
}

impl DvmJob {
    /// Create a new job from a request event
    pub(crate) fn new(request: Nip90Event) -> Self {
        Self {
            request,
            results: Vec::new(),
            feedback: Vec::new(),
        }
    }

    /// Get all results grouped by DVM pubkey
    pub(crate) fn results_by_dvm(&self) -> Vec<(&str, Vec<&Nip90Event>)> {
        let mut by_pubkey: std::collections::HashMap<&str, Vec<&Nip90Event>> = std::collections::HashMap::new();
        for result in &self.results {
            by_pubkey.entry(&result.pubkey).or_default().push(result);
        }
        let mut sorted: Vec<_> = by_pubkey.into_iter().collect();
        sorted.sort_by(|a, b| {
            // Sort by earliest response time
            let a_time = a.1.iter().map(|e| e.created_at).min().unwrap_or(u64::MAX);
            let b_time = b.1.iter().map(|e| e.created_at).min().unwrap_or(u64::MAX);
            a_time.cmp(&b_time)
        });
        sorted
    }

    /// Get the number of unique DVMs that responded
    pub(crate) fn dvm_count(&self) -> usize {
        let mut pubkeys: Vec<&str> = self.results.iter().map(|e| e.pubkey.as_str()).collect();
        pubkeys.sort();
        pubkeys.dedup();
        pubkeys.len()
    }
}

/// A DVM (Data Vending Machine) discovered via NIP-89 announcement
#[derive(Clone, Debug)]
pub(crate) struct Dvm {
    pub(crate) pubkey: String,
    pub(crate) name: Option<String>,
    pub(crate) about: Option<String>,
    pub(crate) picture: Option<String>,
    pub(crate) supported_kinds: Vec<u16>,
    pub(crate) nip89_event_id: String,
    pub(crate) created_at: u64,
}

impl Dvm {
    /// Get supported job types as JobType enums
    pub(crate) fn supported_job_types(&self) -> Vec<JobType> {
        self.supported_kinds.iter()
            .map(|k| JobType::from_kind(*k))
            .collect()
    }

    /// Get short pubkey for display
    pub(crate) fn short_pubkey(&self) -> String {
        if self.pubkey.len() > 8 {
            format!("{}...", &self.pubkey[..8])
        } else {
            self.pubkey.clone()
        }
    }

    /// Get display name (name or truncated pubkey)
    pub(crate) fn display_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.short_pubkey())
    }
}

/// State for DVM directory
#[derive(Clone, Default)]
pub(crate) struct DvmDirectoryState {
    pub(crate) dvms: Vec<Dvm>,
    pub(crate) current_view: DvmView,
    pub(crate) scroll_offset: f32,
}

impl DvmDirectoryState {
    pub(crate) fn new() -> Self {
        Self {
            dvms: Vec::new(),
            current_view: DvmView::Feed,
            scroll_offset: 0.0,
        }
    }

    /// Add or update a DVM from NIP-89 announcement
    pub(crate) fn add_dvm(&mut self, dvm: Dvm) {
        // Check if we already have this DVM (by pubkey)
        if let Some(existing) = self.dvms.iter_mut().find(|d| d.pubkey == dvm.pubkey) {
            // Update if newer
            if dvm.created_at > existing.created_at {
                *existing = dvm;
            }
        } else {
            self.dvms.push(dvm);
        }
        // Sort by name (or pubkey if no name)
        self.dvms.sort_by(|a, b| {
            a.display_name().to_lowercase().cmp(&b.display_name().to_lowercase())
        });
    }
}

/// Parse a NIP-89 DVM announcement (kind 31990)
fn parse_nip89_announcement(json: &str) -> Option<Dvm> {
    let parsed: serde_json::Value = serde_json::from_str(json).ok()?;
    let arr = parsed.as_array()?;

    if arr.len() < 3 {
        return None;
    }

    let msg_type = arr[0].as_str()?;
    if msg_type != "EVENT" {
        return None;
    }

    let event = &arr[2];
    let kind = event["kind"].as_u64()? as u16;

    // Must be kind 31990 (NIP-89 DVM announcement)
    if kind != 31990 {
        return None;
    }

    let id = event["id"].as_str()?.to_string();
    let pubkey = event["pubkey"].as_str()?.to_string();
    let created_at = event["created_at"].as_u64()?;
    let content = event["content"].as_str().unwrap_or("");
    let tags = event["tags"].as_array()?;

    // Extract supported kinds from "k" tags
    let mut supported_kinds = Vec::new();
    for tag in tags {
        if let Some(arr) = tag.as_array() {
            if arr.get(0).and_then(|v| v.as_str()) == Some("k") {
                if let Some(kind_str) = arr.get(1).and_then(|v| v.as_str()) {
                    if let Ok(k) = kind_str.parse::<u16>() {
                        supported_kinds.push(k);
                    }
                }
            }
        }
    }

    // Parse content as JSON for metadata (name, about, picture)
    let mut name = None;
    let mut about = None;
    let mut picture = None;

    if !content.is_empty() {
        if let Ok(meta) = serde_json::from_str::<serde_json::Value>(content) {
            name = meta["name"].as_str().map(|s| s.to_string());
            about = meta["about"].as_str().map(|s| s.to_string());
            picture = meta["picture"].as_str().map(|s| s.to_string());
        }
    }

    Some(Dvm {
        pubkey,
        name,
        about,
        picture,
        supported_kinds,
        nip89_event_id: id,
        created_at,
    })
}

/// Connect to a Nostr relay and subscribe to NIP-90 events, NIP-89 DVMs, and NIP-01 global feed
pub(crate) fn connect_to_relay(
    url: &str,
    on_nip90_event: impl Fn(Nip90Event) + 'static,
    on_dvm: impl Fn(Dvm) + 'static,
    on_text_note: impl Fn(TextNote) + 'static,
    on_author_meta: impl Fn(AuthorMeta) + 'static,
    on_status: impl Fn(RelayStatus) + 'static,
) -> Option<NostrRelayHandle> {
    on_status(RelayStatus::Connecting);

    let ws = web_sys::WebSocket::new(url).ok()?;

    // On open, subscribe to NIP-90, NIP-89, and global feed
    let ws_clone = ws.clone();
    let onopen = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::Event| {
        web_sys::console::log_1(&"Nostr relay connected".into());
        // Subscribe to NIP-90 job events
        let sub_msg = build_nip90_subscription();
        let _ = ws_clone.send_with_str(&sub_msg);
        // Subscribe to NIP-89 DVM announcements
        let dvm_sub_msg = build_nip89_subscription();
        let _ = ws_clone.send_with_str(&dvm_sub_msg);
        // Subscribe to NIP-01 global feed (kind:1 text notes)
        let global_sub_msg = build_global_feed_subscription();
        let _ = ws_clone.send_with_str(&global_sub_msg);
    });
    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    // On message, parse and emit events
    let on_status_clone = Rc::new(RefCell::new(Some(on_status)));
    let on_status_for_msg = on_status_clone.clone();
    let onmessage = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MessageEvent| {
        if let Some(data) = event.data().as_string() {
            // Check for EOSE (end of stored events) - means we're connected and caught up
            if data.contains("\"EOSE\"") {
                if let Some(ref cb) = *on_status_for_msg.borrow() {
                    cb(RelayStatus::Connected);
                }
                return;
            }

            // Try parsing as NIP-01 text note (kind:1)
            if let Some(note) = parse_text_note(&data) {
                if let Some(ref cb) = *on_status_for_msg.borrow() {
                    cb(RelayStatus::Connected);
                }
                on_text_note(note);
                return;
            }

            // Try parsing as author metadata (kind:0)
            if let Some(author) = parse_author_metadata(&data) {
                if let Some(ref cb) = *on_status_for_msg.borrow() {
                    cb(RelayStatus::Connected);
                }
                on_author_meta(author);
                return;
            }

            // Try parsing as NIP-90 event
            if let Some(nip90_event) = parse_nostr_event(&data) {
                if let Some(ref cb) = *on_status_for_msg.borrow() {
                    cb(RelayStatus::Connected);
                }
                on_nip90_event(nip90_event);
                return;
            }

            // Try parsing as NIP-89 DVM announcement
            if let Some(dvm) = parse_nip89_announcement(&data) {
                if let Some(ref cb) = *on_status_for_msg.borrow() {
                    cb(RelayStatus::Connected);
                }
                on_dvm(dvm);
            }
        }
    });
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    // On error
    let on_status_for_err = on_status_clone.clone();
    let onerror = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::ErrorEvent| {
        web_sys::console::log_1(&"Nostr relay error".into());
        if let Some(ref cb) = *on_status_for_err.borrow() {
            cb(RelayStatus::Error);
        }
    });
    ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    // On close
    let on_status_for_close = on_status_clone;
    let onclose = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::CloseEvent| {
        web_sys::console::log_1(&"Nostr relay disconnected".into());
        if let Some(ref cb) = *on_status_for_close.borrow() {
            cb(RelayStatus::Disconnected);
        }
    });
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();

    Some(NostrRelayHandle { ws })
}

// ============================================================================
// NIP-01 Global Feed Types
// ============================================================================

/// A NIP-01 text note (kind:1) for the global feed
#[derive(Clone, Debug)]
pub(crate) struct TextNote {
    pub(crate) id: String,
    pub(crate) pubkey: String,
    pub(crate) created_at: u64,
    pub(crate) content: String,
    pub(crate) is_reply: bool,
}

impl TextNote {
    /// Get truncated pubkey for display (first 8 chars of hex)
    pub(crate) fn short_pubkey(&self) -> String {
        if self.pubkey.len() > 8 {
            format!("{}...", &self.pubkey[..8])
        } else {
            self.pubkey.clone()
        }
    }

    /// Get relative time string (e.g. "2m ago", "1h ago")
    pub(crate) fn relative_time(&self, now_secs: u64) -> String {
        if self.created_at > now_secs {
            return "now".to_string();
        }
        let diff = now_secs - self.created_at;
        if diff < 60 {
            format!("{}s", diff)
        } else if diff < 3600 {
            format!("{}m", diff / 60)
        } else if diff < 86400 {
            format!("{}h", diff / 3600)
        } else {
            format!("{}d", diff / 86400)
        }
    }
}

/// Author metadata from kind:0
#[derive(Clone, Debug, Default)]
pub(crate) struct AuthorMeta {
    pub(crate) pubkey: String,
    pub(crate) name: Option<String>,
    pub(crate) display_name: Option<String>,
}

impl AuthorMeta {
    /// Get the best display name available
    pub(crate) fn best_name(&self) -> Option<&str> {
        self.display_name.as_deref().or(self.name.as_deref())
    }
}

/// State for the global feed pane
#[derive(Clone, Default)]
pub(crate) struct GlobalFeedState {
    pub(crate) notes: Vec<TextNote>,
    pub(crate) authors: std::collections::HashMap<String, AuthorMeta>,
    pub(crate) scroll_offset: f32,
    pub(crate) pending_metadata: std::collections::HashSet<String>,
}

impl GlobalFeedState {
    pub(crate) fn new() -> Self {
        Self {
            notes: Vec::new(),
            authors: std::collections::HashMap::new(),
            scroll_offset: 0.0,
            pending_metadata: std::collections::HashSet::new(),
        }
    }

    /// Add a note, keeping sorted by timestamp (most recent first), capped at max_notes
    /// Skips replies (notes with is_reply=true)
    pub(crate) fn add_note(&mut self, note: TextNote, max_notes: usize) {
        // Skip replies
        if note.is_reply {
            return;
        }

        // Avoid duplicates
        if self.notes.iter().any(|n| n.id == note.id) {
            return;
        }

        // Track pubkey for metadata fetching
        if !self.authors.contains_key(&note.pubkey) {
            self.pending_metadata.insert(note.pubkey.clone());
        }

        // Find insertion point to keep sorted by created_at (descending)
        let insert_idx = self.notes
            .iter()
            .position(|n| n.created_at < note.created_at)
            .unwrap_or(self.notes.len());

        self.notes.insert(insert_idx, note);
        self.notes.truncate(max_notes);
    }

    /// Add author metadata
    pub(crate) fn add_author(&mut self, author: AuthorMeta) {
        self.pending_metadata.remove(&author.pubkey);
        self.authors.insert(author.pubkey.clone(), author);
    }

    /// Get pubkeys that need metadata fetched
    pub(crate) fn get_pending_metadata(&mut self) -> Vec<String> {
        let pending: Vec<String> = self.pending_metadata.drain().collect();
        pending
    }
}

/// Parse a NIP-01 text note (kind:1) from relay JSON
pub(crate) fn parse_text_note(json: &str) -> Option<TextNote> {
    let parsed: serde_json::Value = serde_json::from_str(json).ok()?;
    let arr = parsed.as_array()?;

    if arr.len() < 3 {
        return None;
    }

    let msg_type = arr[0].as_str()?;
    if msg_type != "EVENT" {
        return None;
    }

    let event = &arr[2];
    let kind = event["kind"].as_u64()? as u16;

    // Must be kind 1 (text note)
    if kind != 1 {
        return None;
    }

    let id = event["id"].as_str()?.to_string();
    let pubkey = event["pubkey"].as_str()?.to_string();
    let created_at = event["created_at"].as_u64()?;
    let content = event["content"].as_str().unwrap_or("").to_string();
    let tags = event["tags"].as_array();

    // Check if this is a reply by looking for "e" tags
    // A note is a reply if it has an "e" tag (referencing another event)
    let is_reply = tags.map(|t| {
        t.iter().any(|tag| {
            if let Some(arr) = tag.as_array() {
                arr.first()
                    .and_then(|v| v.as_str())
                    .map(|s| s == "e")
                    .unwrap_or(false)
            } else {
                false
            }
        })
    }).unwrap_or(false);

    Some(TextNote {
        id,
        pubkey,
        created_at,
        content,
        is_reply,
    })
}

/// Parse author metadata (kind:0) from relay JSON
pub(crate) fn parse_author_metadata(json: &str) -> Option<AuthorMeta> {
    let parsed: serde_json::Value = serde_json::from_str(json).ok()?;
    let arr = parsed.as_array()?;

    if arr.len() < 3 {
        return None;
    }

    let msg_type = arr[0].as_str()?;
    if msg_type != "EVENT" {
        return None;
    }

    let event = &arr[2];
    let kind = event["kind"].as_u64()? as u16;

    // Must be kind 0 (metadata)
    if kind != 0 {
        return None;
    }

    let pubkey = event["pubkey"].as_str()?.to_string();
    let content = event["content"].as_str().unwrap_or("");

    // Parse content as JSON for metadata
    let mut name = None;
    let mut display_name = None;

    if !content.is_empty() {
        if let Ok(meta) = serde_json::from_str::<serde_json::Value>(content) {
            name = meta["name"].as_str().map(|s| s.to_string());
            display_name = meta["display_name"].as_str().map(|s| s.to_string());
        }
    }

    Some(AuthorMeta {
        pubkey,
        name,
        display_name,
    })
}

/// Build subscription for NIP-01 global feed (kind:1 text notes)
pub(crate) fn build_global_feed_subscription() -> String {
    serde_json::json!([
        "REQ",
        "global-notes",
        {
            "kinds": [1],
            "limit": 50
        }
    ]).to_string()
}

/// Build subscription for author metadata (kind:0)
pub(crate) fn build_metadata_subscription(pubkeys: &[String]) -> String {
    serde_json::json!([
        "REQ",
        "metadata-sub",
        {
            "kinds": [0],
            "authors": pubkeys
        }
    ]).to_string()
}

/// Default relay URLs to connect to
pub(crate) const DEFAULT_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://nos.lol",
];
