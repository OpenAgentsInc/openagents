/// Kind range for job requests
pub const JOB_REQUEST_KIND_MIN: u16 = 5000;
pub const JOB_REQUEST_KIND_MAX: u16 = 5999;

/// Kind range for job results (request kind + 1000)
pub const JOB_RESULT_KIND_MIN: u16 = 6000;
pub const JOB_RESULT_KIND_MAX: u16 = 6999;

/// Kind for job feedback
pub const KIND_JOB_FEEDBACK: u16 = 7000;

// Common job request kinds (from DVM spec)
/// Text extraction / OCR
pub const KIND_JOB_TEXT_EXTRACTION: u16 = 5000;
/// Summarization
pub const KIND_JOB_SUMMARIZATION: u16 = 5001;
/// Translation
pub const KIND_JOB_TRANSLATION: u16 = 5002;
/// Text generation / Chat
pub const KIND_JOB_TEXT_GENERATION: u16 = 5050;
/// Image generation
pub const KIND_JOB_IMAGE_GENERATION: u16 = 5100;
/// Speech to text
pub const KIND_JOB_SPEECH_TO_TEXT: u16 = 5250;

// OpenAgents compute job kinds (Bazaar)
/// Sandbox run - execute commands in isolated container
pub const KIND_JOB_SANDBOX_RUN: u16 = 5930;
/// Repository index - generate embeddings/symbols for a git repo
pub const KIND_JOB_REPO_INDEX: u16 = 5931;
/// Patch generation - generate code patches from issue descriptions
pub const KIND_JOB_PATCH_GEN: u16 = 5932;
/// Code review - review code changes and provide feedback
pub const KIND_JOB_CODE_REVIEW: u16 = 5933;

// RLM (Recursive Language Models) job kinds
/// RLM sub-query - a sub-task in a recursive language model execution
/// Result kind: 6940
pub const KIND_JOB_RLM_SUBQUERY: u16 = 5940;
/// RLM sub-query result
pub const KIND_RESULT_RLM_SUBQUERY: u16 = 6940;
/// Check if a kind is a job request kind (5000-5999).
pub fn is_job_request_kind(kind: u16) -> bool {
    (JOB_REQUEST_KIND_MIN..=JOB_REQUEST_KIND_MAX).contains(&kind)
}

/// Check if a kind is a job result kind (6000-6999).
pub fn is_job_result_kind(kind: u16) -> bool {
    (JOB_RESULT_KIND_MIN..=JOB_RESULT_KIND_MAX).contains(&kind)
}

/// Check if a kind is a job feedback kind (7000).
pub fn is_job_feedback_kind(kind: u16) -> bool {
    kind == KIND_JOB_FEEDBACK
}

/// Check if a kind is any DVM-related kind (5000-7000).
pub fn is_dvm_kind(kind: u16) -> bool {
    is_job_request_kind(kind) || is_job_result_kind(kind) || is_job_feedback_kind(kind)
}

/// Get the result kind for a given request kind.
pub fn get_result_kind(request_kind: u16) -> Option<u16> {
    if is_job_request_kind(request_kind) {
        Some(request_kind + 1000)
    } else {
        None
    }
}

/// Get the request kind for a given result kind.
pub fn get_request_kind(result_kind: u16) -> Option<u16> {
    if is_job_result_kind(result_kind) {
        Some(result_kind - 1000)
    } else {
        None
    }
}
