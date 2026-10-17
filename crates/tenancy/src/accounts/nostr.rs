//! Nostr keys as sign-in identities (#11993).
//!
//! A Nostr key is the account: the account service verifies a signed
//! NIP-98 event (in `oa-auth`) and hands the verified public key here.
//! The key is held as the principal `nostr:<64 lowercase hex>` on the
//! account, and an account may hold several (one per device, or one key
//! the person uses everywhere). Nothing else is kept: there is no profile
//! to refresh and no secret.

use super::identities::personal_workspace;
use super::{Account, Refusal, Store, Trouble, Workspace, fresh};

/// The prefix of a Nostr key principal.
const PREFIX: &str = "nostr:";

/// The most Nostr keys one account holds.
pub const KEYS_PER_ACCOUNT: usize = 32;

/// Whether `pubkey` is a Nostr public key as this book stores it: 64
/// lowercase hex characters (a BIP-340 x-only key).
#[must_use]
pub fn valid_pubkey(pubkey: &str) -> bool {
    pubkey.len() == 64
        && pubkey
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The principal reference for a Nostr public key.
#[must_use]
pub fn nostr_principal(pubkey: &str) -> String {
    format!("{PREFIX}{pubkey}")
}

/// The Nostr public keys an account holds, in the order they were added.
#[must_use]
pub fn keys_of(account: &Account) -> Vec<String> {
    account
        .principals
        .iter()
        .filter_map(|p| p.strip_prefix(PREFIX))
        .map(str::to_string)
        .collect()
}

/// What a Nostr sign-in resolved to.
#[derive(Clone, Debug)]
pub struct NostrSignIn {
    pub account: Account,
    /// The personal workspace, when this sign-in created the account.
    pub workspace: Option<Workspace>,
    /// Whether this sign-in created the account.
    pub created: bool,
}

fn checked(pubkey: &str) -> Result<String, Refusal> {
    if valid_pubkey(pubkey) {
        Ok(nostr_principal(pubkey))
    } else {
        Err(Refusal::Store(Trouble::Invalid(
            "a Nostr key must be 64 lowercase hex characters".into(),
        )))
    }
}

fn holder<'a>(store: &'a Store, principal: &str) -> Option<&'a Account> {
    store
        .accounts
        .values()
        .find(|account| account.principals.iter().any(|p| p == principal))
}

impl super::Accounts {
    /// Sign in with a verified Nostr key: find the account that holds
    /// `nostr:<pubkey>`, or create the account, its personal workspace
    /// bound to `tenant`, and the principal, in one write.
    pub fn sign_in_nostr(&self, pubkey: &str, tenant: &str) -> Result<NostrSignIn, Refusal> {
        let principal = checked(pubkey)?;
        if tenant.is_empty() {
            return Err(Refusal::EmptyField("tenant"));
        }
        self.mutate(|store, _| {
            if let Some(account) = holder(store, &principal) {
                return Ok(NostrSignIn {
                    account: account.clone(),
                    workspace: None,
                    created: false,
                });
            }
            let stamp = crate::registry::now_utc();
            let account = Account {
                id: format!("acct_{}", &fresh().map_err(Refusal::Store)?[..16]),
                label: "OpenAgents user".to_string(),
                principals: vec![principal.clone()],
                created: stamp.clone(),
                agent: None,
            };
            let workspace = personal_workspace(&account, tenant, &stamp)?;
            store
                .referrals
                .inherit_workspace(&workspace)
                .map_err(|error| Refusal::Store(Trouble::Invalid(error.to_string())))?;
            store.accounts.insert(account.id.clone(), account.clone());
            store
                .workspaces
                .insert(workspace.id.clone(), workspace.clone());
            Ok(NostrSignIn {
                account,
                workspace: Some(workspace),
                created: true,
            })
        })
    }

    /// Add a Nostr key to `account`. Adding a key the account already
    /// holds changes nothing; a key another account holds is refused
    /// with [`Refusal::PrincipalTaken`].
    pub fn link_nostr(&self, account: &str, pubkey: &str) -> Result<Account, Refusal> {
        let principal = checked(pubkey)?;
        self.mutate(|store, _| {
            if let Some(other) = holder(store, &principal) {
                if other.id == account {
                    return Ok(other.clone());
                }
                return Err(Refusal::PrincipalTaken {
                    principal: principal.clone(),
                    account: other.id.clone(),
                });
            }
            let record = store
                .accounts
                .get_mut(account)
                .ok_or_else(|| Refusal::UnknownAccount(account.to_string()))?;
            if keys_of(record).len() >= KEYS_PER_ACCOUNT {
                return Err(Refusal::Store(Trouble::Invalid(format!(
                    "an account holds at most {KEYS_PER_ACCOUNT} Nostr keys"
                ))));
            }
            record.principals.push(principal.clone());
            Ok(record.clone())
        })
    }

