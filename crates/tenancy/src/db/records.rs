//! The per-record stores beside the documents: an account's GitHub
//! access (`oa_auth::repos`) and a tenant's own provider keys (the
//! gateway's BYOK store). Their owners keep their record types and
//! sealing; this module only reads and writes the records, a writer
//! holding the row's lock for its read-modify-write.

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

/// A tenant's provider keys: provider word to kept record.
///
/// # Errors
///
/// The read fails.
pub fn provider_keys(database: &Database, tenant: &str) -> Result<Vec<(String, Value)>, Error> {
    let rows = database.query(
        "SELECT provider, record FROM identity.provider_keys WHERE tenant = $1 ORDER BY provider",
        &[&tenant],
    )?;
    Ok(rows.iter().map(|row| (row.get(0), row.get(1))).collect())
}

/// Keep `record` as `tenant`'s key for `provider`, replacing any.
///
/// # Errors
///
/// The write fails.
pub fn put_provider_key(
    database: &Database,
    tenant: &str,
    provider: &str,
    record: &Value,
) -> Result<(), Error> {
    database.execute(
        "INSERT INTO identity.provider_keys (tenant, provider, record) VALUES ($1, $2, $3)
         ON CONFLICT (tenant, provider) DO UPDATE SET record = EXCLUDED.record, updated_at = now()",
        &[&tenant, &provider, record],
    )?;
    Ok(())
}

/// Forget `tenant`'s key for `provider`; whether there was one.
///
/// # Errors
///
/// The write fails.
pub fn delete_provider_key(
    database: &Database,
    tenant: &str,
    provider: &str,
) -> Result<bool, Error> {
    Ok(database.execute(
        "DELETE FROM identity.provider_keys WHERE tenant = $1 AND provider = $2",
        &[&tenant, &provider],
    )? > 0)
}
