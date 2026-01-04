//! NIP-90: Data Vending Machine (DVM)
//!
//! Job request and result kinds for decentralized compute.
//! Pylon desktops subscribe to job requests and publish results.

/// Job request kind range (5000-5999)
pub const JOB_REQUEST_KIND_MIN: u16 = 5000;
pub const JOB_REQUEST_KIND_MAX: u16 = 5999;

/// Job result kind range (6000-6999)
pub const JOB_RESULT_KIND_MIN: u16 = 6000;
pub const JOB_RESULT_KIND_MAX: u16 = 6999;

/// Job feedback kind
pub const KIND_JOB_FEEDBACK: u16 = 7000;

/// Common job kinds
pub const KIND_TEXT_GENERATION: u16 = 5050;
pub const KIND_TEXT_GENERATION_RESULT: u16 = 6050;

/// Check if a kind is a job request kind
pub fn is_job_request_kind(kind: u16) -> bool {
    kind >= JOB_REQUEST_KIND_MIN && kind <= JOB_REQUEST_KIND_MAX
}

/// Check if a kind is a job result kind
pub fn is_job_result_kind(kind: u16) -> bool {
    kind >= JOB_RESULT_KIND_MIN && kind <= JOB_RESULT_KIND_MAX
}

/// Check if a kind is job feedback
pub fn is_job_feedback_kind(kind: u16) -> bool {
    kind == KIND_JOB_FEEDBACK
}

/// Check if a kind is DVM-related
pub fn is_dvm_kind(kind: u16) -> bool {
    is_job_request_kind(kind) || is_job_result_kind(kind) || is_job_feedback_kind(kind)
}

/// Get the corresponding result kind for a request kind
pub fn get_result_kind(request_kind: u16) -> u16 {
    request_kind + 1000
}

/// Get the corresponding request kind for a result kind
pub fn get_request_kind(result_kind: u16) -> u16 {
    result_kind - 1000
}

/// Extract job request event ID from a job result
pub fn get_request_event_id(event: &nostr::Event) -> Option<String> {
    if !is_job_result_kind(event.kind) {
        return None;
    }

    // Look for "e" tag pointing to request
    for tag in &event.tags {
        if tag.len() >= 2 && tag[0] == "e" {
            return Some(tag[1].clone());
        }
    }

    // Also check "request" tag
    for tag in &event.tags {
        if tag.len() >= 2 && tag[0] == "request" {
            return Some(tag[1].clone());
        }
    }

    None
}

/// Extract job status from a job result or feedback
pub fn get_job_status(event: &nostr::Event) -> Option<String> {
    for tag in &event.tags {
        if tag.len() >= 2 && tag[0] == "status" {
            return Some(tag[1].clone());
        }
    }
    None
}

/// Extract requester pubkey from a job result
pub fn get_requester_pubkey(event: &nostr::Event) -> Option<String> {
    for tag in &event.tags {
        if tag.len() >= 2 && tag[0] == "p" {
            return Some(tag[1].clone());
        }
    }
    None
}
