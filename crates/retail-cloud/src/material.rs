//! Delivering only admitted source and customer credentials (#10712).
//!
//! A retail sandbox receives exactly two things: the admitted public source
//! at its exact commit, and the customer's own provider key for the admitted
//! model payer. Nothing else is read to fill a gap: no owner or operator
//! credential, no Secret Manager entry, no environment variable. The key is
//! written to one file under `/tmp` with mode 0600, never placed on a command
//! line, in the environment, a log, a manifest, or an artifact, and removed
//! at teardown. The journal records the recipient, the payer, and the key's
//! digest, never the key.
//!
//! [`Sandbox`] is the seam to the provisioned computer;
//! [`crate::fake::FakeSandbox`] simulates it.

use route_contract::snapshot::Payer;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::authority::{self, Current, Source, Step};
use crate::journal::Journal;
use crate::offer::FundedRequest;
use crate::{Error, Result, sha256_hex};

pub(crate) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS material (
    execution TEXT PRIMARY KEY,
    resource TEXT NOT NULL,
    source TEXT NOT NULL,
    payer TEXT NOT NULL,
    key_digest TEXT NOT NULL,
    key_path TEXT NOT NULL,
    delivered_at INTEGER NOT NULL,
    removed_at INTEGER
);
";

/// A customer's secret. It cannot be printed, serialized, or cloned by
/// accident.
pub struct CustomerSecret(String);

impl CustomerSecret {
    #[must_use]
    pub fn new(secret: String) -> Self {
        Self(secret)
    }

    /// The secret's digest, which is all the journal keeps.
    #[must_use]
    pub fn digest(&self) -> String {
        sha256_hex(self.0.as_bytes())
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for CustomerSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CustomerSecret(redacted)")
    }
}

impl Drop for CustomerSecret {
    fn drop(&mut self) {
        // Overwrite the bytes before the allocation is freed.
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.fill(0);
    }
}

/// How the customer supplies model access.
#[derive(Debug)]
pub enum Credential {
    /// The customer's own provider API key.
    ApiKey {
        provider: String,
        secret: CustomerSecret,
    },
    /// The customer's own engine login (a ChatGPT or Claude session).
    OwnLogin { engine: String },
    /// A provider OpenAgents pays.
    PaidProvider { provider: String },
}

/// Why material was not delivered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum MaterialRefusal {
    /// A login cannot leave the customer's own computer in v1.
    OwnLoginExport,
    /// OpenAgents-paid model access is not part of the v1 class.
    PaidProviderUnsupported,
    /// The credential is for another provider than the admitted payer: a
    /// new offer is needed.
    ProviderChanged,
    /// The source differs from the admitted one: a new offer is needed.
    SourceChanged,
    /// The clone's `HEAD` is not the admitted commit, or the tree is dirty.
    SourceUnverified,
    /// The sandbox cannot keep a file private to the task.
    IsolationUnsupported,
}

/// The seam to a provisioned sandbox.
pub trait Sandbox {
    /// Write `contents` to `path` with mode 0600. The contents never appear
    /// in a command line or the environment.
    fn write_private(&self, resource: &str, path: &str, contents: &str) -> Result<bool>;
    /// Clone `source` into the workspace and return `HEAD` and whether the
    /// tree is clean.
    fn clone_source(&self, resource: &str, source: &Source) -> Result<(String, bool)>;
    /// Remove a file. Removing a missing file succeeds.
    fn remove(&self, resource: &str, path: &str) -> Result<()>;
    /// Whether `path` exists.
    fn exists(&self, resource: &str, path: &str) -> Result<bool>;
}

/// What was delivered, as the journal records it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delivered {
    pub execution: String,
    pub resource: String,
    pub source: Source,
    pub payer: Payer,
    pub key_digest: String,
    pub key_path: String,
}

/// Where the key lives in the sandbox.
#[must_use]
pub fn key_path(execution: &str) -> String {
    format!("/tmp/oa-retail/{execution}/provider.key")
}

