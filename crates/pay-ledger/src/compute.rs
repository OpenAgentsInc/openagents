//! The purchased compute balance: one customer account that every client
//! (the native window, the Grid workshop, the CLI, an API key, a phone)
//! resolves to, kept in this ledger beside the settlements it pays
//! (`docs/cloud/retail-contract.md`, `docs/cloud/compute-balance.md`).
//!
//! A principal is one client's binding to an account. It stores a SHA-256
//! digest of the client's credential, never the credential, and two rights
//! of its own: reading the balance and spending it. Reading and spending
//! are the only rights this book holds. Pairing a device, joining a world,
//! and executing on a computer are granted elsewhere and never imply either
//! right here, and holding a balance grants no execution.
//!
//! Amounts are integer millisatoshis, as in the rest of the ledger.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use crate::{Error, Ledger, Result};

pub mod purchase;

pub use purchase::{Purchase, PurchaseState, Receipt, TopUp};

/// The kind of client a principal binds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalKind {
    /// The native terminal window.
    Window,
    /// The Grid workshop in Verse.
    Workshop,
    /// The `openagents` CLI.
    Cli,
    /// An HTTP API key.
    ApiKey,
    /// A phone.
    Phone,
}

impl PrincipalKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Workshop => "workshop",
            Self::Cli => "cli",
            Self::ApiKey => "api_key",
            Self::Phone => "phone",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "window" => Self::Window,
            "workshop" => Self::Workshop,
            "cli" => Self::Cli,
            "api_key" => Self::ApiKey,
            "phone" => Self::Phone,
            _ => return None,
        })
    }
}

/// What a principal may do with its account's balance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rights {
    pub read: bool,
    pub spend: bool,
}

/// What a caller asks a principal for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    Read,
    Spend,
}

/// A request to bind one client to an account.
#[derive(Debug, Clone)]
pub struct Binding {
    /// Stable for the client: `window:<host key>`, `key:<key id>`, ...
    pub principal: String,
    pub account: String,
    pub kind: PrincipalKind,
    /// The SHA-256 of the client's credential ([`credential_digest`]).
    pub credential: String,
    pub rights: Rights,
    pub at: i64,
}

/// A principal as the ledger keeps it. It holds no secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    pub id: String,
    pub account: String,
    pub kind: PrincipalKind,
    pub generation: i64,
    pub rights: Rights,
    pub bound_at: i64,
    pub revoked_at: Option<i64>,
}

/// The digest a principal's credential is stored and compared as.
#[must_use]
pub fn credential_digest(secret: &str) -> String {
    crate::digest(secret)
}

fn nonempty(text: &str, what: &'static str) -> Result<()> {
    if text.is_empty() || text.len() > 256 {
        return Err(Error::Invalid(what));
    }
    Ok(())
}

