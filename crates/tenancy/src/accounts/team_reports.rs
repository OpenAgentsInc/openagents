//! Checked private task references in the existing native account history.
//! Attaching evidence mutates no financial ledger and grants no capability.
use super::{Accounts, Lock, MemberRef, Role, Store, unix_now};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA: &str = "openagents.team-task-evidence.v1";
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub path: String,
    pub sha256: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub schema: String,
    pub receipt: String,
    pub manifest: Reference,
    pub task: String,
    pub candidate: String,
}
fn text(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl Evidence {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA
            || !self.receipt.strip_prefix("sha256:").is_some_and(hash)
            || !hash(&self.manifest.sha256)
            || !text(&self.task)
            || !text(&self.candidate)
            || self.manifest.path.is_empty()
            || self.manifest.path.len() > 1024
            || !std::path::Path::new(&self.manifest.path)
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_)))
        {
            return Err("Invalid exact private team evidence reference.".into());
        }
        Ok(())
    }
    fn fingerprint(&self) -> String {
        receipts::execution::digest_request(
            &serde_json::to_value(self).expect("evidence serializes"),
        )
    }
}
/// The owning adapter reconstructs these identities from native service records.
/// This is not a caller-supplied claim or a remote attestation.
#[derive(Clone, Debug)]
pub struct Verified {
    pub workspace: String,
    pub member: String,
    pub request: String,
    pub attempt: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub evidence: Evidence,
    pub workspace: String,
    pub member: String,
    pub request: String,
    pub attempt: u32,
    pub reviewer: String,
    pub account_revision: String,
    pub recorded_at: u64,
    pub fingerprint: String,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub records: BTreeMap<String, Record>,
}
impl Book {
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
    pub(super) fn validate(&self, store: &Store) -> Result<(), String> {
        if self.records.len() > 4096 {
            return Err("Team task evidence exceeds its bound.".into());
        }
        for (key, r) in &self.records {
            r.evidence.validate()?;
            if key != &r.evidence.receipt
                || r.fingerprint != r.evidence.fingerprint()
                || !store.workspaces.contains_key(&r.workspace)
                || !store.accounts.contains_key(&r.member)
                || !store.accounts.contains_key(&r.reviewer)
                || !text(&r.request)
                || r.attempt == 0
                || !r.account_revision.strip_prefix("sha256:").is_some_and(hash)
            {
                return Err("Invalid retained native team task attribution.".into());
            }
        }
        Ok(())
    }
}
impl Accounts {
    /// Bind current native service and rebuilt evidence under the membership writer.
    /// Exact retries return the original record without charging or adding work.
    pub fn report_attach(
        &self,
        evidence: Evidence,
        mut actor: impl FnMut(&Store) -> Result<MemberRef, String>,
        verify: impl FnOnce(&Store) -> Result<Verified, String>,
    ) -> Result<Record, String> {
        evidence.validate()?;
        let lock = Lock::acquire(&self.dir).map_err(|e| e.to_string())?;
        let (mut store, mut held) = self.team_state()?;
        let before = actor(&store)?;
        if !matches!(before.role, Role::Owner | Role::Admin) {
            return Err("Only a current workspace owner or admin attaches task evidence.".into());
        }
        let native = verify(&store)?;
        lock.check().map_err(|e| e.to_string())?;
        self.team_check_state(&held, &store.digest)?;
        if actor(&store)? != before || native.workspace != before.workspace {
            return Err("Current native reporting authority changed.".into());
        }
        let fingerprint = evidence.fingerprint();
        if let Some(old) = store.team_reports.records.get(&evidence.receipt) {
            if old.fingerprint != fingerprint
                || old.workspace != native.workspace
                || old.member != native.member
                || old.request != native.request
                || old.attempt != native.attempt
            {
                return Err("The original task evidence binding is immutable.".into());
            }
            return Ok(old.clone());
        }
        if store.team_reports.records.len() >= 4096 {
            return Err("Team task evidence is full.".into());
        }
        let record = Record {
            evidence,
            workspace: native.workspace,
            member: native.member,
            request: native.request,
            attempt: native.attempt,
            reviewer: before.account,
            account_revision: store.digest.clone(),
            recorded_at: unix_now(),
            fingerprint,
        };
        store
            .team_reports
            .records
            .insert(record.evidence.receipt.clone(), record.clone());
        self.team_commit(&lock, &mut held, &mut store)?;
        Ok(record)
    }
    /// Hold current native membership and custody through a bounded report read.
    pub fn report_read<T>(
        &self,
        mut actor: impl FnMut(&Store) -> Result<MemberRef, String>,
        read: impl FnOnce(&Store, &MemberRef) -> Result<T, String>,
    ) -> Result<T, String> {
        let lock = Lock::acquire(&self.dir).map_err(|e| e.to_string())?;
        let (store, held) = self.team_state()?;
        let before = actor(&store)?;
        let value = read(&store, &before)?;
        lock.check().map_err(|e| e.to_string())?;
        self.team_check_state(&held, &store.digest)?;
        if actor(&store)? != before {
            return Err("Current native reporting authority changed.".into());
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn member(store: &Store, workspace: &str, account: &str) -> MemberRef {
        let ws = &store.workspaces[workspace];
        let m = &ws.members[account];
        MemberRef {
            workspace: workspace.into(),
            account: account.into(),
            role: m.role,
            epoch: m.epoch,
            members_epoch: ws.members_epoch,
        }
    }
    #[test]
    fn evidence_is_owner_scoped_immutable_retry_safe_and_native_revision_bound() {
        let dir = tempfile::tempdir().unwrap();
        let accounts = Accounts::install(dir.path()).unwrap();
        let owner = accounts.create_account("fixture owner", &[]).unwrap();
        let colleague = accounts.create_account("fixture member", &[]).unwrap();
        let ws = accounts
            .create_workspace(
                &owner.id,
                "fixture",
                super::super::WorkspaceKind::Organization,
                "fixture-tenant",
                None,
            )
            .unwrap()
            .id;
        let invitation = accounts.invite(&owner.id, &ws, Role::Member, 3600).unwrap();
        accounts.accept(&colleague.id, &invitation.token).unwrap();
        let e = Evidence {
            schema: SCHEMA.into(),
            receipt: format!("sha256:{}", "a".repeat(64)),
            manifest: Reference {
                path: "manifest.json".into(),
                sha256: "b".repeat(64),
            },
            task: "fixture task".into(),
            candidate: "fixture candidate".into(),
        };
        let verify = || Verified {
            workspace: ws.clone(),
            member: colleague.id.clone(),
            request: "native-request".into(),
            attempt: 1,
        };
        assert!(
            accounts
                .report_attach(
                    e.clone(),
                    |s| Ok(member(s, &ws, &colleague.id)),
                    |_| Ok(verify())
                )
                .is_err()
        );
        let record = accounts
            .report_attach(
                e.clone(),
                |s| Ok(member(s, &ws, &owner.id)),
                |_| Ok(verify()),
            )
            .unwrap();
        let revision = accounts.store().unwrap().digest;
        let retry = accounts
            .report_attach(
                e.clone(),
                |s| Ok(member(s, &ws, &owner.id)),
                |_| Ok(verify()),
            )
            .unwrap();
        assert_eq!(record.fingerprint, retry.fingerprint);
        assert_eq!(revision, accounts.store().unwrap().digest);
        let mut changed = e.clone();
        changed.manifest.sha256 = "c".repeat(64);
        assert!(
            accounts
                .report_attach(changed, |s| Ok(member(s, &ws, &owner.id)), |_| Ok(verify()))
                .is_err()
        );
        let mut crossing = false;
        assert!(
            accounts
                .report_read(
                    |s| {
                        let mut m = member(s, &ws, &owner.id);
                        if crossing {
                            m.epoch += 1;
                        }
                        crossing = true;
                        Ok(m)
                    },
                    |_, _| Ok(())
                )
                .is_err()
        );
        let restarted = Accounts::open(dir.path()).unwrap();
        assert_eq!(restarted.store().unwrap().team_reports.records.len(), 1);
        let mut invalid = e;
        invalid.manifest.path = "../outside".into();
        assert!(invalid.validate().is_err());
    }
}

#[cfg(test)]
mod custody_tests {
    use super::*;
    #[test]
    fn replaced_native_account_state_cannot_finish_an_admitted_read() {
        crate::files_only!();
        let dir = tempfile::tempdir().unwrap();
        let accounts = Accounts::install(dir.path()).unwrap();
        let owner = accounts.create_account("fixture owner", &[]).unwrap();
        let ws = accounts
            .create_workspace(
                &owner.id,
                "fixture",
                super::super::WorkspaceKind::Personal,
                "fixture-tenant",
                None,
            )
            .unwrap()
            .id;
        let actor = |store: &Store| {
            let workspace = &store.workspaces[&ws];
            let m = &workspace.members[&owner.id];
            Ok(MemberRef {
                workspace: ws.clone(),
                account: owner.id.clone(),
                role: m.role,
                epoch: m.epoch,
                members_epoch: workspace.members_epoch,
            })
        };
        assert!(
            accounts
                .report_read(actor, |_, _| {
                    let path = dir.path().join("accounts.json");
                    let old = dir.path().join("old-state");
                    std::fs::rename(&path, &old).unwrap();
                    std::fs::copy(&old, &path).unwrap();
                    Ok(())
                })
                .is_err()
        );
    }
}
