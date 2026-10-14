//! Agent accounts (#11630): accounts an agent made for itself, with no
//! person and no email, through the gateway's `POST /v1/agents/register`.
//!
//! An agent account is an ordinary [`Account`] with one personal
//! workspace and one `oak_` key, plus an [`Agent`] record on the account
//! that marks it as an agent, never a person. The record carries the
//! digest of the claim secret the agent got at registration, so a person
//! can later claim the agent (link it to their own account), and whether
//! the agent was revoked, and by whom.
//!
//! Registration, claiming, and revocation are each one store mutation, so
//! an agent never exists half made.

use serde::{Deserialize, Serialize};

use super::{
    Account, Accounts, MemberStatus, Membership, Refusal, Role, Store, Trouble, Workspace,
    WorkspaceKind, digest_secret, fresh,
};
use std::collections::BTreeMap;

/// The claim token's wire prefix: `clm_<account id>.<secret>`.
pub const CLAIM_PREFIX: &str = "clm_";

/// The longest an agent's label may be, in bytes.
pub const MAX_LABEL: usize = 64;

/// The marker on an agent's account.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Agent {
    /// The agent's key id, the one `oak_` key registration issued.
    pub key: String,
    /// The agent's personal workspace.
    pub workspace: String,
    /// When the agent registered, RFC 3339 in UTC.
    pub registered: String,
    /// The SHA-256 of the claim secret, until a person claims the agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_digest: Option<String>,
    /// Unix seconds after which the claim secret no longer works.
    #[serde(default)]
    pub claim_expires_unix: u64,
    /// The person's account that claimed the agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimed_by: Option<String>,
    /// When it was claimed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claimed: Option<String>,
    /// When the agent was revoked; a revoked agent's key no longer works.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked: Option<String>,
    /// Who revoked it: a site admin's or the claiming person's account.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_by: Option<String>,
}

/// Why an agent operation was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentRefusal {
    /// The claim token isn't `clm_<account>.<secret>` shaped, names no
    /// agent, or its secret doesn't match.
    InvalidClaim,
    /// The claim secret's time ran out.
    ClaimExpired,
    /// Someone already claimed this agent.
    AlreadyClaimed,
    /// The agent was revoked.
    Revoked,
    /// An agent account can't claim agents.
    AgentCannotClaim,
    /// The account isn't an agent.
    NotAgent,
}

impl std::fmt::Display for AgentRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidClaim => "that claim code isn't valid",
            Self::ClaimExpired => "that claim code has expired",
            Self::AlreadyClaimed => "this agent was already claimed",
            Self::Revoked => "this agent was revoked",
            Self::AgentCannotClaim => "an agent account can't claim agents; sign in as a person",
            Self::NotAgent => "that account isn't an agent",
        })
    }
}

/// What registration made.
#[derive(Clone, Debug)]
pub struct Registered {
    pub account: Account,
    pub workspace: Workspace,
}

/// A claim token's account id and secret.
#[must_use]
pub fn split_claim(token: &str) -> Option<(&str, &str)> {
    let body = token.strip_prefix(CLAIM_PREFIX)?;
    let (account, secret) = body.split_once('.')?;
    let shaped = account.starts_with("acct_")
        && account.len() <= 64
        && account[5..].bytes().all(|b| b.is_ascii_hexdigit())
        && secret.len() == 64
        && secret.bytes().all(|b| b.is_ascii_hexdigit());
    shaped.then_some((account, secret))
}

/// A fresh claim secret: 64 hex characters.
///
/// # Errors
///
/// No randomness is available.
pub fn claim_secret() -> Result<String, Trouble> {
    fresh()
}

/// The digest the store keeps of a claim secret.
#[must_use]
pub fn claim_digest(secret: &str) -> String {
    digest_secret(secret)
}

/// Whether `label` is a name an agent may give itself: 1 to
/// [`MAX_LABEL`] bytes, printable, no control characters.
#[must_use]
pub fn valid_label(label: &str) -> bool {
    !label.trim().is_empty() && label.len() <= MAX_LABEL && !label.chars().any(char::is_control)
}

impl Store {
    /// The agent record on `account`, when it is an agent.
    #[must_use]
    pub fn agent(&self, account: &str) -> Option<&Agent> {
        self.accounts.get(account)?.agent.as_ref()
    }

    /// Every agent account, oldest first by id order.
    pub fn agents(&self) -> impl Iterator<Item = &Account> {
        self.accounts
            .values()
            .filter(|account| account.agent.is_some())
    }
}