/// Deliver the admitted source and the customer's key to `resource`.
/// Delivering again after a crash rewrites the same file and reverifies the
/// clone.
///
/// # Errors
///
/// [`Error::Denied`] without the execute and disclose authorities,
/// [`Error::Material`] with a [`MaterialRefusal`], or a sandbox failure.
pub fn deliver(
    journal: &mut Journal,
    sandbox: &impl Sandbox,
    funded: &FundedRequest,
    current: &Current,
    resource: &str,
    source: &Source,
    credential: &Credential,
    now: i64,
) -> Result<Delivered> {
    authority::check(Step::UploadMaterial, &funded.admission, current).map_err(Error::Denied)?;
    // Only this execution's own ready sandbox receives its material.
    match journal.provisioning(&funded.execution)?.map(|p| p.state) {
        Some(crate::provision::ProvisionState::Ready {
            resource: ready, ..
        }) if ready == resource => {}
        _ => {
            return Err(Error::Invalid(
                "the sandbox is not this execution's ready computer",
            ));
        }
    }
    let (provider, secret) = match credential {
        Credential::OwnLogin { .. } => {
            return Err(Error::Material(MaterialRefusal::OwnLoginExport));
        }
        Credential::PaidProvider { .. } => {
            return Err(Error::Material(MaterialRefusal::PaidProviderUnsupported));
        }
        Credential::ApiKey { provider, secret } => (provider, secret),
    };
    let Payer::CallerKey { provider: admitted } = &funded.admission.model_payer else {
        return Err(Error::Material(MaterialRefusal::ProviderChanged));
    };
    if provider != admitted {
        return Err(Error::Material(MaterialRefusal::ProviderChanged));
    }
    if source != &funded.admission.source {
        return Err(Error::Material(MaterialRefusal::SourceChanged));
    }
    let (head, clean) = sandbox.clone_source(resource, source)?;
    if head != source.commit || !clean {
        return Err(Error::Material(MaterialRefusal::SourceUnverified));
    }
    let path = key_path(&funded.execution);
    if !sandbox.write_private(resource, &path, secret.expose())? {
        return Err(Error::Material(MaterialRefusal::IsolationUnsupported));
    }
    let delivered = Delivered {
        execution: funded.execution.clone(),
        resource: resource.into(),
        source: source.clone(),
        payer: funded.admission.model_payer.clone(),
        key_digest: secret.digest(),
        key_path: path,
    };
    journal.connection.execute(
        "INSERT INTO material(execution,resource,source,payer,key_digest,key_path,delivered_at) VALUES(?,?,?,?,?,?,?)
         ON CONFLICT(execution) DO UPDATE SET resource=excluded.resource, key_digest=excluded.key_digest, delivered_at=excluded.delivered_at, removed_at=NULL",
        params![
            delivered.execution,
            delivered.resource,
            serde_json::to_string(&delivered.source)?,
            serde_json::to_string(&delivered.payer)?,
            delivered.key_digest,
            delivered.key_path,
            now
        ],
    )?;
    Ok(delivered)
}

/// Remove the customer's key from the sandbox and record that it is gone.
///
/// # Errors
///
/// A sandbox failure, or a key still present after removal.
pub fn remove_credentials(
    journal: &mut Journal,
    sandbox: &impl Sandbox,
    execution: &str,
    now: i64,
) -> Result<bool> {
    let Some((resource, path)) = journal
        .connection
        .query_row(
            "SELECT resource,key_path FROM material WHERE execution=?",
            [execution],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()?
    else {
        return Ok(false);
    };
    sandbox.remove(&resource, &path)?;
    if sandbox.exists(&resource, &path)? {
        return Err(Error::Invalid("the key is still in the sandbox"));
    }
    journal.connection.execute(
        "UPDATE material SET removed_at=COALESCE(removed_at, ?) WHERE execution=?",
        params![now, execution],
    )?;
    Ok(true)
}

impl Journal {
    /// What was delivered for `execution`, and whether the key was removed.
    ///
    /// # Errors
    ///
    /// A SQLite or decoding failure.
    pub fn delivered(&self, execution: &str) -> Result<Option<(Delivered, bool)>> {
        let row = self
            .connection
            .query_row(
                "SELECT execution,resource,source,payer,key_digest,key_path,removed_at FROM material WHERE execution=?",
                [execution],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, String>(5)?,
                        r.get::<_, Option<i64>>(6)?,
                    ))
                },
            )
            .optional()?;
        let Some((execution, resource, source, payer, key_digest, key_path, removed)) = row else {
            return Ok(None);
        };
        Ok(Some((
            Delivered {
                execution,
                resource,
                source: serde_json::from_str(&source)?,
                payer: serde_json::from_str(&payer)?,
                key_digest,
                key_path,
            },
            removed.is_some(),
        )))
    }
}

/// Replace every occurrence of a secret in `text` before it is logged or
/// retained.
#[must_use]
pub fn scrub(text: &str, secret: &CustomerSecret) -> String {
    if secret.expose().is_empty() {
        return text.to_owned();
    }
    text.replace(secret.expose(), "[redacted]")
}
