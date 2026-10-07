//! Declared task artifacts and cleanup duties survive the launching client.
//! Payloads stay in the private journal for the admitted retention period;
//! resource deletion is observed independently from artifact completeness.

use crate::authority::{self, Current, Source, Step};
use crate::journal::Journal;
use crate::offer::FundedRequest;
use crate::provision::{Provider, ProvisionState, ResourceState};
use crate::{Error, Result, sha256_hex};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub(crate) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS retention (
 execution TEXT PRIMARY KEY, binding TEXT NOT NULL, manifest TEXT,
 attempted_at INTEGER, expires_at INTEGER NOT NULL, complete INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS retained_artifact (
 execution TEXT NOT NULL, name TEXT NOT NULL, digest TEXT NOT NULL,
 bytes BLOB, PRIMARY KEY(execution,name)
);
CREATE TABLE IF NOT EXISTS cleanup (
 execution TEXT NOT NULL, resource TEXT NOT NULL,
 intent_at INTEGER NOT NULL, acknowledged_at INTEGER,
 attempts INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(execution,resource)
);
CREATE TABLE IF NOT EXISTS cleanup_discovery (
 execution TEXT PRIMARY KEY, requested_at INTEGER NOT NULL, reconciled INTEGER NOT NULL DEFAULT 0, checked_at INTEGER NOT NULL DEFAULT 0
);
";
const MAX_ARTIFACTS: usize = 8;
const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Patch,
    Checks,
    Log,
}

/// One declared UTF-8 artifact. Names are logical identifiers, never paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub name: String,
    pub kind: Kind,
    pub digest: String,
    pub size: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub execution: String,
    pub task: String,
    pub resource: String,
    pub source: Source,
    pub engine: String,
    pub artifacts: Vec<Artifact>,
}

