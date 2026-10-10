//! Background Claude Code tasks on the user's own plan (BYO-03).
//!
//! A Cloud job whose engine is [`crate::claude::ENGINE`] runs the pinned,
//! unmodified `claude -p` inside the user's own computer on the sign-in that
//! computer holds (`docs/cloud/claude-code-byo.md`). How many such turns may
//! run at once is BYO-04's rule, [`crate::claude::admit_turns`]: one
//! automated turn at a time on a plan login, fan-out refused. This module
//! owns what happens when a turn ends on Claude Code's own account:
//!
//! - **Usage limit.** The job does not fail. It becomes [`State::Paused`]
//!   until the reset Claude Code reported (or the capacity owner's default
//!   hold when it reported none), the pause is recorded in that computer's
//!   usage-limit book through the capacity owner
//!   (`microcoder_loop::capacity`), and the operator continues the same
//!   session after the reset. The pause is retained in the job record, so a
//!   restarted operator picks it up ([`crate::operator::Operator::resume_paused`]).
//! - **Missing or expired login.** The job stops with a sign-in prompt that
//!   names the computer's Sign in to Claude action (BYO-01), and an event
//!   says so; it never fails with only an exit status.
//!
//! The job's evidence names the engine, its pinned version, and the
//! credential *type* ([`evidence`]): never a credential, which OpenAgents
//! never holds for a plan login.

use crate::{Mode, Record, Spec, State};
use coder_engine_status::Notice;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Where a Claude job keeps its engine evidence and pause in
/// [`Record::binding`].
pub const BINDING: &str = "claude";

/// The longest a paused job sleeps between checks for cancellation.
pub const CHECK_EVERY_SECONDS: u64 = 2;

/// How many times, a second apart, a due job asks the current policy before
/// it stays paused for a follow or a restarted operator.
pub const ADMIT_TRIES: u32 = 5;

/// Whether this job runs the Claude Code engine through the Coder runtime.
#[must_use]
pub fn applies(spec: &Spec) -> bool {
    spec.agent == crate::claude::ENGINE && spec.mode == Mode::Coder
}

/// The credential type a job runs on, by name only.
#[must_use]
pub fn credential_type(names: &[String]) -> &'static str {
    credential_word(crate::claude::sign_in(names.iter().map(String::as_str)))
}

/// The engine evidence a job carries: engine, pinned version, credential
/// type. Closed words and the pin only; nothing here can hold a credential.
#[must_use]
pub fn evidence(spec: &Spec) -> Value {
    json!({
        "engine": crate::claude::ENGINE,
        "version": crate::claude::VERSION,
        "credential": credential_type(&spec.credential_names),
    })
}

/// Record the engine evidence on a new job, keyed to the computer whose
/// login (or credential) it runs on, and show it as the first event.
pub fn admit(record: &mut Record, computer: &str) {
    if !applies(&record.spec) {
        return;
    }
    let evidence = evidence(&record.spec);
    record.binding[BINDING] = json!({"evidence": evidence, "computer": computer});
    let mut event = evidence;
    event["event"] = json!("engine");
    record.events.push(event);
}

fn credential_word(sign_in: crate::claude::SignIn) -> &'static str {
    use crate::claude::{OwnCredential, SignIn};
    match sign_in {
        SignIn::PlanLogin => "claude_plan_login",
        SignIn::Own(OwnCredential::AnthropicApiKey) => "anthropic_api_key",
        SignIn::Own(OwnCredential::Bedrock) => "bedrock",
        SignIn::Own(OwnCredential::Vertex) => "vertex",
        SignIn::Own(OwnCredential::Foundry) => "foundry",
        SignIn::Own(OwnCredential::SubscriptionToken) => "claude_subscription_token",
    }
}

/// The sign-in class the job's current turn runs on, from its evidence.
#[must_use]
pub fn turn_sign_in(record: &Record) -> crate::claude::SignIn {
    use crate::claude::{OwnCredential, SignIn};
    match record.binding[BINDING]["evidence"]["credential"].as_str() {
        Some("anthropic_api_key") => SignIn::Own(OwnCredential::AnthropicApiKey),
        Some("bedrock") => SignIn::Own(OwnCredential::Bedrock),
        Some("vertex") => SignIn::Own(OwnCredential::Vertex),
        Some("foundry") => SignIn::Own(OwnCredential::Foundry),
        Some("claude_subscription_token") => SignIn::Own(OwnCredential::SubscriptionToken),
        Some(_) => SignIn::PlanLogin,
        None => crate::claude::sign_in(record.spec.credential_names.iter().map(String::as_str)),
    }
}