fn is_digest(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Ledger {
    /// Create the compute account `id`, or return it unchanged when it
    /// exists.
    pub fn create_compute_account(&mut self, id: &str, at: i64) -> Result<()> {
        nonempty(id, "account id")?;
        self.connection.execute(
            "INSERT INTO compute_account(id,created_at) VALUES(?,?) ON CONFLICT(id) DO NOTHING",
            params![id, at],
        )?;
        Ok(())
    }

    /// Bind a client to an account. Binding the same principal again with
    /// the same account, kind, credential, and rights returns it unchanged;
    /// anything else conflicts, because a credential changes only through
    /// [`Ledger::rotate_principal`] and an account never changes.
    pub fn bind_principal(&mut self, binding: &Binding) -> Result<Principal> {
        nonempty(&binding.principal, "principal id")?;
        if !is_digest(&binding.credential) {
            return Err(Error::Invalid(
                "a credential is bound by its SHA-256 digest",
            ));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM compute_account WHERE id=?)",
            [&binding.account],
            |r| r.get(0),
        )?;
        if !exists {
            return Err(Error::Invalid("no such compute account"));
        }
        if let Some((principal, credential)) = read_principal(&tx, &binding.principal)? {
            if principal.account == binding.account
                && principal.kind == binding.kind
                && principal.rights == binding.rights
                && principal.revoked_at.is_none()
                && credential == binding.credential
            {
                return Ok(principal);
            }
            return Err(Error::Conflict("the principal is bound with other terms"));
        }
        tx.execute(
            "INSERT INTO compute_principal(id,account,kind,credential,generation,can_read,can_spend,bound_at) VALUES(?,?,?,?,1,?,?,?)",
            params![
                binding.principal,
                binding.account,
                binding.kind.as_str(),
                binding.credential,
                binding.rights.read,
                binding.rights.spend,
                binding.at
            ],
        )?;
        let principal = read_principal(&tx, &binding.principal)?
            .ok_or(Error::Invalid("missing principal"))?
            .0;
        tx.commit()?;
        Ok(principal)
    }

    /// Replace a principal's credential. The old credential stops
    /// resolving at once; the generation moves on.
    pub fn rotate_principal(&mut self, principal: &str, credential: &str) -> Result<Principal> {
        if !is_digest(credential) {
            return Err(Error::Invalid(
                "a credential is bound by its SHA-256 digest",
            ));
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (current, _) =
            read_principal(&tx, principal)?.ok_or(Error::Denied("unknown principal"))?;
        if current.revoked_at.is_some() {
            return Err(Error::Denied("the principal is revoked"));
        }
        tx.execute(
            "UPDATE compute_principal SET credential=?, generation=generation+1 WHERE id=?",
            params![credential, principal],
        )?;
        let rotated = read_principal(&tx, principal)?
            .ok_or(Error::Invalid("missing principal"))?
            .0;
        tx.commit()?;
        Ok(rotated)
    }

    /// Revoke a principal. Revocation is final; the client binds again
    /// under a new principal id.
    pub fn revoke_principal(&mut self, principal: &str, at: i64) -> Result<Principal> {
        self.connection.execute(
            "UPDATE compute_principal SET revoked_at=? WHERE id=? AND revoked_at IS NULL",
            params![at, principal],
        )?;
        read_principal(&self.connection, principal)?
            .map(|(p, _)| p)
            .ok_or(Error::Denied("unknown principal"))
    }

    /// The account `principal` may read or spend, given the client's
    /// credential digest.
    ///
    /// # Errors
    ///
    /// [`Error::Denied`] for an unknown or revoked principal, a credential
    /// that does not match the current generation, or a missing right.
    pub fn resolve_principal(
        &self,
        principal: &str,
        credential: &str,
        need: Need,
    ) -> Result<Principal> {
        let (found, stored) = read_principal(&self.connection, principal)?
            .ok_or(Error::Denied("unknown principal"))?;
        if found.revoked_at.is_some() {
            return Err(Error::Denied("the principal is revoked"));
        }
        if stored != credential {
            return Err(Error::Denied("the credential does not match"));
        }
        let allowed = match need {
            Need::Read => found.rights.read,
            Need::Spend => found.rights.spend,
        };
        if !allowed {
            return Err(Error::Denied("the principal lacks this right"));
        }
        Ok(found)
    }

    /// Every principal bound to `account`, oldest first.
    pub fn principals(&self, account: &str) -> Result<Vec<Principal>> {
        let mut statement = self
            .connection
            .prepare("SELECT id FROM compute_principal WHERE account=? ORDER BY bound_at, id")?;
        let ids = statement
            .query_map([account], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.iter()
            .map(|id| {
                read_principal(&self.connection, id)?
                    .map(|(p, _)| p)
                    .ok_or(Error::Invalid("missing principal"))
            })
            .collect()
    }
}

fn read_principal(connection: &Connection, id: &str) -> Result<Option<(Principal, String)>> {
    let row = connection
        .query_row(
            "SELECT id,account,kind,credential,generation,can_read,can_spend,bound_at,revoked_at FROM compute_principal WHERE id=?",
            [id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, bool>(5)?,
                    r.get::<_, bool>(6)?,
                    r.get::<_, i64>(7)?,
                    r.get::<_, Option<i64>>(8)?,
                ))
            },
        )
        .optional()?;
    let Some((id, account, kind, credential, generation, read, spend, bound_at, revoked_at)) = row
    else {
        return Ok(None);
    };
    let kind = PrincipalKind::parse(&kind).ok_or(Error::Invalid("principal kind"))?;
    Ok(Some((
        Principal {
            id,
            account,
            kind,
            generation,
            rights: Rights { read, spend },
            bound_at,
            revoked_at,
        },
        credential,
    )))
}