/// The sandbox task owner's retained, already scrubbed task artifacts.
/// Transport failure leaves delivery incomplete; it never defers cleanup
/// indefinitely or substitutes a filesystem path supplied by a client.
pub trait Artifacts {
    fn manifest(&self, resource: &str, task: &str) -> Result<Manifest>;
    fn read(&self, resource: &str, task: &str, name: &str, max_bytes: usize) -> Result<Vec<u8>>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cleanup {
    pub resource: String,
    pub intent_at: i64,
    pub acknowledged_at: Option<i64>,
    pub attempts: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub execution: String,
    pub manifest: Option<Manifest>,
    pub complete: bool,
    pub expired: bool,
    pub expires_at: i64,
    pub resources: Vec<Cleanup>,
    pub discovery_complete: bool,
}
impl Receipt {
    #[must_use]
    pub fn deleted(&self) -> bool {
        self.discovery_complete && self.resources.iter().all(|r| r.acknowledged_at.is_some())
    }
}

/// Record a service cleanup duty from the original funded identity. This
/// service operation also runs after revocation; new dispatch and
/// provisioning are refused once this intent exists.
pub fn request(journal: &mut Journal, funded: &FundedRequest, now: i64) -> Result<()> {
    if now < 0 || journal.funded(&funded.execution)?.as_ref() != Some(funded) {
        return Err(Error::Conflict(
            "cleanup requires the original funded identity",
        ));
    }
    let tx = journal.immediate()?;
    tx.execute("INSERT INTO retention(execution,binding,expires_at) VALUES(?,?,?) ON CONFLICT(execution) DO NOTHING",
        params![funded.execution,serde_json::to_string(funded)?,now.saturating_add(crate::contract::RETENTION_DAYS as i64*86400)])?;
    tx.execute("INSERT INTO cleanup_discovery(execution,requested_at) VALUES(?,?) ON CONFLICT(execution) DO NOTHING",params![funded.execution,now])?;
    tx.commit()?;
    Ok(())
}

/// Collect artifacts once, discover all recorded provisioning attempts,
/// and reconcile deletion of those exact provider resources. Retrying an
/// idempotent delete never creates a computer or changes execution identity.
/// A failed provider observation cannot acknowledge deletion.
pub fn advance(
    journal: &mut Journal,
    provider: &impl Provider,
    artifacts: &impl Artifacts,
    execution: &str,
    now: i64,
) -> Result<Receipt> {
    let (binding, attempted): (String, Option<i64>) = journal.connection.query_row(
        "SELECT binding,attempted_at FROM retention WHERE execution=?",
        [execution],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let funded: FundedRequest = serde_json::from_str(&binding)?;
    if attempted.is_none() {
        // Persist the attempted collection even when its acknowledgment is
        // lost. Missing bytes remain explicitly incomplete in the receipt.
        journal.check_custody()?;
        journal.connection.execute(
            "UPDATE retention SET attempted_at=? WHERE execution=? AND attempted_at IS NULL",
            params![now, execution],
        )?;
        let _collection: Result<()> = (|| {
            if let Some(dispatch) = journal.dispatch(execution)?
                && let Ok(manifest) = artifacts.manifest(&dispatch.resource, &dispatch.task)
            {
                if manifest.execution != execution
                    || manifest.resource != dispatch.resource
                    || manifest.task != dispatch.task
                    || manifest.source != funded.admission.source
                    || manifest.engine != "codex"
                    || manifest.artifacts.is_empty()
                    || manifest.artifacts.len() > MAX_ARTIFACTS
                {
                    return Err(Error::Invalid(
                        "artifact manifest differs from its admitted task",
                    ));
                }
                let mut names = BTreeSet::new();
                for item in &manifest.artifacts {
                    if item.name.is_empty()
                        || item.name.len() > 128
                        || !item
                            .name
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
                        || !names.insert(&item.name)
                        || item.size > MAX_BYTES
                        || item.digest.len() != 64
                        || !item.digest.bytes().all(|c| c.is_ascii_hexdigit())
                    {
                        return Err(Error::Invalid("invalid declared artifact"));
                    }
                }
                let key_digest = journal.delivered(execution)?.map(|(d, _)| d.key_digest);
                let mut collected = Vec::new();
                for item in &manifest.artifacts {
                    if let Ok(bytes) =
                        artifacts.read(&dispatch.resource, &dispatch.task, &item.name, item.size)
                        && bytes.len() == item.size
                        && sha256_hex(&bytes) == item.digest
                        && !contains_key(&bytes, key_digest.as_deref())
                    {
                        collected.push((item, bytes));
                    }
                }
                let tx = journal.immediate()?;
                for (item, bytes) in &collected {
                    tx.execute("INSERT INTO retained_artifact(execution,name,digest,bytes) VALUES(?,?,?,?) ON CONFLICT(execution,name) DO NOTHING",params![execution,item.name,item.digest,bytes])?;
                }
                tx.execute(
                    "UPDATE retention SET manifest=?,complete=? WHERE execution=?",
                    params![
                        serde_json::to_string(&manifest)?,
                        collected.len() == manifest.artifacts.len(),
                        execution
                    ],
                )?;
                tx.commit()?;
            }
            Ok(())
        })();
        // Collection refusal is an incomplete delivery, not permission to
        // leave a billing resource running. The durable duty still proceeds.
    }
    let provision = journal.provisioning(execution)?;
    let mut resources = BTreeSet::new();
    let mut discovered = true;
    if let Some(provision) = provision {
        let creation_unknown = matches!(&provision.state, ProvisionState::Creating);
        if let Some(abandoned) = provision.abandoned {
            resources.extend(
                abandoned
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
            );
        }
        match provision.state {
            ProvisionState::Starting { resource } | ProvisionState::Ready { resource, .. } => {
                resources.insert(resource);
            }
            _ => {}
        }
        for attempt in 1..=provision.attempt {
            let label = format!("{execution}#{attempt}");
            match provider.find(&label) {
                Ok(Some(resource))
                    if resource.account == funded.account && resource.provisioning == label =>
                {
                    resources.insert(resource.id);
                }
                Ok(None) => {
                    if creation_unknown {
                        discovered = false;
                    }
                }
                _ => discovered = false,
            }
        }
    }
    let tx = journal.immediate()?;
    for resource in resources {
        tx.execute("INSERT INTO cleanup(execution,resource,intent_at) VALUES(?,?,?) ON CONFLICT(execution,resource) DO NOTHING",params![execution,resource,now])?;
    }
    tx.execute(
        "UPDATE cleanup_discovery SET reconciled=? WHERE execution=?",
        params![discovered, execution],
    )?;
    tx.commit()?;
    let receipt = journal
        .retention_receipt(execution, now)?
        .ok_or(Error::Invalid("missing retention duty"))?;
    for resource in receipt
        .resources
        .iter()
        .filter(|r| r.acknowledged_at.is_none())
    {
        match provider.state(&resource.resource) {
            Ok(ResourceState::Deleted) => acknowledge(journal, execution, &resource.resource, now)?,
            Ok(_) => {
                journal.check_custody()?;
                journal.connection.execute(
                    "UPDATE cleanup SET attempts=attempts+1 WHERE execution=? AND resource=?",
                    params![execution, resource.resource],
                )?;
                let _ = provider.delete(&resource.resource);
                if matches!(
                    provider.state(&resource.resource),
                    Ok(ResourceState::Deleted)
                ) {
                    acknowledge(journal, execution, &resource.resource, now)?;
                }
            }
            Err(_) => {}
        }
    }
    journal.check_custody()?;
    journal.connection.execute("UPDATE retained_artifact SET bytes=NULL WHERE execution=? AND EXISTS(SELECT 1 FROM retention WHERE execution=? AND expires_at<=?)",params![execution,execution,now])?;
    journal
        .retention_receipt(execution, now)?
        .ok_or(Error::Invalid("missing retention duty"))
}

fn acknowledge(journal: &Journal, execution: &str, resource: &str, now: i64) -> Result<()> {
    journal.check_custody()?;
    journal.connection.execute("UPDATE cleanup SET acknowledged_at=COALESCE(acknowledged_at,?) WHERE execution=? AND resource=?",params![now,execution,resource])?;
    Ok(())
}
fn contains_key(bytes: &[u8], digest: Option<&str>) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return true;
    };
    digest.is_some_and(|digest| {
        text.split(|c: char| !c.is_ascii_alphanumeric() && !matches!(c, '-' | '_'))
            .any(|token| sha256_hex(token.as_bytes()) == digest)
    })
}