impl Accounts {
    /// Make an agent account in one mutation: the account (holding the
    /// key `key_id` as its principal), its personal workspace bound to
    /// `tenant`, and the [`Agent`] record with the claim secret's digest.
    ///
    /// # Errors
    ///
    /// The label is empty or too long, the key is already held, or the
    /// store refused.
    pub fn register_agent(
        &self,
        label: &str,
        tenant: &str,
        key_id: &str,
        claim_digest: &str,
        claim_expires_unix: u64,
    ) -> Result<Registered, Refusal> {
        if !valid_label(label) {
            return Err(Refusal::EmptyField("name"));
        }
        if tenant.is_empty() {
            return Err(Refusal::EmptyField("tenant"));
        }
        let principal = format!("key:{key_id}");
        self.mutate(|store, _| {
            if let Some(other) = store
                .accounts
                .values()
                .find(|account| account.principals.contains(&principal))
            {
                return Err(Refusal::PrincipalTaken {
                    principal: principal.clone(),
                    account: other.id.clone(),
                });
            }
            let stamp = crate::registry::now_utc();
            let account_id = format!("acct_{}", &fresh().map_err(Refusal::Store)?[..16]);
            let workspace_id = format!("ws_{}", &fresh().map_err(Refusal::Store)?[..16]);
            let mut members = BTreeMap::new();
            members.insert(
                account_id.clone(),
                Membership {
                    account: account_id.clone(),
                    role: Role::Owner,
                    status: MemberStatus::Active,
                    epoch: 1,
                    granted: stamp.clone(),
                    revoked: None,
                },
            );
            let workspace = Workspace {
                id: workspace_id.clone(),
                kind: WorkspaceKind::Personal,
                name: label.to_string(),
                tenant: tenant.to_string(),
                seats: None,
                members_epoch: 1,
                members,
                created: stamp.clone(),
            };
            let account = Account {
                id: account_id.clone(),
                label: label.to_string(),
                principals: vec![principal.clone()],
                created: stamp.clone(),
                agent: Some(Agent {
                    key: key_id.to_string(),
                    workspace: workspace_id.clone(),
                    registered: stamp,
                    claim_digest: Some(claim_digest.to_string()),
                    claim_expires_unix,
                    claimed_by: None,
                    claimed: None,
                    revoked: None,
                    revoked_by: None,
                }),
            };
            store.accounts.insert(account_id, account.clone());
            store
                .referrals
                .inherit_workspace(&workspace)
                .map_err(|error| Refusal::Store(Trouble::Invalid(error.to_string())))?;
            store.workspaces.insert(workspace_id, workspace.clone());
            Ok(Registered { account, workspace })
        })
    }

    /// A person claims an agent with the claim token the agent was given
    /// at registration. The secret works once; the agent is then linked to
    /// `person`, who can see and revoke it.
    ///
    /// # Errors
    ///
    /// The token is malformed, wrong, expired, or used; the person is an
    /// agent; the agent was revoked; or the store refused.
    pub fn claim_agent(
        &self,
        person: &str,
        token: &str,
        now_unix: u64,
    ) -> Result<Account, Refusal> {
        let (agent_id, secret) =
            split_claim(token).ok_or(Refusal::Agent(AgentRefusal::InvalidClaim))?;
        let presented = digest_secret(secret);
        self.mutate(|store, _| {
            let claimer = store
                .accounts
                .get(person)
                .ok_or_else(|| Refusal::UnknownAccount(person.to_string()))?;
            if claimer.agent.is_some() {
                return Err(Refusal::Agent(AgentRefusal::AgentCannotClaim));
            }
            let account = store
                .accounts
                .get_mut(agent_id)
                .ok_or(Refusal::Agent(AgentRefusal::InvalidClaim))?;
            let agent = account
                .agent
                .as_mut()
                .ok_or(Refusal::Agent(AgentRefusal::InvalidClaim))?;
            if agent.claimed_by.is_some() {
                return Err(Refusal::Agent(AgentRefusal::AlreadyClaimed));
            }
            let Some(expected) = agent.claim_digest.as_deref() else {
                return Err(Refusal::Agent(AgentRefusal::InvalidClaim));
            };
            if !constant_eq(expected.as_bytes(), presented.as_bytes()) {
                return Err(Refusal::Agent(AgentRefusal::InvalidClaim));
            }
            if agent.revoked.is_some() {
                return Err(Refusal::Agent(AgentRefusal::Revoked));
            }
            if now_unix > agent.claim_expires_unix {
                return Err(Refusal::Agent(AgentRefusal::ClaimExpired));
            }
            agent.claim_digest = None;
            agent.claimed_by = Some(person.to_string());
            agent.claimed = Some(crate::registry::now_utc());
            Ok(account.clone())
        })
    }

