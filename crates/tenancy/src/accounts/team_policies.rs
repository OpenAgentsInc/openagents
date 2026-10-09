//! Owner-reviewed restrictions beside current native account authority.
use super::{Accounts, Lock, MemberRef, MemberStatus, Role, Store, active_member, unix_now};
use receipts::team_policy::{Change, Effect, Reference, Revision, SCHEMA, Snapshot, identifier};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dispatch {
    pub snapshot: Snapshot,
    pub handed_off: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub policies: BTreeMap<String, Vec<Revision>>,
    /// A sealed handoff is never repeated, including after a lost response.
    pub dispatches: BTreeMap<String, Dispatch>,
}
impl Book {
    pub fn is_empty(&self) -> bool {
        self.policies.is_empty() && self.dispatches.is_empty()
    }
    pub(super) fn validate(&self, store: &Store) -> Result<(), String> {
        if self.policies.len() > 1024 || self.dispatches.len() > 4096 {
            return Err("Team policy history exceeds its bound.".into());
        }
        for (ws, history) in &self.policies {
            if !store.workspaces.contains_key(ws) || history.is_empty() || history.len() > 128 {
                return Err("Invalid team policy workspace history.".into());
            }
            let mut previous: Option<&Revision> = None;
            for r in history {
                r.validate()?;
                if r.workspace != *ws
                    || r.terms.version != previous.map_or(1, |p| p.terms.version + 1)
                    || r.supersedes.as_deref() != previous.map(|p| p.digest.as_str())
                    || !store.accounts.contains_key(&r.reviewer)
                    || !store.accounts.contains_key(&r.owner)
                {
                    return Err("Invalid team policy lineage.".into());
                }
                previous = Some(r);
            }
        }
        for (request, dispatch) in &self.dispatches {
            let s = &dispatch.snapshot;
            s.validate()?;
            if !identifier(request)
                || !store.accounts.contains_key(&s.member)
                || !self.policies.get(&s.policy.workspace).is_some_and(|h| {
                    h.iter()
                        .any(|p| p.reference() == s.policy && p.terms.rules.contains(&s.rule))
                })
            {
                return Err("Invalid retained team dispatch admission.".into());
            }
        }
        Ok(())
    }
    pub fn current(&self, workspace: &str) -> Option<&Revision> {
        self.policies.get(workspace)?.last()
    }
}
/// Holds the native account writer lock through the actual effect handoff.
/// It carries no grant to execute, disclose, or spend beyond native admission.
pub struct Guard {
    pub snapshot: Snapshot,
    _lock: Lock,
    accounts: Accounts,
    state: super::team_capabilities::StateHandle,
    digest: String,
}
impl Guard {
    /// Check native custody and expiry at the exact disclosure handoff.
    pub fn before_effect(&self) -> Result<(), String> {
        self._lock.check().map_err(|e| e.to_string())?;
        self.accounts.team_check_state(&self.state, &self.digest)?;
        if unix_now() >= self.snapshot.policy.expires_unix {
            return Err("The reviewed team policy expired before disclosure.".into());
        }
        Ok(())
    }
}
fn member(store: &Store, supplied: &MemberRef) -> Result<(), String> {
    let ws = store
        .workspaces
        .get(&supplied.workspace)
        .ok_or("Unknown policy workspace.")?;
    let m = active_member(ws, &supplied.account).map_err(|e| e.to_string())?;
    if m.role != supplied.role
        || m.epoch != supplied.epoch
        || ws.members_epoch != supplied.members_epoch
    {
        return Err("Current team membership changed.".into());
    }
    Ok(())
}
fn authority(store: &Store, r: &Revision, now: u64) -> Result<(), String> {
    if now >= r.terms.expires_unix {
        return Err("The reviewed team policy expired.".into());
    }
    let ws = store
        .workspaces
        .get(&r.workspace)
        .ok_or("Unknown policy workspace.")?;
    let owner = active_member(ws, &r.owner).map_err(|e| e.to_string())?;
    let reviewer = active_member(ws, &r.reviewer).map_err(|e| e.to_string())?;
    if owner.role != Role::Owner
        || owner.epoch != r.owner_epoch
        || reviewer.role < Role::Admin
        || reviewer.epoch != r.reviewer_epoch
    {
        return Err("The policy's current owner or administrator authority changed.".into());
    }
    Ok(())
}
impl Accounts {
    /// Reauthenticate inside the native writer lock before reviewing a change.
    /// An administrator may only remove existing exact scopes or shorten expiry.
    pub fn review_team_policy(
        &self,
        workspace: &str,
        change: Change,
        authenticate: impl FnOnce(&Store) -> Result<MemberRef, String>,
    ) -> Result<Revision, String> {
        change.terms.validate()?;
        let lock = Lock::acquire(&self.dir).map_err(|e| e.to_string())?;
        lock.check().map_err(|e| e.to_string())?;
        let (mut store, mut held) = self.team_state()?;
        let actor = authenticate(&store)?;
        member(&store, &actor)?;
        if actor.workspace != workspace || actor.role < Role::Admin {
            return Err("Only a current team owner or administrator may review policy.".into());
        }
        let now = unix_now();
        if change.terms.expires_unix <= now
            || change.terms.expires_unix > now.saturating_add(2_592_000)
        {
            return Err("Review an expiry within the next 30 days.".into());
        }
        let previous = store.team_policies.current(workspace);
        if previous.map(|p| &p.digest) != change.expected_digest.as_ref()
            || change.terms.version != previous.map_or(1, |p| p.terms.version + 1)
        {
            return Err("The reviewed policy predecessor changed; read it before retrying.".into());
        }
        if actor.role == Role::Admin
            && previous.is_none_or(|p| {
                authority(&store, p, now).is_err() || !change.terms.narrows(&p.terms)
            })
        {
            return Err("An administrator may only narrow the current owner policy.".into());
        }
        let ws = &store.workspaces[workspace];
        let owner = ws
            .members
            .values()
            .find(|m| m.status == MemberStatus::Active && m.role == Role::Owner)
            .ok_or("A current owner is required.")?;
        let mut revision = Revision {
            schema: SCHEMA.into(),
            workspace: workspace.into(),
            terms: change.terms,
            supersedes: change.expected_digest,
            reviewer: actor.account,
            reviewer_epoch: actor.epoch,
            owner: owner.account.clone(),
            owner_epoch: owner.epoch,
            reviewed_at: now,
            digest: String::new(),
        };
        revision.digest = revision.compute_digest();
        store
            .team_policies
            .policies
            .entry(workspace.into())
            .or_default()
            .push(revision.clone());
        self.team_commit(&lock, &mut held, &mut store)?;
        Ok(revision)
    }
    /// Read a reference under fresh native membership. Rules are administrator-only.
    pub fn read_team_policy(
        &self,
        authenticate: impl FnOnce(&Store) -> Result<MemberRef, String>,
    ) -> Result<(Reference, Option<Revision>), String> {
        let lock = Lock::acquire(&self.dir).map_err(|e| e.to_string())?;
        lock.check().map_err(|e| e.to_string())?;
        let (store, held) = self.team_state()?;
        let actor = authenticate(&store)?;
        member(&store, &actor)?;
        lock.check().map_err(|e| e.to_string())?;
        self.team_check_state(&held, &store.digest)?;
        let r = store
            .team_policies
            .current(&actor.workspace)
            .ok_or("No reviewed team policy is available.")?;
        Ok((
            r.reference(),
            (actor.role >= Role::Admin).then(|| r.clone()),
        ))
    }
    /// Exact provenance admission. `authenticate` is a trusted local adapter that
    /// reopens its native key or session; request fields are never credential proof.
    /// When `dispatch` is set, seal the once-only handoff before any native effect.
    pub fn team_policy_guard(
        &self,
        effect: &Effect,
        expected: Option<&Snapshot>,
        dispatch: Option<&str>,
        authenticate: impl FnOnce(&Store) -> Result<(MemberRef, String), String>,
    ) -> Result<Guard, String> {
        self.policy_guard(effect, expected, dispatch, false, authenticate)
    }
    pub fn prepare_team_policy_guard(
        &self,
        effect: &Effect,
        request: &str,
        authenticate: impl FnOnce(&Store) -> Result<(MemberRef, String), String>,
    ) -> Result<Guard, String> {
        self.policy_guard(effect, None, Some(request), true, authenticate)
    }
    fn policy_guard(
        &self,
        effect: &Effect,
        expected: Option<&Snapshot>,
        dispatch: Option<&str>,
        prepare: bool,
        authenticate: impl FnOnce(&Store) -> Result<(MemberRef, String), String>,
    ) -> Result<Guard, String> {
        effect.validate()?;
        let lock = Lock::acquire(&self.dir).map_err(|e| e.to_string())?;
        lock.check().map_err(|e| e.to_string())?;
        let (mut store, mut held) = self.team_state()?;
        let (actor, credential) = authenticate(&store)?;
        member(&store, &actor)?;
        let r = store
            .team_policies
            .current(&actor.workspace)
            .ok_or("No reviewed team data policy is available.")?;
        let now = unix_now();
        authority(&store, r, now)?;
        let rule = r.terms.rules.iter().find(|r| &r.effect == effect).ok_or(
            "The exact source, data, model, plugin, recipient, or placement was not reviewed.",
        )?;
        let snapshot = Snapshot {
            policy: r.reference(),
            member: actor.account,
            membership_epoch: actor.epoch,
            workspace_members_epoch: actor.members_epoch,
            credential_reference: credential,
            rule: rule.clone(),
            admitted_at: expected.map_or(now, |s| s.admitted_at),
        };
        snapshot.validate()?;
        if expected.is_some_and(|s| s != &snapshot) {
            return Err("The originally admitted team policy or member authority changed.".into());
        }
        if let Some(request) = dispatch {
            if !identifier(request) {
                return Err("Invalid original team request identity.".into());
            }
            if let Some(old) = store.team_policies.dispatches.get(request) {
                if prepare || old.handed_off || old.snapshot != snapshot {
                    return Err("This original team request was already admitted or handed off; read its outcome instead of repeating it.".into());
                }
            }
            store.team_policies.dispatches.insert(
                request.into(),
                Dispatch {
                    snapshot: snapshot.clone(),
                    handed_off: !prepare,
                },
            );
            self.team_commit(&lock, &mut held, &mut store)?;
        }
        Ok(Guard {
            snapshot,
            _lock: lock,
            accounts: self.clone(),
            state: held,
            digest: store.digest,
        })
    }
    pub fn team_policy_handed_off(&self, request: &str) -> Result<bool, String> {
        Ok(self
            .team_state()?
            .0
            .team_policies
            .dispatches
            .contains_key(request))
    }
}
#[cfg(test)]
mod tests;