    /// Remove a Nostr key from `account`. Refused with
    /// [`Refusal::LastCredential`] when it is the account's last one, and
    /// with [`Refusal::UnknownPrincipal`] when the account doesn't hold it.
    pub fn unlink_nostr(&self, account: &str, pubkey: &str) -> Result<Account, Refusal> {
        let principal = checked(pubkey)?;
        self.mutate(|store, _| {
            let record = store
                .accounts
                .get_mut(account)
                .ok_or_else(|| Refusal::UnknownAccount(account.to_string()))?;
            if !record.principals.contains(&principal) {
                return Err(Refusal::UnknownPrincipal(principal.clone()));
            }
            if keys_of(record).len() <= 1 {
                return Err(Refusal::LastCredential(account.to_string()));
            }
            record.principals.retain(|p| *p != principal);
            Ok(record.clone())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Accounts, WorkspaceKind};

    const A: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
    const B: &str = "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";

    #[test]
    fn first_sign_in_creates_the_account_once_and_the_second_finds_it() {
        let dir = tempfile::tempdir().unwrap();
        let accounts = Accounts::install(dir.path()).unwrap();
        let first = accounts.sign_in_nostr(A, "signup").unwrap();
        assert!(first.created);
        assert_eq!(first.account.principals, vec![nostr_principal(A)]);
        let workspace = first.workspace.unwrap();
        assert_eq!(workspace.kind, WorkspaceKind::Personal);
        assert_eq!(workspace.tenant, "signup");
        let second = accounts.sign_in_nostr(A, "signup").unwrap();
        assert!(!second.created);
        assert_eq!(second.account.id, first.account.id);
        let store = Accounts::open(dir.path()).unwrap().store().unwrap();
        assert_eq!(store.accounts.len(), 1);
        assert_eq!(store.workspaces.len(), 1);
    }

    #[test]
    fn keys_are_refused_unless_lowercase_hex() {
        let dir = tempfile::tempdir().unwrap();
        let accounts = Accounts::install(dir.path()).unwrap();
        assert!(accounts.sign_in_nostr(&A.to_uppercase(), "signup").is_err());
        assert!(accounts.sign_in_nostr(&A[..63], "signup").is_err());
        assert!(accounts.sign_in_nostr(A, "").is_err());
    }

    #[test]
    fn a_key_links_once_never_to_two_accounts_and_the_last_stays() {
        let dir = tempfile::tempdir().unwrap();
        let accounts = Accounts::install(dir.path()).unwrap();
        let mine = accounts.sign_in_nostr(A, "signup").unwrap().account;
        let theirs = accounts.sign_in_nostr(B, "signup").unwrap().account;
        assert!(matches!(
            accounts.link_nostr(&mine.id, B),
            Err(Refusal::PrincipalTaken { .. })
        ));
        assert!(matches!(
            accounts.unlink_nostr(&mine.id, A),
            Err(Refusal::LastCredential(_))
        ));
        // An account made another way (here, an API key) adds its first key.
        let older = accounts
            .create_account("Pat", &["key:0000000000000000".to_string()])
            .unwrap();
        let linked = accounts.link_nostr(&older.id, &"c".repeat(64)).unwrap();
        assert_eq!(keys_of(&linked), vec!["c".repeat(64)]);
        assert_eq!(
            accounts
                .link_nostr(&older.id, &"c".repeat(64))
                .unwrap()
                .principals
                .len(),
            2
        );
        let second = accounts.link_nostr(&mine.id, &"d".repeat(64)).unwrap();
        assert_eq!(keys_of(&second), vec![A.to_string(), "d".repeat(64)]);
        let left = accounts.unlink_nostr(&mine.id, A).unwrap();
        assert_eq!(keys_of(&left), vec!["d".repeat(64)]);
        assert!(matches!(
            accounts.unlink_nostr(&mine.id, B),
            Err(Refusal::UnknownPrincipal(_))
        ));
        assert_eq!(
            accounts
                .account_of_principal(&nostr_principal(B))
                .unwrap()
                .as_deref(),
            Some(theirs.id.as_str())
        );
    }
}
