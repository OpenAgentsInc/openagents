//! Delegated machine grants (#12163): the owner grants one coordinator
//! [`Scope::Delegate`], and that agent grants and revokes the
//! [`Scope::DELEGABLE`] scopes to other chats.
//!
//! The coordinator's host checks that it holds `delegate` ([`holds`]); the
//! target chat's host, reached through the device channel, writes the
//! subgrant ([`grant`]) or ends it ([`revoke`]). Both write a line in the
//! machine-action log with the delegating agent as the agent, the chat in
//! `target`, the scopes and the reason, so `openagents lease actions` on
//! that computer shows who granted what to whom. The owner's
//! [`super::revoke`] of the coordinator ends everything it issued.

use std::path::Path;
use std::time::Duration;

use super::{Entry, Grants, MachineGrant, Outcome, Scope, System};

/// Whether `issuer` holds `delegate` in the lease root `root` now.
///
/// # Errors
/// It doesn't, or the grants can't be read; the refusal is logged.
pub fn holds(root: &Path, issuer: &str, target: &str, reason: &str) -> Result<(), String> {
    let grants = Grants::load(root)?;
    if grants.allows(issuer.trim(), Scope::Delegate, crate::now_ms()) {
        return Ok(());
    }
    Err(refuse(
        root,
        issuer,
        target,
        reason,
        format!(
            "refused: {issuer} holds no delegate grant; the owner grants it once with `openagents lease grant machine {issuer} --scope delegate`"
        ),
    ))
}

/// Grants `scopes` to `agent` in the lease root `root` for the `delegate`
/// holder `issuer`, for `duration` or until revoked, replacing that issuer's
/// earlier subgrant to the same agent, and logs it.
///
/// # Errors
/// A scope isn't delegable, the reason or an ID is missing, the issuer names
/// itself, or the file can't be read or written. Refusals are logged.
pub fn grant(
    root: &Path,
    issuer: &str,
    agent: &str,
    scopes: &[Scope],
    duration: Option<Duration>,
    reason: &str,
) -> Result<MachineGrant, String> {
    let (issuer, agent, reason) = (issuer.trim(), agent.trim(), reason.trim());
    let refusal = if issuer.is_empty() || issuer.contains(char::is_whitespace) {
        Some("a subgrant needs the delegating agent's ID".to_owned())
    } else if agent.is_empty() || agent.contains(char::is_whitespace) {
        Some("a subgrant needs one agent ID, such as a chat ID".to_owned())
    } else if agent == issuer {
        Some("a delegate holder can't grant itself".to_owned())
    } else if reason.is_empty() {
        Some("a subgrant needs a reason, which goes in the log".to_owned())
    } else if scopes.is_empty() {
        Some("a subgrant needs at least one scope".to_owned())
    } else if let Some(scope) = scopes.iter().find(|scope| !scope.delegable()) {
        Some(format!(
            "{scope} isn't delegable: a delegate holder grants only build.prune, process.stop and host.upgrade"
        ))
    } else {
        None
    };
    if let Some(why) = refusal {
        return Err(refuse(
            root,
            issuer,
            agent,
            reason,
            format!("refused: {why}"),
        ));
    }
    let mut grants = Grants::load(root)?;
    let now = crate::now_ms();
    let mut scopes = scopes.to_vec();
    scopes.sort();
    scopes.dedup();
    let granted = MachineGrant {
        agent: agent.to_owned(),
        scopes,
        granted_at_ms: now,
        expires_at_ms: duration.map(|duration| {
            now.saturating_add(u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        }),
        issued_by: Some(issuer.to_owned()),
        reason: Some(reason.to_owned()),
    };
    grants
        .grants
        .retain(|grant| grant.agent != agent || grant.issued_by.as_deref() != Some(issuer));
    grants.grants.push(granted.clone());
    grants
        .save(root)
        .map_err(|error| format!("the machine grants can't be written: {error}"))?;
    let until = match duration {
        Some(duration) => format!("for {}s", duration.as_secs()),
        None => "until revoked".to_owned(),
    };
    let mut entry = Entry::new(&System, issuer, Scope::Delegate, reason);
    entry.outcome = Outcome::Done;
    entry.target = Some(agent.to_owned());
    entry.detail = format!("granted {} {until}", words(&granted.scopes));
    let _ = super::log(root, &entry);
    Ok(granted)
}

/// Ends the subgrant `issuer` gave `agent` in the lease root `root`, and
/// logs it; whether there was one.
///
/// # Errors
/// The file can't be read or written.
pub fn revoke(root: &Path, issuer: &str, agent: &str, reason: &str) -> Result<bool, String> {
    let (issuer, agent) = (issuer.trim(), agent.trim());
    let mut grants = Grants::load(root)?;
    let (ended, kept): (Vec<MachineGrant>, Vec<MachineGrant>) = grants
        .grants
        .into_iter()
        .partition(|grant| grant.agent == agent && grant.issued_by.as_deref() == Some(issuer));
    grants.grants = kept;
    if ended.is_empty() {
        return Ok(false);
    }
    grants
        .save(root)
        .map_err(|error| format!("the machine grants can't be written: {error}"))?;
    for grant in &ended {
        log_end(root, grant, reason.trim());
    }
    Ok(true)
}

/// Logs the end of the subgrant `grant`, for `why`.
pub(crate) fn log_end(root: &Path, grant: &MachineGrant, why: &str) {
    let issuer = grant.issued_by.as_deref().unwrap_or(super::proof::OWNER);
    let mut entry = Entry::new(&System, issuer, Scope::Delegate, why);
    entry.outcome = Outcome::Done;
    entry.target = Some(grant.agent.clone());
    entry.detail = format!("revoked {}", words(&grant.scopes));
    let _ = super::log(root, &entry);
}

fn refuse(root: &Path, issuer: &str, target: &str, reason: &str, why: String) -> String {
    let mut entry = Entry::new(&System, issuer.trim(), Scope::Delegate, reason.trim());
    entry.outcome = Outcome::Refused;
    entry.target = Some(target.trim().to_owned()).filter(|target| !target.is_empty());
    entry.detail.clone_from(&why);
    let _ = super::log(root, &entry);
    why
}

fn words(scopes: &[Scope]) -> String {
    scopes
        .iter()
        .map(|scope| scope.word())
        .collect::<Vec<_>>()
        .join(", ")
}

/// A machine grant sent to a chat's own host (`GrantMachine`, #12163).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantRequest {
    pub agent: String,
    pub scopes: Vec<Scope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub for_secs: Option<u64>,
    /// The `delegate` holder, whose own host checked its grant; the owner
    /// when absent, who may send only `delegate` this way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    #[serde(default)]
    pub reason: String,
}

