use super::{JobFeedback, JobRequest, JobResult, KIND_JOB_FEEDBACK};

fn event_timestamp_now() -> u64 {
    crate::nip01::unix_now_secs().map_or(0, |timestamp| timestamp)
}

/// Create an EventTemplate for a job request.
///
/// This is a convenience function that converts a JobRequest into an EventTemplate
/// ready to be signed and published. The content field comes from the request's
/// content (empty for non-encrypted requests).
///
/// # Example
///
/// ```
/// use nostr::nip90::{JobRequest, JobInput, create_job_request_event, KIND_JOB_TEXT_GENERATION};
///
/// # fn example() -> Result<(), nostr::nip90::Nip90Error> {
/// let request = JobRequest::new(KIND_JOB_TEXT_GENERATION)?
///     .add_input(JobInput::text("Write a haiku about Nostr"))
///     .with_bid(1000);
///
/// let event_template = create_job_request_event(&request);
/// assert_eq!(event_template.kind, KIND_JOB_TEXT_GENERATION);
/// # Ok(())
/// # }
/// ```
pub fn create_job_request_event(request: &JobRequest) -> crate::nip01::EventTemplate {
    crate::nip01::EventTemplate {
        kind: request.kind,
        tags: request.to_tags(),
        content: request.content.clone(),
        created_at: event_timestamp_now(),
    }
}

/// Create an EventTemplate for a job result.
///
/// This converts a JobResult into an EventTemplate ready to be signed and published.
/// The result kind is automatically calculated from the request kind.
///
/// # Example
///
/// ```
/// use nostr::nip90::{JobResult, create_job_result_event, KIND_JOB_TEXT_GENERATION};
///
/// # fn example() -> Result<(), nostr::nip90::Nip90Error> {
/// let result = JobResult::new(
///     KIND_JOB_TEXT_GENERATION,
///     "request_event_id",
///     "customer_pubkey",
///     "Nostr flows free,\nDecentralized thoughts connect,\nSovereign and true.",
/// )?;
///
/// let event_template = create_job_result_event(&result);
/// assert_eq!(event_template.kind, KIND_JOB_TEXT_GENERATION + 1000);
/// # Ok(())
/// # }
/// ```
pub fn create_job_result_event(result: &JobResult) -> crate::nip01::EventTemplate {
    crate::nip01::EventTemplate {
        kind: result.kind,
        tags: result.to_tags(),
        content: result.content.clone(),
        created_at: event_timestamp_now(),
    }
}

/// Create an EventTemplate for job feedback.
///
/// This converts a JobFeedback into an EventTemplate ready to be signed and published.
/// Feedback events use kind 7000.
///
/// # Example
///
/// ```
/// use nostr::nip90::{JobFeedback, JobStatus, create_job_feedback_event};
///
/// # fn example() -> Result<(), nostr::nip90::Nip90Error> {
/// let feedback = JobFeedback::new("request_event_id", "customer_pubkey", JobStatus::Processing)?;
///
/// let event_template = create_job_feedback_event(&feedback);
/// assert_eq!(event_template.kind, 7000);
/// # Ok(())
/// # }
/// ```
pub fn create_job_feedback_event(feedback: &JobFeedback) -> crate::nip01::EventTemplate {
    crate::nip01::EventTemplate {
        kind: KIND_JOB_FEEDBACK,
        tags: feedback.to_tags(),
        content: feedback.content.clone(),
        created_at: event_timestamp_now(),
    }
}
