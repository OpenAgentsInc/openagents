//! Accounts made by a WorkOS sign-in (#12323): a person who signs up with
//! email, GitHub, Google, or SSO through WorkOS and matches no account we
//! hold gets a new account and its personal workspace here. The link to
//! the WorkOS User is kept in [`crate::workos_links`], not on the account.

use super::identities::personal_workspace;
use super::nostr::NostrSignIn;
use super::{Account, Refusal, Trouble, fresh};

impl super::Accounts {
    /// Make an account labelled `label` with no principals, and its
    /// personal workspace bound to `tenant`, in one write. The answer has
    /// the same shape a Nostr sign-up gives.
    pub fn sign_up_workos(&self, label: &str, tenant: &str) -> Result<NostrSignIn, Refusal> {
        if tenant.is_empty() {
            return Err(Refusal::EmptyField("tenant"));
        }
        let label = if label.trim().is_empty() {
            "OpenAgents user"
        } else {
            label.trim()
        };
        self.mutate(|store, _| {
            let stamp = crate::registry::now_utc();
            let account = Account {
                id: format!("acct_{}", &fresh().map_err(Refusal::Store)?[..16]),
                label: label.chars().take(120).collect(),
                principals: Vec::new(),
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
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_workos_sign_up_makes_an_account_and_its_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let accounts = crate::Accounts::install(dir.path()).unwrap();
        let made = accounts.sign_up_workos("Ada Lovelace", "signup").unwrap();
        assert!(made.created);
        assert!(made.account.principals.is_empty());
        assert_eq!(made.account.label, "Ada Lovelace");
        let workspace = made.workspace.unwrap();
        assert!(accounts.authorize(&workspace.id, &made.account.id).is_ok());
        let unnamed = accounts.sign_up_workos("  ", "signup").unwrap();
        assert_eq!(unnamed.account.label, "OpenAgents user");
    }
}