/// Set the class the job's next turn runs on (BYO-05: the user's released
/// credential, or the plan login inside the computer). A change is shown
/// as an engine event. The type only; never a credential.
pub fn set_turn_sign_in(record: &mut Record, sign_in: crate::claude::SignIn) {
    if !applies(&record.spec) || turn_sign_in(record) == sign_in {
        return;
    }
    let word = credential_word(sign_in);
    record.binding[BINDING]["evidence"]["credential"] = json!(word);
    record.events.push(json!({
        "event": "engine",
        "engine": crate::claude::ENGINE,
        "version": crate::claude::VERSION,
        "credential": word,
    }));
}

/// How a Claude turn ended on Claude Code's own account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A usage limit, with the reset Claude Code reported.
    Limited { resets_at: Option<u64> },
    /// No login, or one that expired or stopped working.
    SignIn,
}

/// Error text a failed job carries: its error, a failed result, and
/// errors tool calls returned. Never what a model wrote.
fn error_texts(record: &Record) -> Vec<&str> {
    fn tool_error<'a>(event: &'a Value, out: &mut Vec<&'a str>) {
        if event["event"] == "tool" && event["running"] == false {
            if let Some(error) = event["output"]["error"].as_str() {
                out.push(error);
            }
        }
        if event["event"] == "delegation" {
            tool_error(&event["update"], out);
        }
    }
    let mut out: Vec<&str> = record.error.iter().map(String::as_str).collect();
    if let Some(error) = record.result.as_ref().and_then(|r| r["error"].as_str()) {
        out.push(error);
    }
    let start = record.binding["turn_start"]
        .as_u64()
        .and_then(|at| usize::try_from(at).ok())
        .unwrap_or(0)
        .min(record.events.len());
    for event in &record.events[start..] {
        tool_error(event, &mut out);
        if event.get("event").is_none() {
            if let Some(error) = event["error"].as_str() {
                out.push(error);
            }
        }
    }
    out
}

/// What a failed Claude job's errors say, if it ended on a usage limit or
/// a login that needs the person.
#[must_use]
pub fn outcome(record: &Record, now: u64) -> Option<Outcome> {
    if !applies(&record.spec) || record.state != State::Failed {
        return None;
    }
    let notices: Vec<Notice> = error_texts(record)
        .into_iter()
        .flat_map(str::lines)
        .filter_map(|line| Notice::from_text(line, now))
        .collect();
    if let Some(resets_at) = notices.iter().find_map(|n| match n {
        Notice::Limited { resets_at, .. } => Some(*resets_at),
        _ => None,
    }) {
        return Some(Outcome::Limited { resets_at });
    }
    notices
        .iter()
        .any(|n| matches!(n, Notice::LoginExpired { .. }))
        .then_some(Outcome::SignIn)
}

/// A retained pause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pause {
    pub resets_at: Option<u64>,
    /// When the job resumes, in Unix seconds.
    pub until: u64,
}

/// The job's retained pause, if it is paused.
#[must_use]
pub fn pause(record: &Record) -> Option<Pause> {
    if record.state != State::Paused {
        return None;
    }
    let pause = &record.binding[BINDING]["pause"];
    Some(Pause {
        resets_at: pause["resets_at"].as_u64(),
        until: pause["until"].as_u64()?,
    })
}

/// The computer's usage-limit book under the operator state: one per
/// computer, because a limit belongs to the login that computer holds.
#[must_use]
pub fn book(state_root: &Path, record: &Record) -> PathBuf {
    let computer = record.binding[BINDING]["computer"]
        .as_str()
        .filter(|c| crate::validate_id(c).is_ok())
        .unwrap_or("default");
    state_root.join("claude-capacity").join(computer)
}

/// The sign-in prompt a stopped job shows.
pub const SIGN_IN_PROMPT: &str = "Claude Code on this computer is not signed in, or its login expired, so the task stopped. Use Sign in to Claude for this computer, complete Anthropic's own sign-in in its terminal, then continue the task.";

/// The prompt a resumed turn carries.
#[must_use]
pub fn resume_prompt(task: &str) -> String {
    format!(
        "A Claude Code usage limit paused this task, and the limit has reset. Continue where you left off; do not repeat finished work.\n\nThe request was:\n{task}"
    )
}

fn pause_text(pause: Pause) -> String {
    match pause.resets_at {
        Some(at) => format!(
            "Paused: Claude Code reported a usage limit on this computer's Claude sign-in that resets {}. The task resumes automatically then. OpenAgents keeps no usage ledger for your plan.",
            coder_engine_status::utc(at)
        ),
        None => format!(
            "Paused: Claude Code reported a usage limit without a reset time. The task tries again at {}.",
            coder_engine_status::utc(pause.until)
        ),
    }
}

