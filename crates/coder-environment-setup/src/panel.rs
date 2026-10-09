//! The setup chat an operator's environment panel shows (ENV-07).
//!
//! [`Panel`] answers [`coder_cloud::operator::SetupSessions`] for one
//! composed [`Setup`] owner: it lists an environment's sessions read-only
//! and retains user steering synchronously, with its evidence, before it
//! answers. Waking a checkpointed computer is a provider call, so the panel
//! never makes it on the request path: it hands the session to the owner's
//! own loop through the wake channel, which calls [`Setup::resume`].

use crate::SetupSession;
use crate::service::Setup;
use crate::service::SetupError;
use crate::transition::{Run, SetupState};
use coder_access::{Code, environment as view};
use coder_working_computer::provider::Commands;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

pub struct Panel<P> {
    setup: Arc<Setup<P>>,
    wake: UnboundedSender<String>,
}

impl<P> Panel<P> {
    /// A panel over `setup` and the receiver its owner drains, calling
    /// [`Setup::resume`] for each session ID.
    pub fn new(setup: Arc<Setup<P>>) -> (Self, UnboundedReceiver<String>) {
        let (wake, rx) = unbounded_channel();
        (Self { setup, wake }, rx)
    }
}

fn tag(value: serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s,
        serde_json::Value::Object(m) => ["state", "kind"]
            .iter()
            .find_map(|k| m.get(*k).and_then(|v| v.as_str()).map(str::to_owned))
            .unwrap_or_else(|| "unknown".into()),
        _ => "unknown".into(),
    }
}
fn last<T: Clone>(rows: &[T]) -> Vec<T> {
    rows[rows.len().saturating_sub(view::MAX_ROWS)..].to_vec()
}

/// The panel row for one session: state, steering, and command states.
/// Command specs, environments, and outputs stay in the record and its
/// evidence.
pub fn row(s: &SetupSession) -> view::Setup {
    let (question, reason) = match &s.state {
        SetupState::AwaitingInput { question } => (Some(question.clone()), None),
        SetupState::Failed { reason } | SetupState::Cancelled { reason } => {
            (None, Some(reason.clone()))
        }
        _ => (None, None),
    };
    view::Setup {
        id: s.id.clone(),
        state: tag(serde_json::to_value(&s.state).unwrap_or_default()),
        question,
        reason,
        objective: s.objective.clone(),
        steering: last(&s.steering)
            .into_iter()
            .map(|t| view::Steering {
                at_ms: t.at_ms,
                text: t.text,
            })
            .collect(),
        commands: last(&s.commands)
            .into_iter()
            .map(|c| view::SetupCommand {
                id: c.id.clone(),
                purpose: tag(serde_json::to_value(&c.purpose).unwrap_or_default()),
                state: match &c.run {
                    Run::Unknown { .. } => "needs_reconciliation".into(),
                    run => tag(serde_json::to_value(run).unwrap_or_default()),
                },
            })
            .collect(),
        recipe_revisions: last(&s.recipe_revisions),
        updated_ms: s.updated_ms,
        steerable: !s.state.terminal(),
    }
}

fn code(e: &SetupError) -> Code {
    match e {
        SetupError::Session(crate::store::StoreError::NotFound) => Code::Forbidden,
        SetupError::Session(crate::store::StoreError::Busy) => Code::Unavailable,
        SetupError::Refused(_) => Code::Conflict,
        _ => Code::Unavailable,
    }
}

impl<P: Commands + Send + Sync> coder_cloud::operator::SetupSessions for Panel<P> {
    fn sessions(&self, environment: &str) -> Result<Vec<view::Setup>, Code> {
        let mut rows: Vec<_> = self
            .setup
            .sessions()
            .list()
            .map_err(|_| Code::Unavailable)?
            .into_iter()
            .filter(|s| s.environment == environment)
            .collect();
        rows.reverse();
        rows.truncate(view::MAX_ROWS);
        Ok(rows.iter().map(row).collect())
    }

    fn steer(
        &self,
        environment: &str,
        session: &str,
        text: &str,
        now_ms: u64,
    ) -> Result<String, Code> {
        let current = self
            .setup
            .sessions()
            .read(session)
            .map_err(|e| code(&SetupError::Session(e)))?;
        if current.environment != environment {
            return Err(Code::Forbidden);
        }
        let s = self
            .setup
            .retain_steering(session, text, now_ms)
            .map_err(|e| code(&e))?;
        if matches!(s.state, SetupState::AwaitingInput { .. }) {
            // The owner's loop wakes the computer; a closed loop leaves the
            // steering retained for its next visit.
            let _ = self.wake.send(s.id.clone());
            return Ok("steering_retained_wake_requested".into());
        }
        Ok("steering_retained".into())
    }
}