/// The end of a machine grant sent to a chat's host (`RevokeMachine`).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevokeRequest {
    pub agent: String,
    /// The `delegate` holder ending its own subgrant; the owner when
    /// absent, ending all `agent` holds and issued.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    #[serde(default)]
    pub reason: String,
}

/// Writes `request` in the lease root `root`.
///
/// # Errors
/// As [`grant`], or the owner sent a scope other than `delegate`.
pub fn serve_grant(root: &Path, request: &GrantRequest) -> Result<MachineGrant, String> {
    let duration = request.for_secs.map(Duration::from_secs);
    match request.issuer.as_deref() {
        Some(issuer) => grant(
            root,
            issuer,
            &request.agent,
            &request.scopes,
            duration,
            &request.reason,
        ),
        None if request.scopes == [Scope::Delegate] => {
            let granted = super::grant(root, &request.agent, &request.scopes, duration)?;
            let mut entry = Entry::new(&System, super::proof::OWNER, Scope::Delegate, &request.reason);
            entry.outcome = Outcome::Done;
            entry.target = Some(granted.agent.clone());
            entry.detail = "granted delegate".to_owned();
            let _ = super::log(root, &entry);
            Ok(granted)
        }
        None => Err(
            "refused: the owner grants machine actions at a terminal on the computer itself; only delegate travels to another computer".to_owned(),
        ),
    }
}

/// Ends what `request` names in the lease root `root`; whether anything
/// ended.
///
/// # Errors
/// The file can't be read or written.
pub fn serve_revoke(root: &Path, request: &RevokeRequest) -> Result<bool, String> {
    match request.issuer.as_deref() {
        Some(issuer) => revoke(root, issuer, &request.agent, &request.reason),
        None => super::revoke(root, &request.agent),
    }
}