/// After a turn ended: pause a job a usage limit stopped, recording the
/// limit in the computer's book through the capacity owner, or stop a job
/// whose login needs the person with the sign-in prompt. Returns what it
/// did; the caller saves the record.
///
/// # Errors
/// When the capacity book cannot be written; the job then stays failed.
pub fn settle(record: &mut Record, book: &Path, now: u64) -> crate::Result<Option<Outcome>> {
    let Some(outcome) = outcome(record, now) else {
        return Ok(None);
    };
    match outcome {
        Outcome::Limited { resets_at } => {
            use microcoder_loop::capacity::{Kind, Provider, Refusal, record_with};
            let refusal = Refusal::new(Provider::Claude, Kind::UsageLimit, now, resets_at);
            // A reset that already passed by the time the turn ended means
            // the plan has capacity again: resume at once.
            let until = match resets_at {
                Some(at) if at <= now => now,
                _ => refusal.until,
            };
            // The book belongs to the computer's login, which this host
            // never reads: no account fingerprint is taken.
            record_with(book, refusal, |_| None)?;
            let pause = Pause { resets_at, until };
            let entry = &mut record.binding[BINDING];
            entry["pause"] = json!({"resets_at": resets_at, "until": until, "at": now});
            entry["pauses"] = json!(entry["pauses"].as_u64().unwrap_or(0) + 1);
            record.state = State::Paused;
            record.error = Some(pause_text(pause));
            record.events.push(json!({
                "event": "paused",
                "engine": crate::claude::ENGINE,
                "reason": "usage_limit",
                "resets_at": resets_at,
                "until": until,
            }));
        }
        Outcome::SignIn => {
            record.binding[BINDING]["sign_in"] = json!("Sign in to Claude");
            record.error = Some(SIGN_IN_PROMPT.into());
            record.events.push(json!({
                "event": "sign_in_required",
                "engine": crate::claude::ENGINE,
                "action": "Sign in to Claude",
            }));
        }
    }
    record.updated_ms = crate::now_ms();
    Ok(Some(outcome))
}

/// Whether a paused job's reset has passed.
#[must_use]
pub fn due(record: &Record, now: u64) -> bool {
    pause(record).is_some_and(|p| now >= p.until)
}

