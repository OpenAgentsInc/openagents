//! The per-record stores beside the documents: an account's GitHub
//! access (`oa_auth::repos`) and a workspace's own provider keys (the
//! gateway's BYOK store; kept by registry tenant before #11186). Their
//! owners keep their record types and sealing; this module only reads
//! and writes the records, a writer holding the row's lock for its
//! read-modify-write.

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{Database, Error};

fn hex_digest(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// An account's GitHub-access record, when it has one.
///
/// # Errors
///
/// The read fails.
pub fn github_access(database: &Database, account: &str) -> Result<Option<Value>, Error> {
    let rows = database.query(
        "SELECT record FROM identity.github_access WHERE account_id = $1",
        &[&account],
    )?;
    Ok(rows.first().map(|row| row.get(0)))
}

/// The GitHub-access record whose account hashes to `digest` (a broker
/// ticket carries the digest, not the account).
///
/// # Errors
///
/// The read fails.
pub fn github_access_by_digest(database: &Database, digest: &str) -> Result<Option<Value>, Error> {
    let rows = database.query(
        "SELECT record FROM identity.github_access WHERE account_digest = $1",
        &[&digest.to_ascii_lowercase()],
    )?;
    Ok(rows.first().map(|row| row.get(0)))
}

/// Change an account's GitHub-access record under its lock: `change`
/// gets the current record (or `None`) and returns the record to keep and
/// its answer. An `Err` from `change` writes nothing.
///
/// # Errors
///
/// The database fails; `change`'s own error is the inner result.
pub fn update_github_access<T, E>(
    database: &Database,
    account: &str,
    change: impl FnOnce(Option<Value>) -> Result<(Value, T), E>,
) -> Result<Result<T, E>, Error> {
    let mut tx = database.transaction()?;
    tx.lock(&format!("openagents.github-access.{account}"))?;
    let rows = tx.query(
        "SELECT record FROM identity.github_access WHERE account_id = $1",
        &[&account],
    )?;
    let current = rows.first().map(|row| row.get::<_, Value>(0));
    let (record, out) = match change(current.clone()) {
        Ok(changed) => changed,
        Err(error) => return Ok(Err(error)),
    };
    if current.as_ref() != Some(&record) {
        tx.execute(
            "INSERT INTO identity.github_access (account_id, account_digest, record) VALUES ($1, $2, $3)
             ON CONFLICT (account_id) DO UPDATE SET record = EXCLUDED.record, updated_at = now()",
            &[&account, &hex_digest(account), &record],
        )?;
    }
    tx.commit()?;
    Ok(Ok(out))
}

/// A workspace's provider keys: provider word to kept record (#11186).
///
/// # Errors
///
/// The read fails.
pub fn provider_keys(database: &Database, workspace: &str) -> Result<Vec<(String, Value)>, Error> {
    let rows = database.query(
        "SELECT provider, record FROM identity.provider_keys WHERE workspace_id = $1 ORDER BY provider",
        &[&workspace],
    )?;
    Ok(rows.iter().map(|row| (row.get(0), row.get(1))).collect())
}

/// Keep `record` as `workspace`'s key for `provider`, replacing any.
///
/// # Errors
///
/// The write fails.
pub fn put_provider_key(
    database: &Database,
    workspace: &str,
    provider: &str,
    record: &Value,
) -> Result<(), Error> {
    database.execute(
        "INSERT INTO identity.provider_keys (workspace_id, provider, record) VALUES ($1, $2, $3)
         ON CONFLICT (workspace_id, provider) DO UPDATE SET record = EXCLUDED.record, updated_at = now()",
        &[&workspace, &provider, record],
    )?;
    Ok(())
}

/// Forget `workspace`'s key for `provider`; whether there was one.
///
/// # Errors
///
/// The write fails.
pub fn delete_provider_key(
    database: &Database,
    workspace: &str,
    provider: &str,
) -> Result<bool, Error> {
    Ok(database.execute(
        "DELETE FROM identity.provider_keys WHERE workspace_id = $1 AND provider = $2",
        &[&workspace, &provider],
    )? > 0)
}

/// A provider key still kept by registry tenant, from before #11186.
#[derive(Clone, Debug, PartialEq)]
pub struct TenantProviderKey {
    pub tenant: String,
    pub provider: String,
    /// The workspace an operator named as the one that saved it.
    pub workspace: Option<String>,
    pub record: Value,
}

/// Every provider key still kept by tenant
/// (`identity.provider_keys_by_tenant`).
///
/// # Errors
///
/// The read fails.
pub fn tenant_provider_keys(database: &Database) -> Result<Vec<TenantProviderKey>, Error> {
    let rows = database.query(
        "SELECT tenant, provider, workspace_id, record FROM identity.provider_keys_by_tenant
         ORDER BY tenant, provider",
        &[],
    )?;
    Ok(rows
        .iter()
        .map(|row| TenantProviderKey {
            tenant: row.get(0),
            provider: row.get(1),
            workspace: row.get(2),
            record: row.get(3),
        })
        .collect())
}

/// Name the workspace that saved a tenant-kept key, so the gateway moves
/// it there at its next start. The workspace must exist and be bound to
/// the key's tenant. Whether a row was named.
///
/// # Errors
///
/// The workspace is unknown or on another tenant, or the write fails.
pub fn assign_tenant_provider_key(
    database: &Database,
    tenant: &str,
    provider: &str,
    workspace: &str,
) -> Result<bool, Error> {
    let bound = database.query(
        "SELECT tenant FROM workspace.workspaces WHERE id = $1",
        &[&workspace],
    )?;
    match bound.first().map(|row| row.get::<_, Option<String>>(0)) {
        None => return Err(Error(format!("no workspace {workspace}"))),
        Some(held) if held.as_deref() != Some(tenant) => {
            return Err(Error(format!(
                "workspace {workspace} is not on the tenant {tenant}"
            )));
        }
        Some(_) => {}
    }
    Ok(database.execute(
        "UPDATE identity.provider_keys_by_tenant SET workspace_id = $3, updated_at = now()
         WHERE tenant = $1 AND provider = $2",
        &[&tenant, &provider, &workspace],
    )? > 0)
}

/// Move a tenant-kept key into `workspace` as `record` (re-sealed by its
/// owner for the workspace) and forget the tenant row, in one
/// transaction. A key the workspace already saved itself is kept, and
/// the tenant row is still forgotten. Whether `record` was kept.
///
/// # Errors
///
/// The write fails; nothing changes.
pub fn adopt_tenant_provider_key(
    database: &Database,
    tenant: &str,
    provider: &str,
    workspace: &str,
    record: &Value,
) -> Result<bool, Error> {
    let mut tx = database.transaction()?;
    let kept = tx.execute(
        "INSERT INTO identity.provider_keys (workspace_id, provider, record) VALUES ($1, $2, $3)
         ON CONFLICT (workspace_id, provider) DO NOTHING",
        &[&workspace, &provider, record],
    )? > 0;
    tx.execute(
        "DELETE FROM identity.provider_keys_by_tenant WHERE tenant = $1 AND provider = $2",
        &[&tenant, &provider],
    )?;
    tx.commit()?;
    Ok(kept)
}
