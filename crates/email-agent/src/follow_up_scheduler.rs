use std::collections::BTreeMap;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum FollowUpRuleKind {
    NoReplyAfterDays { days_without_reply: u32 },
    UnansweredCriticalThread { hours_without_reply: u32 },
    ReminderCadence {
        cadence_days: u32,
        max_reminders: u32,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FollowUpRule {
    pub rule_id: String,
    pub enabled: bool,
    pub kind: FollowUpRuleKind,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FollowUpSchedulerPolicy {
    pub rules: Vec<FollowUpRule>,
    pub business_hours_start_utc: u8,
    pub business_hours_end_utc: u8,
    pub quiet_hours_start_utc: u8,
    pub quiet_hours_end_utc: u8,
    pub per_recipient_daily_limit: u32,
}

impl Default for FollowUpSchedulerPolicy {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            business_hours_start_utc: 9,
            business_hours_end_utc: 17,
            quiet_hours_start_utc: 21,
            quiet_hours_end_utc: 7,
            per_recipient_daily_limit: 3,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ThreadFollowUpContext {
    pub thread_id: String,
    pub recipient_email: String,
    pub last_inbound_unix: u64,
    pub last_outbound_unix: u64,
    pub awaiting_reply: bool,
    pub is_critical: bool,
    pub reminder_count: u32,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FollowUpJobStatus {
    Scheduled,
    Executed,
    SkippedRecipientLimit,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FollowUpJob {
    pub job_id: String,
    pub thread_id: String,
    pub recipient_email: String,
    pub rule_id: String,
    pub scheduled_for_unix: u64,
    pub status: FollowUpJobStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FollowUpEventType {
    UpcomingScheduled,
    DeferredByWindow,
    Executed,
    SkippedRecipientLimit,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FollowUpEvent {
    pub event_id: String,
    pub job_id: String,
    pub thread_id: String,
    pub recipient_email: String,
    pub at_unix: u64,
    pub event_type: FollowUpEventType,
    pub detail: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct FollowUpSchedulerState {
    pub jobs: BTreeMap<String, FollowUpJob>,
    pub event_log: Vec<FollowUpEvent>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FollowUpTickOutcome {
    pub upcoming_jobs: Vec<FollowUpJob>,
    pub executed_jobs: Vec<FollowUpJob>,
    pub emitted_events: Vec<FollowUpEvent>,
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum FollowUpSchedulerError {
    #[error("invalid follow-up policy: {0}")]
    InvalidPolicy(String),
}

pub fn run_follow_up_scheduler_tick(
    state: &mut FollowUpSchedulerState,
    contexts: &[ThreadFollowUpContext],
    policy: &FollowUpSchedulerPolicy,
    now_unix: u64,
) -> Result<FollowUpTickOutcome, FollowUpSchedulerError> {
    validate_policy(policy)?;

    let mut upcoming_jobs = Vec::<FollowUpJob>::new();
    let mut executed_jobs = Vec::<FollowUpJob>::new();
    let mut emitted_events = Vec::<FollowUpEvent>::new();

    let mut contexts_sorted = contexts.to_vec();
    contexts_sorted.sort_by(|left, right| {
        left.thread_id
            .cmp(&right.thread_id)
            .then_with(|| left.recipient_email.cmp(&right.recipient_email))
    });

    let mut rules_sorted = policy
        .rules
        .iter()
        .filter(|rule| rule.enabled)
        .cloned()
        .collect::<Vec<FollowUpRule>>();
    rules_sorted.sort_by(|left, right| left.rule_id.cmp(&right.rule_id));

    for context in contexts_sorted {
        for rule in &rules_sorted {
            if has_open_job_for(state, context.thread_id.as_str(), rule.rule_id.as_str()) {
                continue;
            }

            let Some(raw_due_at) = rule_due_at(&context, rule) else {
                continue;
            };
            let adjusted_due_at = next_allowed_send_time(raw_due_at, policy);
            let mut reason = None;
            if adjusted_due_at > raw_due_at {
                reason = Some("deferred_to_allowed_send_window".to_string());
            }

            let day_index = adjusted_due_at / 86_400;
            if recipient_follow_up_count_for_day(
                state,
                context.recipient_email.as_str(),
                day_index,
            ) >= policy.per_recipient_daily_limit
            {
                let skipped = FollowUpJob {
                    job_id: next_job_id(state),
                    thread_id: context.thread_id.clone(),
                    recipient_email: context.recipient_email.clone(),
                    rule_id: rule.rule_id.clone(),
                    scheduled_for_unix: adjusted_due_at,
                    status: FollowUpJobStatus::SkippedRecipientLimit,
                    reason: Some("recipient_daily_limit_reached".to_string()),
                };
                state.jobs.insert(skipped.job_id.clone(), skipped.clone());
                let event = push_event(
                    state,
                    skipped.job_id.as_str(),
                    context.thread_id.as_str(),
                    context.recipient_email.as_str(),
                    now_unix,
                    FollowUpEventType::SkippedRecipientLimit,
                    format!(
                        "recipient {} reached daily limit {}",
                        context.recipient_email, policy.per_recipient_daily_limit
                    ),
                );
                emitted_events.push(event);
                continue;
            }

            let status = if adjusted_due_at <= now_unix {
                FollowUpJobStatus::Executed
            } else {
                FollowUpJobStatus::Scheduled
            };
            let job = FollowUpJob {
                job_id: next_job_id(state),
                thread_id: context.thread_id.clone(),
                recipient_email: context.recipient_email.clone(),
                rule_id: rule.rule_id.clone(),
                scheduled_for_unix: adjusted_due_at,
                status,
                reason,
            };
            state.jobs.insert(job.job_id.clone(), job.clone());

            if job
                .reason
                .as_deref()
                .is_some_and(|entry| entry == "deferred_to_allowed_send_window")
            {
                let event = push_event(
                    state,
                    job.job_id.as_str(),
                    context.thread_id.as_str(),
                    context.recipient_email.as_str(),
                    now_unix,
                    FollowUpEventType::DeferredByWindow,
                    format!("deferred to unix {}", job.scheduled_for_unix),
                );
                emitted_events.push(event);
            }

            match job.status {
                FollowUpJobStatus::Scheduled => {
                    upcoming_jobs.push(job.clone());
                    let event = push_event(
                        state,
                        job.job_id.as_str(),
                        context.thread_id.as_str(),
                        context.recipient_email.as_str(),
                        now_unix,
                        FollowUpEventType::UpcomingScheduled,
                        format!("scheduled_for={}", job.scheduled_for_unix),
                    );
                    emitted_events.push(event);
                }
                FollowUpJobStatus::Executed => {
                    executed_jobs.push(job.clone());
                    let event = push_event(
                        state,
                        job.job_id.as_str(),
                        context.thread_id.as_str(),
                        context.recipient_email.as_str(),
                        now_unix,
                        FollowUpEventType::Executed,
                        format!("executed_at={now_unix}"),
                    );
                    emitted_events.push(event);
                }
                FollowUpJobStatus::SkippedRecipientLimit => {}
            }
        }
    }

    Ok(FollowUpTickOutcome {
        upcoming_jobs,
        executed_jobs,
        emitted_events,
    })
}

fn validate_policy(policy: &FollowUpSchedulerPolicy) -> Result<(), FollowUpSchedulerError> {
    let hours = [
        ("business_hours_start_utc", policy.business_hours_start_utc),
        ("business_hours_end_utc", policy.business_hours_end_utc),
        ("quiet_hours_start_utc", policy.quiet_hours_start_utc),
        ("quiet_hours_end_utc", policy.quiet_hours_end_utc),
    ];
    for (name, value) in hours {
        if value >= 24 {
            return Err(FollowUpSchedulerError::InvalidPolicy(format!(
                "{name} must be between 0 and 23"
            )));
        }
    }

    if policy.business_hours_start_utc == policy.business_hours_end_utc {
        return Err(FollowUpSchedulerError::InvalidPolicy(
            "business hours start and end cannot be equal".to_string(),
        ));
    }
    if policy.per_recipient_daily_limit == 0 {
        return Err(FollowUpSchedulerError::InvalidPolicy(
            "per_recipient_daily_limit must be greater than zero".to_string(),
        ));
    }

    Ok(())
}

fn has_open_job_for(state: &FollowUpSchedulerState, thread_id: &str, rule_id: &str) -> bool {
    state.jobs.values().any(|job| {
        job.thread_id == thread_id
            && job.rule_id == rule_id
            && matches!(job.status, FollowUpJobStatus::Scheduled)
    })
}

fn rule_due_at(context: &ThreadFollowUpContext, rule: &FollowUpRule) -> Option<u64> {
    if !context.awaiting_reply {
        return None;
    }

    match rule.kind {
        FollowUpRuleKind::NoReplyAfterDays { days_without_reply } => {
            Some(
                context
                    .last_outbound_unix
                    .saturating_add((days_without_reply as u64).saturating_mul(86_400)),
            )
        }
        FollowUpRuleKind::UnansweredCriticalThread {
            hours_without_reply,
        } => {
            if !context.is_critical {
                return None;
            }
            let due_at = context
                .last_outbound_unix
                .saturating_add((hours_without_reply as u64).saturating_mul(3_600));
            Some(due_at)
        }
        FollowUpRuleKind::ReminderCadence {
            cadence_days,
            max_reminders,
        } => {
            if context.reminder_count >= max_reminders {
                return None;
            }
            let next_interval = (context.reminder_count as u64).saturating_add(1);
            Some(
                context
                    .last_outbound_unix
                    .saturating_add((cadence_days as u64).saturating_mul(86_400 * next_interval)),
            )
        }
    }
}

fn next_allowed_send_time(candidate_unix: u64, policy: &FollowUpSchedulerPolicy) -> u64 {
    let mut at = candidate_unix;
    for _ in 0..(24 * 14) {
        let hour = ((at / 3_600) % 24) as u8;
        if is_hour_in_window(hour, policy.business_hours_start_utc, policy.business_hours_end_utc)
            && !is_hour_in_window(hour, policy.quiet_hours_start_utc, policy.quiet_hours_end_utc)
        {
            return at;
        }
        at = at.saturating_add(3_600);
    }
    at
}

fn is_hour_in_window(hour: u8, start: u8, end: u8) -> bool {
    if start < end {
        hour >= start && hour < end
    } else {
        hour >= start || hour < end
    }
}

fn recipient_follow_up_count_for_day(
    state: &FollowUpSchedulerState,
    recipient_email: &str,
    day_index: u64,
) -> u32 {
    state
        .jobs
        .values()
        .filter(|job| job.recipient_email.eq_ignore_ascii_case(recipient_email))
        .filter(|job| job.scheduled_for_unix / 86_400 == day_index)
        .filter(|job| job.status != FollowUpJobStatus::SkippedRecipientLimit)
        .count() as u32
}

fn next_job_id(state: &FollowUpSchedulerState) -> String {
    format!("followup-{:05}", state.jobs.len().saturating_add(1))
}

fn next_event_id(state: &FollowUpSchedulerState) -> String {
    format!("followup-event-{:05}", state.event_log.len().saturating_add(1))
}

fn push_event(
    state: &mut FollowUpSchedulerState,
    job_id: &str,
    thread_id: &str,
    recipient_email: &str,
    at_unix: u64,
    event_type: FollowUpEventType,
    detail: String,
) -> FollowUpEvent {
    let event = FollowUpEvent {
        event_id: next_event_id(state),
        job_id: job_id.to_string(),
        thread_id: thread_id.to_string(),
        recipient_email: recipient_email.to_string(),
        at_unix,
        event_type,
        detail,
    };
    state.event_log.push(event.clone());
    event
}

#[cfg(test)]
mod tests {
    use super::{
        FollowUpEventType, FollowUpRule, FollowUpRuleKind, FollowUpSchedulerPolicy,
        FollowUpSchedulerState, FollowUpTickOutcome, ThreadFollowUpContext,
        run_follow_up_scheduler_tick,
    };

    fn default_context(thread_id: &str, recipient: &str) -> ThreadFollowUpContext {
        ThreadFollowUpContext {
            thread_id: thread_id.to_string(),
            recipient_email: recipient.to_string(),
            last_inbound_unix: 0,
            last_outbound_unix: 0,
            awaiting_reply: true,
            is_critical: false,
            reminder_count: 0,
        }
    }

    fn policy_with(rule: FollowUpRule) -> FollowUpSchedulerPolicy {
        FollowUpSchedulerPolicy {
            rules: vec![rule],
            business_hours_start_utc: 9,
            business_hours_end_utc: 18,
            quiet_hours_start_utc: 22,
            quiet_hours_end_utc: 6,
            per_recipient_daily_limit: 2,
        }
    }

    fn contains_event(outcome: &FollowUpTickOutcome, event_type: FollowUpEventType) -> bool {
        outcome
            .emitted_events
            .iter()
            .any(|event| event.event_type == event_type)
    }

    #[test]
    fn schedules_upcoming_jobs_for_not_yet_due_threads() {
        let now = 5u64 * 86_400 + (10u64 * 3_600);
        let context = ThreadFollowUpContext {
            last_outbound_unix: now.saturating_sub(86_400),
            ..default_context("thread-1", "person@example.com")
        };
        let policy = policy_with(FollowUpRule {
            rule_id: "rule.no_reply_2d".to_string(),
            enabled: true,
            kind: FollowUpRuleKind::NoReplyAfterDays {
                days_without_reply: 2,
            },
        });
        let mut state = FollowUpSchedulerState::default();

        let outcome = run_follow_up_scheduler_tick(&mut state, &[context], &policy, now)
            .expect("tick should succeed");

        assert_eq!(outcome.upcoming_jobs.len(), 1);
        assert!(outcome.executed_jobs.is_empty());
        assert!(contains_event(&outcome, FollowUpEventType::UpcomingScheduled));
    }

    #[test]
    fn executes_critical_thread_follow_up_when_due() {
        let now = 8u64 * 86_400 + (11u64 * 3_600);
        let context = ThreadFollowUpContext {
            last_outbound_unix: now.saturating_sub(6 * 3_600),
            is_critical: true,
            ..default_context("thread-critical", "critical@example.com")
        };
        let policy = policy_with(FollowUpRule {
            rule_id: "rule.critical_4h".to_string(),
            enabled: true,
            kind: FollowUpRuleKind::UnansweredCriticalThread {
                hours_without_reply: 4,
            },
        });
        let mut state = FollowUpSchedulerState::default();

        let outcome = run_follow_up_scheduler_tick(&mut state, &[context], &policy, now)
            .expect("tick should succeed");

        assert_eq!(outcome.executed_jobs.len(), 1);
        assert!(contains_event(&outcome, FollowUpEventType::Executed));
    }

    #[test]
    fn defers_during_quiet_hours_and_emits_window_events() {
        let now = 10u64 * 86_400 + (22u64 * 3_600);
        let context = ThreadFollowUpContext {
            last_outbound_unix: now.saturating_sub(4 * 3_600),
            is_critical: true,
            ..default_context("thread-quiet", "quiet@example.com")
        };
        let policy = policy_with(FollowUpRule {
            rule_id: "rule.critical_2h".to_string(),
            enabled: true,
            kind: FollowUpRuleKind::UnansweredCriticalThread {
                hours_without_reply: 2,
            },
        });
        let mut state = FollowUpSchedulerState::default();

        let outcome = run_follow_up_scheduler_tick(&mut state, &[context], &policy, now)
            .expect("tick should succeed");

        assert_eq!(outcome.upcoming_jobs.len(), 1);
        assert!(contains_event(&outcome, FollowUpEventType::DeferredByWindow));
        assert!(contains_event(&outcome, FollowUpEventType::UpcomingScheduled));
    }

    #[test]
    fn enforces_per_recipient_daily_limit() {
        let now = 12u64 * 86_400 + (10u64 * 3_600);
        let contexts = vec![
            ThreadFollowUpContext {
                last_outbound_unix: now.saturating_sub(5 * 3_600),
                is_critical: true,
                ..default_context("thread-a", "same@example.com")
            },
            ThreadFollowUpContext {
                last_outbound_unix: now.saturating_sub(5 * 3_600),
                is_critical: true,
                ..default_context("thread-b", "same@example.com")
            },
        ];
        let mut policy = policy_with(FollowUpRule {
            rule_id: "rule.critical_2h".to_string(),
            enabled: true,
            kind: FollowUpRuleKind::UnansweredCriticalThread {
                hours_without_reply: 2,
            },
        });
        policy.per_recipient_daily_limit = 1;
        let mut state = FollowUpSchedulerState::default();

        let outcome =
            run_follow_up_scheduler_tick(&mut state, contexts.as_slice(), &policy, now).unwrap();

        assert_eq!(outcome.executed_jobs.len(), 1);
        assert!(contains_event(&outcome, FollowUpEventType::SkippedRecipientLimit));
    }
}
