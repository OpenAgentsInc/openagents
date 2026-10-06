//! Account-owned contact blocks and bounded reports, separate from host authority.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
pub type Id = [u8; 32];
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Spam,
    Harassment,
    UnsafeContent,
    Cheating,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Open,
    Actioned,
    Dismissed,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Block {
        account: u64,
        blocked: bool,
    },
    Report {
        account: u64,
        reason: Reason,
        evidence: Option<Id>,
    },
}
impl Action {
    pub fn target(&self) -> u64 {
        match self {
            Self::Block { account, .. } | Self::Report { account, .. } => *account,
        }
    }
    pub fn digest(&self) -> Result<Id, String> {
        if self.target() == 0
            || matches!(self, Self::Report { evidence: Some(id), .. } if *id == [0;32])
        {
            return Err("Invalid safety target or evidence identity".into());
        }
        let mut hash = Sha256::new();
        hash.update(b"verse.safety.action.v1\0");
        hash.update(serde_json::to_vec(self).map_err(|_| "Cannot encode safety action")?);
        Ok(hash.finalize().into())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Block { account: u64, blocked: bool },
    Report { id: Id },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub realm: Id,
    pub account: u64,
    pub operation: [u8; 16],
    pub digest: Id,
    pub outcome: Outcome,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub realm: Id,
    pub account: u64,
    pub blocked: Vec<u64>,
    pub contacts: Vec<Contact>,
}
/// Public safety address for a resident avatar; no key, inventory, or Studio data.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contact {
    pub life: verse_engine::core::LifeId,
    pub character: u64,
    pub account: u64,
}
impl View {
    pub fn validate(&self) -> Result<(), String> {
        if self.realm == [0; 32]
            || self.account == 0
            || self.blocked.len() > 64
            || self.blocked.iter().any(|a| *a == 0 || *a == self.account)
            || self.blocked.windows(2).any(|a| a[0] >= a[1])
            || self.contacts.len() > 128
            || self.contacts.iter().any(|c| {
                c.account == 0 || c.character == 0 || c.life.instance == 0 || c.life.actor == 0
            })
            || self
                .contacts
                .windows(2)
                .any(|c| c[0].life >= c[1].life || c[0].life.instance != c[1].life.instance)
            || self
                .contacts
                .iter()
                .map(|c| c.character)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.contacts.len()
        {
            return Err("Invalid account safety projection".into());
        }
        Ok(())
    }
}
/// Private operator report. The public wire exposes only its submission receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub id: Id,
    pub realm: Id,
    pub reporter: u64,
    pub target: u64,
    pub reason: Reason,
    pub evidence: Option<Id>,
    pub created_ms: u64,
    pub status: Status,
}
pub fn report_id(realm: Id, account: u64, operation: [u8; 16]) -> Id {
    let mut hash = Sha256::new();
    hash.update(b"verse.safety.report.v1\0");
    hash.update(realm);
    hash.update(account.to_be_bytes());
    hash.update(operation);
    hash.finalize().into()
}
impl Receipt {
    pub fn validate(
        &self,
        realm: Id,
        account: u64,
        operation: [u8; 16],
        action: &Action,
    ) -> Result<(), String> {
        let matches = match (&self.outcome, action) {
            (
                Outcome::Block {
                    account: target,
                    blocked,
                },
                Action::Block {
                    account: requested,
                    blocked: requested_blocked,
                },
            ) => target == requested && blocked == requested_blocked,
            (Outcome::Report { id }, Action::Report { .. }) => {
                *id == report_id(realm, account, operation)
            }
            _ => false,
        };
        if realm == [0; 32]
            || account == 0
            || operation == [0; 16]
            || self.realm != realm
            || self.account != account
            || self.operation != operation
            || self.digest != action.digest()?
            || !matches
        {
            return Err("Account safety receipt is incompatible".into());
        }
        Ok(())
    }
}