impl Journal {
    pub(crate) fn cleanup_requested(&self, execution: &str) -> Result<bool> {
        Ok(self
            .connection
            .query_row(
                "SELECT 1 FROM cleanup_discovery WHERE execution=? UNION SELECT 1 FROM cancellation WHERE execution=?",
                [execution,execution],
                |r| r.get::<_, i32>(0),
            )
            .optional()?
            .is_some())
    }
    /// Receipts contain identities and completeness, never artifact contents.
    pub fn retention_receipt(&self, execution: &str, now: i64) -> Result<Option<Receipt>> {
        let row = self
            .connection
            .query_row(
                "SELECT manifest,complete,expires_at FROM retention WHERE execution=?",
                [execution],
                |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?,
                        r.get::<_, bool>(1)?,
                        r.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((manifest, complete, expires_at)) = row else {
            return Ok(None);
        };
        let mut stmt=self.connection.prepare("SELECT resource,intent_at,acknowledged_at,attempts FROM cleanup WHERE execution=? ORDER BY resource")?;
        let resources = stmt
            .query_map([execution], |r| {
                Ok(Cleanup {
                    resource: r.get(0)?,
                    intent_at: r.get(1)?,
                    acknowledged_at: r.get(2)?,
                    attempts: r.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let discovery_complete = self.connection.query_row(
            "SELECT reconciled FROM cleanup_discovery WHERE execution=?",
            [execution],
            |r| r.get(0),
        )?;
        Ok(Some(Receipt {
            execution: execution.into(),
            manifest: manifest.map(|m| serde_json::from_str(&m)).transpose()?,
            complete,
            expired: now >= expires_at,
            expires_at,
            resources,
            discovery_complete,
        }))
    }
    /// Read only an admitted retained artifact. Expiry or missing delivery
    /// returns no bytes; observing it cannot provision or execute anything.
    pub fn retained_artifact(
        &self,
        funded: &FundedRequest,
        current: &Current,
        name: &str,
        now: i64,
    ) -> Result<Option<Vec<u8>>> {
        authority::check(Step::Read, &funded.admission, current).map_err(Error::Denied)?;
        if self.funded(&funded.execution)?.as_ref() != Some(funded) {
            return Err(Error::Conflict(
                "artifact reader has another funded identity",
            ));
        }
        let bytes=self.connection.query_row("SELECT a.bytes FROM retained_artifact a JOIN retention r USING(execution) WHERE a.execution=? AND a.name=? AND r.expires_at>?",params![funded.execution,name,now],|r|r.get::<_,Option<Vec<u8>>>(0)).optional()?;
        Ok(bytes.flatten())
    }
}

/// Advance at most 16 durable duties without a launching client. Unknown
/// resources remain due; oldest service observations run first so one
/// unavailable provider cannot starve another execution's cleanup.
pub fn worker_step(
    journal: &mut Journal,
    provider: &impl Provider,
    artifacts: &impl Artifacts,
    now: i64,
) -> Result<Vec<(String, Result<Receipt>)>> {
    let ids = {
        let mut stmt=journal.connection.prepare("SELECT d.execution FROM cleanup_discovery d JOIN retention r USING(execution) WHERE d.reconciled=0 OR r.attempted_at IS NULL OR EXISTS(SELECT 1 FROM cleanup c WHERE c.execution=d.execution AND c.acknowledged_at IS NULL) OR (r.expires_at<=? AND EXISTS(SELECT 1 FROM retained_artifact a WHERE a.execution=d.execution AND a.bytes IS NOT NULL)) ORDER BY d.checked_at,d.requested_at,d.execution LIMIT 16")?;
        stmt.query_map([now], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    let mut outcomes = Vec::new();
    for execution in ids {
        journal.check_custody()?;
        journal.connection.execute(
            "UPDATE cleanup_discovery SET checked_at=? WHERE execution=?",
            params![now, execution],
        )?;
        let outcome = advance(journal, provider, artifacts, &execution, now);
        outcomes.push((execution, outcome));
    }
    Ok(outcomes)
}

/// Discover duties from the task owner and the admitted maximum lifetime,
/// then run the bounded cleanup worker. A vanished client is not required
/// to submit a cleanup request. Unknown task status alone never proves that
/// execution ended; its sandbox is stopped at the admitted deadline.
pub fn service_step(
    journal: &mut Journal,
    provider: &impl Provider,
    owner: &impl crate::dispatch::TaskOwner,
    artifacts: &impl Artifacts,
    now: i64,
) -> Result<Vec<(String, Result<Receipt>)>> {
    for funded in journal.all_funded()? {
        if journal.cleanup_requested(&funded.execution)? {
            continue;
        }
        let deadline = i64::try_from(funded.confirmed_at)
            .unwrap_or(i64::MAX)
            .saturating_add(funded.quote.max_seconds as i64)
            .saturating_add(crate::provision::READY_DEADLINE_SECS * 2);
        let ended = if let Some(dispatch) = journal.dispatch(&funded.execution)? {
            matches!(
                owner.status(&dispatch.resource, &dispatch.task),
                Ok(Some(
                    crate::dispatch::TaskStatus::Ended { .. }
                        | crate::dispatch::TaskStatus::Cancelled
                ))
            )
        } else {
            false
        };
        let unavailable = journal.provisioning(&funded.execution)?.is_some_and(|p| {
            matches!(
                p.state,
                ProvisionState::Refused { .. } | ProvisionState::Unavailable
            )
        });
        if ended
            || unavailable
            || now >= deadline
            || journal.cancellation_requested(&funded.execution)?
        {
            request(journal, &funded, now)?;
        }
    }
    worker_step(journal, provider, artifacts, now)
}
