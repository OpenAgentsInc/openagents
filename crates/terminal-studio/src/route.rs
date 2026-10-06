//! The declared Studio route mount over the existing same-user socket.
//! Direct console intents remain independent of this optional route path.
use coder_access::{Code, Operation, Outcome, Right};
use openagents_chat::studio::{Failure, Host, ResultView, State};
use openagents_connect::control::OperationClient;
use route_contract::binding::{Current, HostPlacement};

/// The local operator already admitted by the private control socket. This
/// does not represent a remote device or manufacture a device grant.
pub struct LocalHost {
    client: OperationClient,
    placement: HostPlacement,
}
impl LocalHost {
    pub fn new(socket: std::path::PathBuf, placement: HostPlacement) -> Self {
        Self {
            client: OperationClient::new(socket),
            placement,
        }
    }
}
impl Host for LocalHost {
    fn current(&mut self) -> Result<Current, String> {
        let outcome = self
            .client
            .call(&"0".repeat(64), &Operation::StudioSnapshot {})
            .map_err(|e| e.to_string())?;
        let Outcome::Studio { snapshot } = outcome else {
            return Err("host returned no Studio snapshot".into());
        };
        Ok(Current {
            computer: self.placement.computer.clone(),
            generation: snapshot.stream,
            recipient: self.placement.recipient.clone(),
            grant: None,
            terminal_generation: None,
        })
    }
    fn rights(&self) -> Vec<Right> {
        vec![Right::Observe, Right::Operate, Right::Review]
    }
    fn send(&mut self, request: &str, operation: &Operation) -> Result<Outcome, Failure> {
        self.client.call(request, operation).map_err(|e| {
            if matches!(
                e.code,
                Code::Unavailable | Code::Transport | Code::Malformed
            ) {
                Failure::Unknown
            } else {
                Failure::Refused(e.to_string())
            }
        })
    }
}

/// Project the validated route receipt in the shared workbench. A finished
/// operation does not assert task completion, check success, or cost.
pub fn rows(result: &ResultView, stream: &str) -> Result<Vec<String>, String> {
    if result.schema != route_contract::studio::RESULT_SCHEMA {
        return Err("unknown Studio result schema".into());
    }
    let mut rows = vec![format!("STUDIO ROUTE {}", result.request)];
    match &result.state {
        State::Unknown => {
            rows.push("Outcome unknown; reconcile the original request explicitly.".into())
        }
        State::Refused { reason } => rows.push(format!("Host refused: {reason}")),
        State::Completed { outcome } => {
            outcome.validate().map_err(|e| e.to_string())?;
            match outcome.as_ref() {
                Outcome::Dispatched { receipt } => rows.push(format!(
                    "Host receipt: {} {}",
                    receipt.operation, receipt.reference
                )),
                Outcome::Review { review } => {
                    rows.extend(crate::studio::project_review(stream, review)?.rows)
                }
                Outcome::Merged { merged } => {
                    rows.push(format!(
                        "Task {} {:?}: base {} HEAD {} tree {}",
                        merged.task, merged.verdict, merged.base, merged.head_commit, merged.head
                    ));
                    rows.push(format!("Local landing: {:?}", merged.publication));
                }
                _ => return Err("result is not a Studio route outcome".into()),
            }
        }
    }
    Ok(rows)
}
