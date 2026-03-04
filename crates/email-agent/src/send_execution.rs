use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SendFailureClass {
    Transient,
    Permanent,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SendProviderError {
    pub class: SendFailureClass,
    pub reason: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GmailSendSuccess {
    pub provider_message_id: String,
}

pub trait GmailSendProvider {
    fn send_message(&self, request: &SendRequest) -> Result<GmailSendSuccess, SendProviderError>;
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SendRequest {
    pub draft_id: String,
    pub idempotency_key: String,
    pub recipient_email: String,
    pub subject: String,
    pub body: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SendExecutionPolicy {
    pub max_attempts: u32,
    pub transient_retry_backoff_seconds: u64,
}

impl Default for SendExecutionPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            transient_retry_backoff_seconds: 30,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SendDeliveryState {
    Pending,
    RetryScheduled,
    Sent,
    FailedPermanent,
    FailedTransientExhausted,
}

impl SendDeliveryState {
    pub const fn is_final(self) -> bool {
        matches!(
            self,
            Self::Sent | Self::FailedPermanent | Self::FailedTransientExhausted
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SendRecord {
    pub send_id: String,
    pub draft_id: String,
    pub idempotency_key: String,
    pub request_fingerprint: String,
    pub state: SendDeliveryState,
    pub attempt_count: u32,
    pub provider_message_id: Option<String>,
    pub last_error: Option<String>,
    pub next_retry_at_unix: Option<u64>,
    pub finalized_at_unix: Option<u64>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SendAuditEventType {
    Queued,
    Attempted,
    Sent,
    RetryScheduled,
    FailedPermanent,
    FailedTransientExhausted,
    DedupeHit,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SendAuditRecord {
    pub event_id: String,
    pub send_id: String,
    pub at_unix: u64,
    pub event_type: SendAuditEventType,
    pub detail: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct SendExecutionState {
    pub records_by_idempotency_key: BTreeMap<String, SendRecord>,
    pub audit_log: Vec<SendAuditRecord>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SendExecutionOutcome {
    pub send_id: String,
    pub idempotency_key: String,
    pub state: SendDeliveryState,
    pub attempt_count: u32,
    pub provider_message_id: Option<String>,
    pub next_retry_at_unix: Option<u64>,
    pub dedupe_hit: bool,
    pub final_state: bool,
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum SendExecutionError {
    #[error("invalid send config: {0}")]
    InvalidConfig(String),
    #[error("invalid send request: {0}")]
    InvalidRequest(String),
    #[error("idempotency conflict: {0}")]
    DedupeConflict(String),
}

pub fn execute_send_with_idempotency(
    state: &mut SendExecutionState,
    provider: &dyn GmailSendProvider,
    request: &SendRequest,
    policy: &SendExecutionPolicy,
    now_unix: u64,
) -> Result<SendExecutionOutcome, SendExecutionError> {
    validate_send_inputs(request, policy)?;

    let fingerprint = request_fingerprint(request);
    let mut dedupe_hit = false;
    let mut record = if let Some(existing) = state
        .records_by_idempotency_key
        .remove(request.idempotency_key.as_str())
    {
        dedupe_hit = true;
        if existing.request_fingerprint != fingerprint {
            state
                .records_by_idempotency_key
                .insert(existing.idempotency_key.clone(), existing);
            return Err(SendExecutionError::DedupeConflict(format!(
                "idempotency_key {} already mapped to different payload fingerprint",
                request.idempotency_key
            )));
        }
        existing
    } else {
        let record = SendRecord {
            send_id: next_send_id(state),
            draft_id: request.draft_id.clone(),
            idempotency_key: request.idempotency_key.clone(),
            request_fingerprint: fingerprint,
            state: SendDeliveryState::Pending,
            attempt_count: 0,
            provider_message_id: None,
            last_error: None,
            next_retry_at_unix: None,
            finalized_at_unix: None,
        };
        push_audit_event(
            state,
            record.send_id.as_str(),
            now_unix,
            SendAuditEventType::Queued,
            format!("queued draft {}", record.draft_id),
        );
        record
    };

    if dedupe_hit {
        push_audit_event(
            state,
            record.send_id.as_str(),
            now_unix,
            SendAuditEventType::DedupeHit,
            "reused existing idempotency key".to_string(),
        );
    }

    if record.state.is_final() {
        let outcome = outcome_from_record(&record, dedupe_hit);
        state
            .records_by_idempotency_key
            .insert(record.idempotency_key.clone(), record);
        return Ok(outcome);
    }

    if record.state == SendDeliveryState::RetryScheduled
        && record
            .next_retry_at_unix
            .is_some_and(|next_retry| now_unix < next_retry)
    {
        let outcome = outcome_from_record(&record, dedupe_hit);
        state
            .records_by_idempotency_key
            .insert(record.idempotency_key.clone(), record);
        return Ok(outcome);
    }

    record.attempt_count = record.attempt_count.saturating_add(1);
    record.next_retry_at_unix = None;
    push_audit_event(
        state,
        record.send_id.as_str(),
        now_unix,
        SendAuditEventType::Attempted,
        format!("attempt {}", record.attempt_count),
    );

    match provider.send_message(request) {
        Ok(success) => {
            record.state = SendDeliveryState::Sent;
            record.provider_message_id = Some(success.provider_message_id.clone());
            record.last_error = None;
            record.finalized_at_unix = Some(now_unix);
            push_audit_event(
                state,
                record.send_id.as_str(),
                now_unix,
                SendAuditEventType::Sent,
                format!("provider_message_id={}", success.provider_message_id),
            );
        }
        Err(failure) => match failure.class {
            SendFailureClass::Permanent => {
                record.state = SendDeliveryState::FailedPermanent;
                record.last_error = Some(failure.reason.clone());
                record.finalized_at_unix = Some(now_unix);
                push_audit_event(
                    state,
                    record.send_id.as_str(),
                    now_unix,
                    SendAuditEventType::FailedPermanent,
                    failure.reason,
                );
            }
            SendFailureClass::Transient => {
                record.last_error = Some(failure.reason.clone());
                if record.attempt_count >= policy.max_attempts {
                    record.state = SendDeliveryState::FailedTransientExhausted;
                    record.finalized_at_unix = Some(now_unix);
                    push_audit_event(
                        state,
                        record.send_id.as_str(),
                        now_unix,
                        SendAuditEventType::FailedTransientExhausted,
                        failure.reason,
                    );
                } else {
                    let retry_at = now_unix.saturating_add(
                        policy
                            .transient_retry_backoff_seconds
                            .saturating_mul(record.attempt_count as u64),
                    );
                    record.state = SendDeliveryState::RetryScheduled;
                    record.next_retry_at_unix = Some(retry_at);
                    push_audit_event(
                        state,
                        record.send_id.as_str(),
                        now_unix,
                        SendAuditEventType::RetryScheduled,
                        format!("retry_at={retry_at} reason={}", failure.reason),
                    );
                }
            }
        },
    }

    let outcome = outcome_from_record(&record, dedupe_hit);
    state
        .records_by_idempotency_key
        .insert(record.idempotency_key.clone(), record);
    Ok(outcome)
}

fn validate_send_inputs(
    request: &SendRequest,
    policy: &SendExecutionPolicy,
) -> Result<(), SendExecutionError> {
    if policy.max_attempts == 0 {
        return Err(SendExecutionError::InvalidConfig(
            "max_attempts must be greater than zero".to_string(),
        ));
    }
    if policy.transient_retry_backoff_seconds == 0 {
        return Err(SendExecutionError::InvalidConfig(
            "transient_retry_backoff_seconds must be greater than zero".to_string(),
        ));
    }
    if request.draft_id.trim().is_empty() {
        return Err(SendExecutionError::InvalidRequest(
            "draft_id must not be empty".to_string(),
        ));
    }
    if request.idempotency_key.trim().is_empty() {
        return Err(SendExecutionError::InvalidRequest(
            "idempotency_key must not be empty".to_string(),
        ));
    }
    if request.recipient_email.trim().is_empty() {
        return Err(SendExecutionError::InvalidRequest(
            "recipient_email must not be empty".to_string(),
        ));
    }
    if request.subject.trim().is_empty() {
        return Err(SendExecutionError::InvalidRequest(
            "subject must not be empty".to_string(),
        ));
    }
    if request.body.trim().is_empty() {
        return Err(SendExecutionError::InvalidRequest(
            "body must not be empty".to_string(),
        ));
    }
    Ok(())
}

fn request_fingerprint(request: &SendRequest) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in format!(
        "{}|{}|{}|{}",
        request.draft_id, request.recipient_email, request.subject, request.body
    )
    .as_bytes()
    {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    format!("{hash:016x}")
}

fn next_send_id(state: &SendExecutionState) -> String {
    format!(
        "send-{:05}",
        state
            .records_by_idempotency_key
            .len()
            .saturating_add(1)
    )
}

fn next_audit_event_id(state: &SendExecutionState) -> String {
    format!("send-audit-{:05}", state.audit_log.len().saturating_add(1))
}

fn push_audit_event(
    state: &mut SendExecutionState,
    send_id: &str,
    at_unix: u64,
    event_type: SendAuditEventType,
    detail: String,
) {
    state.audit_log.push(SendAuditRecord {
        event_id: next_audit_event_id(state),
        send_id: send_id.to_string(),
        at_unix,
        event_type,
        detail,
    });
}

fn outcome_from_record(record: &SendRecord, dedupe_hit: bool) -> SendExecutionOutcome {
    SendExecutionOutcome {
        send_id: record.send_id.clone(),
        idempotency_key: record.idempotency_key.clone(),
        state: record.state,
        attempt_count: record.attempt_count,
        provider_message_id: record.provider_message_id.clone(),
        next_retry_at_unix: record.next_retry_at_unix,
        dedupe_hit,
        final_state: record.state.is_final(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        GmailSendProvider, GmailSendSuccess, SendDeliveryState, SendExecutionError,
        SendExecutionPolicy, SendExecutionState, SendFailureClass, SendProviderError, SendRequest,
        execute_send_with_idempotency,
    };
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct MockSendProvider {
        responses: Mutex<VecDeque<Result<GmailSendSuccess, SendProviderError>>>,
        call_count: Mutex<u32>,
    }

    impl MockSendProvider {
        fn new(responses: Vec<Result<GmailSendSuccess, SendProviderError>>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
                call_count: Mutex::new(0),
            }
        }

        fn calls(&self) -> u32 {
            *self.call_count.lock().expect("call counter lock")
        }
    }

    impl GmailSendProvider for MockSendProvider {
        fn send_message(
            &self,
            _request: &SendRequest,
        ) -> Result<GmailSendSuccess, SendProviderError> {
            let mut call_count = self.call_count.lock().expect("call counter lock");
            *call_count = call_count.saturating_add(1);
            let mut responses = self.responses.lock().expect("responses lock");
            responses.pop_front().unwrap_or_else(|| {
                Ok(GmailSendSuccess {
                    provider_message_id: "provider:default".to_string(),
                })
            })
        }
    }

    fn request_with_key(idempotency_key: &str) -> SendRequest {
        SendRequest {
            draft_id: "draft-1".to_string(),
            idempotency_key: idempotency_key.to_string(),
            recipient_email: "ops@example.com".to_string(),
            subject: "Status".to_string(),
            body: "Thanks for the update.".to_string(),
        }
    }

    #[test]
    fn idempotent_success_only_calls_provider_once() {
        let provider = MockSendProvider::new(vec![Ok(GmailSendSuccess {
            provider_message_id: "gmail:abc".to_string(),
        })]);
        let mut state = SendExecutionState::default();
        let request = request_with_key("dedupe-1");

        let first = execute_send_with_idempotency(
            &mut state,
            &provider,
            &request,
            &SendExecutionPolicy::default(),
            100,
        )
        .expect("first send should succeed");
        assert_eq!(first.state, SendDeliveryState::Sent);
        assert_eq!(provider.calls(), 1);

        let second = execute_send_with_idempotency(
            &mut state,
            &provider,
            &request,
            &SendExecutionPolicy::default(),
            101,
        )
        .expect("second send should dedupe");
        assert!(second.dedupe_hit);
        assert_eq!(second.state, SendDeliveryState::Sent);
        assert_eq!(provider.calls(), 1);
    }

    #[test]
    fn transient_failure_retries_then_succeeds() {
        let provider = MockSendProvider::new(vec![
            Err(SendProviderError {
                class: SendFailureClass::Transient,
                reason: "rate_limited".to_string(),
            }),
            Ok(GmailSendSuccess {
                provider_message_id: "gmail:retry-success".to_string(),
            }),
        ]);
        let mut state = SendExecutionState::default();
        let request = request_with_key("dedupe-2");
        let policy = SendExecutionPolicy {
            max_attempts: 3,
            transient_retry_backoff_seconds: 30,
        };

        let first =
            execute_send_with_idempotency(&mut state, &provider, &request, &policy, 200).unwrap();
        assert_eq!(first.state, SendDeliveryState::RetryScheduled);
        assert_eq!(first.attempt_count, 1);
        assert_eq!(provider.calls(), 1);
        assert_eq!(first.next_retry_at_unix, Some(230));

        let skipped =
            execute_send_with_idempotency(&mut state, &provider, &request, &policy, 220).unwrap();
        assert_eq!(skipped.state, SendDeliveryState::RetryScheduled);
        assert_eq!(skipped.attempt_count, 1);
        assert_eq!(provider.calls(), 1);

        let third =
            execute_send_with_idempotency(&mut state, &provider, &request, &policy, 231).unwrap();
        assert_eq!(third.state, SendDeliveryState::Sent);
        assert_eq!(third.attempt_count, 2);
        assert_eq!(provider.calls(), 2);
        assert_eq!(
            third.provider_message_id.as_deref(),
            Some("gmail:retry-success")
        );
    }

    #[test]
    fn permanent_failure_is_final_and_deduped() {
        let provider = MockSendProvider::new(vec![Err(SendProviderError {
            class: SendFailureClass::Permanent,
            reason: "invalid_recipient".to_string(),
        })]);
        let mut state = SendExecutionState::default();
        let request = request_with_key("dedupe-3");

        let first = execute_send_with_idempotency(
            &mut state,
            &provider,
            &request,
            &SendExecutionPolicy::default(),
            300,
        )
        .expect("first send should execute");
        assert_eq!(first.state, SendDeliveryState::FailedPermanent);
        assert_eq!(provider.calls(), 1);

        let second = execute_send_with_idempotency(
            &mut state,
            &provider,
            &request,
            &SendExecutionPolicy::default(),
            301,
        )
        .expect("second call should dedupe final record");
        assert_eq!(second.state, SendDeliveryState::FailedPermanent);
        assert!(second.dedupe_hit);
        assert_eq!(provider.calls(), 1);
    }

    #[test]
    fn conflicting_payload_for_same_idempotency_key_errors() {
        let provider = MockSendProvider::new(vec![Ok(GmailSendSuccess {
            provider_message_id: "gmail:ok".to_string(),
        })]);
        let mut state = SendExecutionState::default();
        let request = request_with_key("dedupe-4");
        execute_send_with_idempotency(
            &mut state,
            &provider,
            &request,
            &SendExecutionPolicy::default(),
            400,
        )
        .expect("first send should succeed");

        let mut changed = request.clone();
        changed.body = "A different payload".to_string();
        let error = execute_send_with_idempotency(
            &mut state,
            &provider,
            &changed,
            &SendExecutionPolicy::default(),
            401,
        )
        .expect_err("payload mismatch should fail");
        assert_eq!(
            error,
            SendExecutionError::DedupeConflict(
                "idempotency_key dedupe-4 already mapped to different payload fingerprint"
                    .to_string()
            )
        );
    }
}