/// Continue a paused job whose reset passed: the same session and
/// computer, with a resume turn, through the backend's continuation path.
/// Returns false when the job is not paused or not due.
pub fn resume(record: &mut Record, now: u64) -> bool {
    if !due(record, now) {
        return false;
    }
    let original = record
        .binding
        .get(BINDING)
        .and_then(|b| b["task"].as_str())
        .map_or_else(|| record.spec.task.clone(), str::to_owned);
    record.binding[BINDING]["task"] = json!(original);
    record.binding["continue_conversation"] = json!(
        record
            .remote_task
            .as_ref()
            .and_then(|t| t.conversation.clone())
    );
    record.binding["turn_start"] = json!(record.events.len());
    record.turns.push(json!({"task":record.spec.task,"result":record.result,"state":"paused","usage":record.usage,"artifacts":record.artifacts,"pause":record.binding[BINDING]["pause"]}));
    if let Some(entry) = record.binding[BINDING].as_object_mut() {
        entry.remove("pause");
    }
    record
        .events
        .push(json!({"event":"resumed","engine":crate::claude::ENGINE}));
    record.spec.task = resume_prompt(&original);
    record.state = State::Resuming;
    record.created_ms = crate::now_ms();
    record.remote_task = None;
    record.cursor = None;
    record.result = None;
    record.error = None;
    record.cancel_requested = false;
    record.cleanup_complete = false;
    record.cleanup_error = None;
    record.artifacts = None;
    record.artifact_error = None;
    record.updated_ms = crate::now_ms();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Placement, Spec};

    fn record(names: &[&str]) -> Record {
        let mut r = Record::new(
            "j1",
            Spec {
                placement: Placement::Gce,
                mode: Mode::Coder,
                agent: "claude".into(),
                task: "Fix the fixture".into(),
                model: None,
                reasoning: None,
                cwd: PathBuf::from("/w"),
                timeout_seconds: 600,
                size: "default".into(),
                template: None,
                credential_names: names.iter().map(|n| (*n).to_string()).collect(),
            },
        )
        .unwrap();
        admit(&mut r, "computer-1");
        r
    }

    fn settle_in(r: &mut Record, root: &Path, now: u64) -> crate::Result<Option<Outcome>> {
        let book = book(root, r);
        settle(r, &book, now)
    }

    fn failed(r: &mut Record, error: &str) {
        r.state = State::Failed;
        r.error = Some("The remote Coder runtime exited with status 1.".into());
        r.resource = Some("host".into());
        r.cleanup_complete = true;
        r.events.push(json!({"event":"tool","name":"acp_subagent","input":null,"output":{"error":error},"running":false}));
    }

    #[test]
    fn evidence_names_engine_version_and_credential_type_never_a_credential() {
        let plan = record(&["GH_TOKEN"]);
        assert_eq!(
            plan.binding[BINDING]["evidence"],
            json!({"engine":"claude","version":crate::claude::VERSION,"credential":"claude_plan_login"})
        );
        assert_eq!(plan.events[0]["event"], "engine");
        let key = record(&[crate::claude::API_KEY]);
        assert_eq!(
            key.binding[BINDING]["evidence"]["credential"],
            "anthropic_api_key"
        );
        let text = serde_json::to_string(&key.binding).unwrap();
        assert!(!text.contains("sk-ant"), "{text}");
    }

    #[test]
    fn a_usage_limit_pauses_until_the_reported_reset_and_records_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = record(&[]);
        failed(
            &mut r,
            "Claude Code reached a usage limit on this computer's Claude sign-in. Claude AI usage limit reached|1790164200",
        );
        let book = book(dir.path(), &r);
        assert!(book.ends_with("claude-capacity/computer-1"));
        let now = 1_790_163_058;
        assert_eq!(
            settle(&mut r, &book, now).unwrap(),
            Some(Outcome::Limited {
                resets_at: Some(1_790_164_200)
            })
        );
        assert_eq!(r.state, State::Paused);
        assert!(!r.state.terminal());
        assert!(r.error.as_deref().unwrap().contains("2026-09-23 11:50 UTC"));
        // Recorded through the capacity owner, in this computer's book.
        let held = microcoder_loop::capacity::Book::load_with(&book, |_| None);
        let refusal = held
            .blocking(microcoder_loop::capacity::Provider::Claude, now)
            .unwrap();
        assert_eq!(refusal.until, 1_790_164_200);
        // Retained: a reread record (a restarted operator) holds the pause.
        let reread: Record = serde_json::from_value(serde_json::to_value(&r).unwrap()).unwrap();
        assert_eq!(
            pause(&reread),
            Some(Pause {
                resets_at: Some(1_790_164_200),
                until: 1_790_164_200
            })
        );
        let mut reread = reread;
        assert!(!resume(&mut reread, 1_790_164_199));
        assert!(resume(&mut reread, 1_790_164_200));
        assert_eq!(reread.state, State::Resuming);
        assert!(reread.spec.task.contains("Fix the fixture"));
        assert!(
            reread
                .spec
                .task
                .starts_with("A Claude Code usage limit paused")
        );
        assert_eq!(reread.binding["turn_start"], reread.events.len() - 1);
        assert!(pause(&reread).is_none());
        // A second limit keeps the original request, not the resume prompt.
        failed(
            &mut reread,
            "You've hit your session limit · resets 11:50am (UTC)",
        );
        settle(&mut reread, &book, 1_790_164_300).unwrap();
        assert!(resume(&mut reread, u64::MAX));
        assert_eq!(reread.spec.task, resume_prompt("Fix the fixture"));
        assert_eq!(reread.binding[BINDING]["pauses"], 2);
    }

    #[test]
    fn a_limit_without_a_reset_holds_the_capacity_owners_default() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = record(&[]);
        failed(
            &mut r,
            "Claude Code reached a usage limit on this computer's Claude sign-in and did not say when it resets.",
        );
        settle_in(&mut r, dir.path(), 1_000).unwrap();
        let p = pause(&r).unwrap();
        assert_eq!(p.resets_at, None);
        assert_eq!(
            p.until,
            1_000 + microcoder_loop::capacity::Provider::Claude.unknown_hold()
        );
    }

    #[test]
    fn a_missing_or_expired_login_stops_with_the_sign_in_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = record(&[]);
        failed(
            &mut r,
            "Claude Code is not signed in on this computer, or its login expired. Please run /login: use Sign in to Claude for this computer, then continue the task.",
        );
        assert_eq!(
            settle_in(&mut r, dir.path(), 1).unwrap(),
            Some(Outcome::SignIn)
        );
        assert_eq!(r.state, State::Failed);
        assert_eq!(r.error.as_deref(), Some(SIGN_IN_PROMPT));
        assert_eq!(r.events.last().unwrap()["action"], "Sign in to Claude");
    }

    #[test]
    fn model_text_and_other_engines_never_pause() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = record(&[]);
        r.state = State::Failed;
        r.events
            .push(json!({"event":"delta","text":"You've hit your usage limit"}));
        assert_eq!(settle_in(&mut r, dir.path(), 1).unwrap(), None);
        let mut codex = record(&[]);
        codex.spec.agent = "codex".into();
        failed(&mut codex, "You've hit your usage limit");
        assert_eq!(outcome(&codex, 1), None);
    }
}
