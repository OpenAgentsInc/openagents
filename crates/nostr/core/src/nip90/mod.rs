//! NIP-90: Data Vending Machines
//!
//! Data Vending Machines (DVMs) enable on-demand computation over Nostr. Customers submit job
//! requests to service providers who perform computation and return results. Payment flows via
//! Lightning (bolt11 invoices) or Nostr Zaps.
//!
//! Internal module boundaries:
//! - `kinds`: event kind constants + kind classification helpers
//! - `model`: request/result/feedback data models + tag parsing/serialization
//! - `builders`: `EventTemplate` construction helpers
//! - `tests`: protocol and workflow coverage
//!
//! **Money in, data out.**
//!
//! ## Protocol Overview
//!
//! DVMs follow a simple request-response pattern:
//!
//! ```text
//! Customer                Service Provider
//!    │                           │
//!    │──── Job Request ──────────>│
//!    │      (kind 5000-5999)      │
//!    │                            │
//!    │<─── Job Feedback ──────────│ (optional)
//!    │      (kind 7000)           │
//!    │                            │
//!    │<─── Job Result ────────────│
//!    │      (kind 6000-6999)      │
//!    │                            │
//!    │──── Payment ───────────────>│
//!    │   (bolt11 or zap)          │
//! ```
//!
//! ## Event Kinds
//!
//! - **5000-5999**: Job request kinds
//!   - 5000: Text extraction/OCR
//!   - 5001: Summarization
//!   - 5002: Translation
//!   - 5050: Text generation (LLM inference)
//!   - 5100: Image generation
//!   - 5250: Speech-to-text
//!
//! - **6000-6999**: Job result kinds (= request kind + 1000)
//!   - Result kind is automatically calculated from request kind
//!
//! - **7000**: Job feedback (status updates, payment requests)
//!
//! ## Creating a Job Request
//!
//! ```rust
//! use nostr::nip90::{JobRequest, JobInput, KIND_JOB_TEXT_GENERATION};
//!
//! let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)?
//!     .add_input(JobInput::text("Write a haiku about Nostr"))
//!     .add_param("temperature", "0.7")
//!     .add_param("max_tokens", "100")
//!     .with_bid(1000)  // millisats
//!     .add_relay("wss://relay.damus.io");
//!
//! // Convert to event tags and publish
//! let tags = request.to_tags();
//! # Ok::<(), nostr::nip90::Nip90Error>(())
//! ```
//!
//! ## Creating a Job Result
//!
//! ```rust
//! use nostr::nip90::{JobResult, KIND_JOB_TEXT_GENERATION};
//!
//! let result = JobResult::new(
//!     KIND_JOB_TEXT_GENERATION,
//!     "request_event_id",
//!     "customer_pubkey",
//!     "Nostr flows free,\nDecentralized thoughts connect,\nSovereign and true.",
//! )?
//! .with_amount(1000, Some("lnbc1000n...".to_string()));
//!
//! // Convert to event tags and publish
//! let tags = result.to_tags();
//! # Ok::<(), nostr::nip90::Nip90Error>(())
//! ```
//!
//! ## Input Types
//!
//! DVMs support four input types via the `i` tag:
//!
//! - **text**: Direct text input
//! - **url**: URL to fetch data from
//! - **event**: Nostr event ID (with optional relay hint)
//! - **job**: Output from another job (chaining)
//!
//! ```rust
//! use nostr::nip90::JobInput;
//!
//! // Direct text
//! let input = JobInput::text("Translate this");
//!
//! // URL with marker
//! let input = JobInput::url("https://example.com/doc.txt")
//!     .with_marker("source");
//!
//! // Event reference
//! let input = JobInput::event("event_id", Some("wss://relay.com".to_string()));
//!
//! // Chain from another job
//! let input = JobInput::job("previous_job_id", None);
//! ```
//!
//! ## Parameters
//!
//! Model-specific parameters are passed via `param` tags:
//!
//! ```rust
//! use nostr::nip90::JobParam;
//!
//! let param = JobParam::new("temperature", "0.7");
//! let param = JobParam::new("max_tokens", "2048");
//! let param = JobParam::new("model", "llama3.2");
//! ```
//!
//! ## Payment Flow
//!
//! 1. Customer includes `bid` tag in job request (optional)
//! 2. Provider MAY send feedback (kind 7000) with `payment-required` status
//! 3. Provider publishes result (kind 6000-6999) with `amount` and `bolt11` tags
//! 4. Customer pays bolt11 invoice or zaps the result event
//!
//! ## Service Provider Discovery
//!
//! Providers advertise their capabilities via NIP-89 application handler events.
//! Customers discover providers by querying relays for handlers of specific job kinds.
//!
//! See the [NIP-90 specification](https://github.com/nostr-protocol/nips/blob/master/90.md)
//! for complete details.

mod builders;
mod kinds;
mod model;

pub use builders::{create_job_feedback_event, create_job_request_event, create_job_result_event};
pub use kinds::{
    JOB_REQUEST_KIND_MAX, JOB_REQUEST_KIND_MIN, JOB_RESULT_KIND_MAX, JOB_RESULT_KIND_MIN,
    KIND_JOB_CODE_REVIEW, KIND_JOB_FEEDBACK, KIND_JOB_IMAGE_GENERATION, KIND_JOB_PATCH_GEN,
    KIND_JOB_REPO_INDEX, KIND_JOB_RLM_SUBQUERY, KIND_JOB_SANDBOX_RUN, KIND_JOB_SPEECH_TO_TEXT,
    KIND_JOB_SUMMARIZATION, KIND_JOB_TEXT_EXTRACTION, KIND_JOB_TEXT_GENERATION,
    KIND_JOB_TRANSLATION, KIND_RESULT_RLM_SUBQUERY, get_request_kind, get_result_kind, is_dvm_kind,
    is_job_feedback_kind, is_job_request_kind, is_job_result_kind,
};
pub use model::{
    InputType, JobFeedback, JobInput, JobParam, JobRequest, JobResult, JobStatus, Nip90Error,
};

#[cfg(test)]
mod tests;