    /// Mark an agent revoked by `by`. The caller revokes its key in the
    /// key store too; this records who and when. Revoking twice keeps the
    /// first record.
    ///
    /// # Errors
    ///
    /// The account isn't an agent, or the store refused.
    pub fn revoke_agent(&self, agent_id: &str, by: &str) -> Result<Account, Refusal> {
        self.mutate(|store, _| {
            let account = store
                .accounts
                .get_mut(agent_id)
                .ok_or_else(|| Refusal::UnknownAccount(agent_id.to_string()))?;
            let agent = account
                .agent
                .as_mut()
                .ok_or(Refusal::Agent(AgentRefusal::NotAgent))?;
            if agent.revoked.is_none() {
                agent.revoked = Some(crate::registry::now_utc());
                agent.revoked_by = Some(by.to_string());
                agent.claim_digest = None;
            }
            Ok(account.clone())
        })
    }
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed() -> (tempfile::TempDir, Accounts) {
        let dir = tempfile::tempdir().unwrap();
        let accounts = Accounts::install(dir.path()).unwrap();
        (dir, accounts)
    }

    #[test]
    fn an_agent_registers_is_marked_and_is_claimed_once() {
        let (_dir, accounts) = installed();
        let secret = claim_secret().unwrap();
        let made = accounts
            .register_agent(
                "scout",
                "signup",
                "0123456789abcdef",
                &claim_digest(&secret),
                2_000_000_000,
            )
            .unwrap();
        let store = accounts.store().unwrap();
        let agent = store.agent(&made.account.id).unwrap();
        assert_eq!(agent.key, "0123456789abcdef");
        assert_eq!(agent.workspace, made.workspace.id);
        assert_eq!(store.agents().count(), 1);
        assert_eq!(
            store.key_owner("0123456789abcdef"),
            Some(made.account.id.as_str())
        );

        let person = accounts.create_account("Ada", &[]).unwrap();
        let token = format!("{CLAIM_PREFIX}{}.{secret}", made.account.id);
        let wrong = format!("{CLAIM_PREFIX}{}.{}", made.account.id, "0".repeat(64));
        assert!(matches!(
            accounts.claim_agent(&person.id, &wrong, 1),
            Err(Refusal::Agent(AgentRefusal::InvalidClaim))
        ));
        assert!(matches!(
            accounts.claim_agent(&person.id, &token, 2_000_000_001),
            Err(Refusal::Agent(AgentRefusal::ClaimExpired))
        ));
        // An agent can't claim another agent.
        let other_secret = claim_secret().unwrap();
        let other = accounts
            .register_agent(
                "other",
                "signup",
                "fedcba9876543210",
                &claim_digest(&other_secret),
                2_000_000_000,
            )
            .unwrap();
        assert!(matches!(
            accounts.claim_agent(&other.account.id, &token, 1),
            Err(Refusal::Agent(AgentRefusal::AgentCannotClaim))
        ));
        let claimed = accounts.claim_agent(&person.id, &token, 1).unwrap();
        assert_eq!(
            claimed.agent.as_ref().unwrap().claimed_by.as_deref(),
            Some(person.id.as_str())
        );
        assert!(matches!(
            accounts.claim_agent(&person.id, &token, 1),
            Err(Refusal::Agent(AgentRefusal::AlreadyClaimed))
        ));
        let revoked = accounts.revoke_agent(&made.account.id, &person.id).unwrap();
        assert!(revoked.agent.unwrap().revoked.is_some());
        assert!(matches!(
            accounts.revoke_agent(&person.id, &person.id),
            Err(Refusal::Agent(AgentRefusal::NotAgent))
        ));
    }

    #[test]
    fn claim_tokens_and_labels_are_shaped() {
        let secret = "a".repeat(64);
        assert!(split_claim(&format!("clm_acct_0123456789abcdef.{secret}")).is_some());
        assert!(split_claim("clm_acct_01.short").is_none());
        assert!(split_claim(&format!("clm_ws_0123.{secret}")).is_none());
        assert!(valid_label("research bot"));
        assert!(!valid_label(" "));
        assert!(!valid_label(&"x".repeat(65)));
        assert!(!valid_label("bad\nlabel"));
    }

    #[test]
    fn a_store_without_agents_keeps_its_digest() {
        let (_dir, accounts) = installed();
        let person = accounts.create_account("Ada", &[]).unwrap();
        let text = serde_json::to_string(&accounts.store().unwrap().accounts[&person.id]).unwrap();
        assert!(!text.contains("agent"), "{text}");
    }
}
