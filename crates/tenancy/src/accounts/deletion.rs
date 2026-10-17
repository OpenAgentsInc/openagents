//! Deleting an account (#12002, App Store guideline 5.1.1(v)).
//!
//! The record stays as a tombstone, because workspaces, invitations, and
//! the other books name accounts by id and the store refuses a dangling
//! reference. Everything that identifies or reaches the person goes: the
//! label becomes [`DELETED_LABEL`], every principal is dropped (so a key,
//! a GitHub or Apple identity, or a Nostr key can make a new account
//! later), the linked identities and their profiles are removed, the
//! account leaves every workspace it doesn't own, and its pending
//! invitations are withdrawn. A workspace it owns keeps the tombstone as
//! its owner, with no way left to reach it.
//!
//! An owner of a workspace that still has other active members is refused
//! with [`Refusal::LastOwner`]: ownership moves to another member first.

use super::{Accounts, InviteStatus, MemberStatus, Refusal, Role};

/// The label a deleted account keeps.
pub const DELETED_LABEL: &str = "Deleted account";

/// What [`Accounts::delete_account`] did.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Deleted {
    /// The principals the account held, so the caller can revoke the
    /// credentials they name (`key:<id>` bearer keys).
    pub principals: Vec<String>,
    /// The workspaces the account owned and still owns as a tombstone.
    pub owned: Vec<String>,
    /// The workspaces the account left.
    pub left: Vec<String>,
}

impl Accounts {
    /// The workspaces `account` owns that have other active members. A
    /// deletion is refused while there are any.
    pub fn shared_owned(&self, account: &str) -> Result<Vec<String>, super::Trouble> {
        let store = self.store()?;
        Ok(store
            .workspaces
            .values()
            .filter(|ws| {
                ws.members
                    .get(account)
                    .is_some_and(|m| m.status == MemberStatus::Active && m.role == Role::Owner)
                    && ws
                        .members
                        .values()
                        .any(|m| m.account != account && m.status == MemberStatus::Active)
            })
            .map(|ws| ws.id.clone())
            .collect())
    }

    /// Delete `account` as the module documentation describes. Deleting
    /// an account already deleted changes nothing more and succeeds.
    pub fn delete_account(&self, account: &str) -> Result<Deleted, Refusal> {
        self.mutate(|store, _| {
            let Some(record) = store.accounts.get(account) else {
                return Err(Refusal::UnknownAccount(account.to_string()));
            };
            let mut deleted = Deleted {
                principals: record.principals.clone(),
                ..Deleted::default()
            };
            let now = crate::registry::now_utc();
            for ws in store.workspaces.values_mut() {
                let Some(membership) = ws.members.get(account) else {
                    continue;
                };
                if membership.status != MemberStatus::Active {
                    continue;
                }
                if membership.role == Role::Owner {
                    let others = ws
                        .members
                        .values()
                        .any(|m| m.account != account && m.status == MemberStatus::Active);
                    if others {
                        return Err(Refusal::LastOwner {
                            workspace: ws.id.clone(),
                        });
                    }
                    deleted.owned.push(ws.id.clone());
                    continue;
                }
                ws.members_epoch += 1;
                let epoch = ws.members_epoch;
                let record = ws.members.get_mut(account).unwrap();
                record.status = MemberStatus::Revoked;
                record.epoch = epoch;
                record.revoked = Some(now.clone());
                deleted.left.push(ws.id.clone());
            }
            for invitation in store.invitations.values_mut() {
                if invitation.invited_by == account && invitation.status == InviteStatus::Pending {
                    invitation.status = InviteStatus::Revoked;
                }
            }
            store.identities.github.retain(|_, i| i.account != account);
            store.identities.apple.retain(|_, i| i.account != account);
            let record = store.accounts.get_mut(account).unwrap();
            record.principals.clear();
            record.label = DELETED_LABEL.to_string();
            Ok(deleted)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::WorkspaceKind;

    fn installed() -> (tempfile::TempDir, Accounts) {
        let dir = tempfile::tempdir().unwrap();
        let accounts = Accounts::install(dir.path()).unwrap();
        (dir, accounts)
    }

    #[test]
    fn a_deleted_account_keeps_no_principal_name_or_membership() {
        let (_dir, accounts) = installed();
        let ada = accounts
            .create_account("Ada", &["key:aaaaaaaaaaaaaaaa".into()])
            .unwrap();
        let bo = accounts.create_account("Bo", &[]).unwrap();
        let own = accounts
            .create_workspace(&ada.id, "Ada", WorkspaceKind::Personal, "signup", None)
            .unwrap();
        let team = accounts
            .create_workspace(&bo.id, "Team", WorkspaceKind::Organization, "acme", None)
            .unwrap();
        let invited = accounts.invite(&bo.id, &team.id, Role::Admin, 600).unwrap();
        accounts.accept(&ada.id, &invited.token).unwrap();
        let pending = accounts
            .invite(&ada.id, &team.id, Role::Member, 600)
            .unwrap();

        let deleted = accounts.delete_account(&ada.id).unwrap();
        assert_eq!(deleted.principals, vec!["key:aaaaaaaaaaaaaaaa".to_string()]);
        assert_eq!(deleted.owned, vec![own.id.clone()]);
        assert_eq!(deleted.left, vec![team.id.clone()]);

        let store = accounts.store().unwrap();
        let record = &store.accounts[&ada.id];
        assert_eq!(record.label, DELETED_LABEL);
        assert!(record.principals.is_empty());
        assert_eq!(
            store.workspaces[&team.id].members[&ada.id].status,
            MemberStatus::Revoked
        );
        assert_eq!(
            store.invitations[&pending.invitation.id].status,
            InviteStatus::Revoked
        );
        assert_eq!(
            accounts
                .account_of_principal("key:aaaaaaaaaaaaaaaa")
                .unwrap(),
            None
        );
        // The key can name a new account now.
        accounts
            .create_account("Ada again", &["key:aaaaaaaaaaaaaaaa".into()])
            .unwrap();
        // A second deletion changes nothing more.
        assert!(
            accounts
                .delete_account(&ada.id)
                .unwrap()
                .principals
                .is_empty()
        );
    }

    #[test]
    fn the_owner_of_a_team_with_members_is_refused() {
        let (_dir, accounts) = installed();
        let ada = accounts.create_account("Ada", &[]).unwrap();
        let bo = accounts.create_account("Bo", &[]).unwrap();
        let team = accounts
            .create_workspace(&ada.id, "Team", WorkspaceKind::Organization, "acme", None)
            .unwrap();
        let invited = accounts
            .invite(&ada.id, &team.id, Role::Member, 600)
            .unwrap();
        accounts.accept(&bo.id, &invited.token).unwrap();
        assert_eq!(
            accounts.shared_owned(&ada.id).unwrap(),
            vec![team.id.clone()]
        );
        assert!(matches!(
            accounts.delete_account(&ada.id),
            Err(Refusal::LastOwner { .. })
        ));
        assert_eq!(accounts.store().unwrap().accounts[&ada.id].label, "Ada");
    }
}
