//! Moving a registry directory's files into the database, and checking
//! that they arrived.
//!
//! [`import`] reads the account service's files (an NFS export, or the
//! share itself while no gateway writes it) and writes every store,
//! idempotently: a store already holding the same revision is left
//! alone, and one holding a different revision is overwritten only with
//! `force`. [`verify`] reads every store back through the database and
//! compares it with the files.

use std::path::Path;

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::docs::{self, Layout};
use super::{Database, Error};

/// The file a gateway writes after importing a directory at its first
/// start on the database, so later starts do not import stale files.
pub const MARKER: &str = "postgres-import.json";

const GITHUB_ACCESS: &str = "github-access";
const PROVIDER_KEYS: &str = "inference-provider-keys.json";

/// What an import or a verify found, store by store.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct Report {
    pub accounts: usize,
    pub workspaces: usize,
    pub memberships: usize,
    pub invitations: usize,
    pub linked_identities: usize,
    pub sessions: usize,
    pub device_sign_ins: usize,
    pub bearer_keys: usize,
    pub github_access: usize,
    pub provider_keys: usize,
    pub revisions: usize,
    /// One line per store: `imported`, `unchanged`, `absent`, or (for a
    /// verify) `matches`.
    pub stores: Vec<(String, String)>,
    /// Differences a verify found. Empty means every record round-trips.
    pub mismatches: Vec<String>,
}

fn read_json(path: &Path) -> Result<Option<Value>, Error> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| Error(format!("{}: {e}", path.display()))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error(format!("{}: {error}", path.display()))),
    }
}

fn count(doc: &Value, path: &[&str]) -> usize {
    let mut node = doc;
    for step in path {
        match node.get(step) {
            Some(next) => node = next,
            None => return 0,
        }
    }
    node.as_object().map_or(0, serde_json::Map::len)
}

/// The account store's document as the typed store writes it, checked.
fn accounts_doc(path: &Path) -> Result<Option<Value>, Error> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    let store = crate::accounts::Store::parse(&text, &path.display().to_string()).map_err(Error)?;
    serde_json::to_value(&store)
        .map(Some)
        .map_err(|e| Error(e.to_string()))
}

fn sessions_doc(path: &Path) -> Result<Option<Value>, Error> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    let store = crate::sessions::Store::parse(&text, &path.display().to_string()).map_err(Error)?;
    serde_json::to_value(&store)
        .map(Some)
        .map_err(|e| Error(e.to_string()))
}

fn keys_doc(dir: &Path) -> Result<Option<Value>, Error> {
    if !dir.join("keys.json").exists() {
        return Ok(None);
    }
    let store = crate::keys::load_files(dir).map_err(|e| Error(e.to_string()))?;
    serde_json::to_value(&store)
        .map(Some)
        .map_err(|e| Error(e.to_string()))
}

/// A document read from the database as its typed store writes it, so
/// it compares equal to the same store read from a file (an empty book
/// the file leaves out reads back as an empty map).
fn normalize(layout: &Layout, held: &Value) -> Result<Value, Error> {
    let typed = match layout.store {
        "accounts" => crate::accounts::Store::parse(&held.to_string(), "database")
            .map_err(Error)
            .and_then(|s| serde_json::to_value(s).map_err(|e| Error(e.to_string()))),
        "sessions" => crate::sessions::Store::parse(&held.to_string(), "database")
            .map_err(Error)
            .and_then(|s| serde_json::to_value(s).map_err(|e| Error(e.to_string()))),
        _ => serde_json::from_value::<crate::keys::KeyStore>(held.clone())
            .map_err(|e| Error(e.to_string()))
            .and_then(|s| serde_json::to_value(s).map_err(|e| Error(e.to_string()))),
    };
    typed
}

fn put_store(
    database: &Database,
    layout: &Layout,
    doc: &Value,
    force: bool,
    report: &mut Report,
) -> Result<(), Error> {
    let mut tx = database.transaction()?;
    tx.lock(&format!("openagents.store.{}", layout.store))?;
    let held = docs::load_tx(&mut tx, layout)?;
    let state = match held {
        None => "imported",
        Some((_, held)) if normalize(layout, &held).ok().as_ref() == Some(doc) => "unchanged",
        Some(_) if force => "imported",
        Some(_) => {
            return Err(Error(format!(
                "the database already holds a different `{}` store; pass --force to replace it with the files",
                layout.store
            )));
        }
    };
    if state == "imported" {
        docs::write(&mut tx, layout, doc)?;
    }
    tx.commit()?;
    database.forget(layout.store);
    report
        .stores
        .push((layout.store.to_string(), state.to_string()));
    Ok(())
}

fn archive_history(database: &Database, dir: &Path, store: &str) -> Result<usize, Error> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(0);
    };
    let mut paths: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    let mut tx = database.transaction()?;
    let mut count = 0;
    for path in paths {
        let doc = match store {
            "accounts" => accounts_doc(&path)?,
            _ => sessions_doc(&path)?,
        };
        if let Some(doc) = doc {
            docs::archive(&mut tx, store, &doc)?;
            count += 1;
        }
    }
    tx.commit()?;
    Ok(count)
}

fn account_digest(account: &str) -> String {
    Sha256::digest(account.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The GitHub-access records in `dir`, by account.
fn github_records(dir: &Path) -> Result<Vec<(String, Value)>, Error> {
    let Ok(entries) = std::fs::read_dir(dir.join(GITHUB_ACCESS)) else {
        return Ok(Vec::new());
    };
    let mut records = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let Some(record) = read_json(&path)? else {
            continue;
        };
        let account = record
            .get("account")
            .and_then(Value::as_str)
            .ok_or_else(|| Error(format!("{}: no account", path.display())))?
            .to_string();
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if stem != account_digest(&account) {
            return Err(Error(format!(
                "{}: the file is not named for its account",
                path.display()
            )));
        }
        records.push((account, record));
    }
    records.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(records)
}

/// The provider keys in `dir`: (where, owner, provider, kept record).
/// The file keeps them by workspace (`workspaces`), and, written before
/// #11186, by registry tenant (`keys`).
fn provider_records(dir: &Path) -> Result<Vec<(KeptBy, String, String, Value)>, Error> {
    let Some(saved) = read_json(&dir.join(PROVIDER_KEYS))? else {
        return Ok(Vec::new());
    };
    let mut records = Vec::new();
    for (field, by) in [("workspaces", KeptBy::Workspace), ("keys", KeptBy::Tenant)] {
        if let Some(owners) = saved.get(field).and_then(Value::as_object) {
            for (owner, providers) in owners {
                if let Some(providers) = providers.as_object() {
                    for (provider, kept) in providers {
                        records.push((by, owner.clone(), provider.clone(), kept.clone()));
                    }
                }
            }
        }
    }
    Ok(records)
}

/// Whose a provider key in the file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeptBy {
    Workspace,
    /// A registry tenant, from before #11186: the gateway moves it to its
    /// workspace (`identity.provider_keys_by_tenant`).
    Tenant,
}

/// Write one account's GitHub-access record.
///
/// # Errors
///
/// The write fails.
pub fn put_github(database: &Database, account: &str, record: &Value) -> Result<(), Error> {
    database.execute(
        "INSERT INTO identity.github_access (account_id, account_digest, record) VALUES ($1, $2, $3)
         ON CONFLICT (account_id) DO UPDATE SET record = EXCLUDED.record, updated_at = now()
         WHERE identity.github_access.record IS DISTINCT FROM EXCLUDED.record",
        &[&account, &account_digest(account), record],
    )?;
    Ok(())
}

/// Write one provider key, kept by workspace or (from before #11186) by
/// tenant.
///
/// # Errors
///
/// The write fails.
fn put_provider_key(
    database: &Database,
    by: KeptBy,
    owner: &str,
    provider: &str,
    record: &Value,
) -> Result<(), Error> {
    let sql = match by {
        KeptBy::Workspace => {
            "INSERT INTO identity.provider_keys (workspace_id, provider, record) VALUES ($1, $2, $3)
             ON CONFLICT (workspace_id, provider) DO UPDATE SET record = EXCLUDED.record, updated_at = now()
             WHERE identity.provider_keys.record IS DISTINCT FROM EXCLUDED.record"
        }
        KeptBy::Tenant => {
            "INSERT INTO identity.provider_keys_by_tenant (tenant, provider, record) VALUES ($1, $2, $3)
             ON CONFLICT (tenant, provider) DO UPDATE SET record = EXCLUDED.record, updated_at = now()
             WHERE identity.provider_keys_by_tenant.record IS DISTINCT FROM EXCLUDED.record"
        }
    };
    database.execute(sql, &[&owner, &provider, record])?;
    Ok(())
}

/// Import every store in the registry directory `dir`.
///
/// # Errors
///
/// A file does not parse or validate, the database refuses a write, or
/// (without `force`) the database already holds a different revision.
pub fn import(database: &Database, dir: &Path, force: bool) -> Result<Report, Error> {
    let mut report = Report::default();
    match accounts_doc(&dir.join("accounts.json"))? {
        Some(doc) => {
            report.accounts = count(&doc, &["accounts"]);
            report.workspaces = count(&doc, &["workspaces"]);
            report.memberships = doc
                .get("workspaces")
                .and_then(Value::as_object)
                .map_or(0, |all| {
                    all.values().map(|ws| count(ws, &["members"])).sum()
                });
            report.invitations = count(&doc, &["invitations"]);
            report.linked_identities = count(&doc, &["identities", "github"]);
            report.revisions +=
                archive_history(database, &dir.join("accounts-history"), "accounts")?;
            put_store(database, &docs::ACCOUNTS, &doc, force, &mut report)?;
        }
        None => report.stores.push(("accounts".into(), "absent".into())),
    }
    match sessions_doc(&dir.join("sessions.json"))? {
        Some(doc) => {
            report.sessions = count(&doc, &["book", "sessions"]);
            report.device_sign_ins = count(&doc, &["book", "devices"]);
            report.revisions +=
                archive_history(database, &dir.join("sessions-history"), "sessions")?;
            put_store(database, &docs::SESSIONS, &doc, force, &mut report)?;
        }
        None => report.stores.push(("sessions".into(), "absent".into())),
    }
    match keys_doc(dir)? {
        Some(doc) => {
            report.bearer_keys = count(&doc, &["keys"]);
            put_store(database, &docs::KEYS, &doc, force, &mut report)?;
        }
        None => report.stores.push(("keys".into(), "absent".into())),
    }
    for (account, record) in github_records(dir)? {
        put_github(database, &account, &record)?;
        report.github_access += 1;
    }
    for (by, owner, provider, record) in provider_records(dir)? {
        put_provider_key(database, by, &owner, &provider, &record)?;
        report.provider_keys += 1;
    }
    Ok(report)
}

/// Read every store back from the database and compare it with the
/// files in `dir`. The report's `mismatches` names each difference.
///
/// # Errors
///
/// A file or the database cannot be read.
pub fn verify(database: &Database, dir: &Path) -> Result<Report, Error> {
    let mut report = Report::default();
    let pairs: [(&Layout, Option<Value>); 3] = [
        (&docs::ACCOUNTS, accounts_doc(&dir.join("accounts.json"))?),
        (&docs::SESSIONS, sessions_doc(&dir.join("sessions.json"))?),
        (&docs::KEYS, keys_doc(dir)?),
    ];
    for (layout, file) in pairs {
        database.forget(layout.store);
        let held = docs::load(database, layout)?;
        match (file, held) {
            (None, _) => report.stores.push((layout.store.into(), "absent".into())),
            (Some(_), None) => report.mismatches.push(format!(
                "{}: in the files, not in the database",
                layout.store
            )),
            (Some(file), Some((_, held))) => {
                // Read back through the typed store, as the service does.
                match normalize(layout, &held) {
                    Ok(typed) if typed == file => {
                        report.stores.push((layout.store.into(), "matches".into()));
                    }
                    Ok(typed) => {
                        report.mismatches.push(format!(
                            "{}: the database's document differs from the file",
                            layout.store
                        ));
                        diff_rows(layout, &file, &typed, &mut report);
                    }
                    Err(error) => report.mismatches.push(format!("{}: {error}", layout.store)),
                }
                match layout.store {
                    "accounts" => {
                        report.accounts = count(&file, &["accounts"]);
                        report.workspaces = count(&file, &["workspaces"]);
                        report.linked_identities = count(&file, &["identities", "github"]);
                    }
                    "sessions" => report.sessions = count(&file, &["book", "sessions"]),
                    _ => report.bearer_keys = count(&file, &["keys"]),
                }
            }
        }
    }
    for (account, record) in github_records(dir)? {
        let rows = database.query(
            "SELECT record FROM identity.github_access WHERE account_id = $1",
            &[&account],
        )?;
        match rows.first().map(|row| row.get::<_, Value>(0)) {
            Some(held) if held == record => report.github_access += 1,
            _ => report
                .mismatches
                .push(format!("github-access: account {account} differs")),
        }
    }
    for (by, owner, provider, record) in provider_records(dir)? {
        let sql = match by {
            KeptBy::Workspace => {
                "SELECT record FROM identity.provider_keys WHERE workspace_id = $1 AND provider = $2"
            }
            KeptBy::Tenant => {
                "SELECT record FROM identity.provider_keys_by_tenant WHERE tenant = $1 AND provider = $2"
            }
        };
        let rows = database.query(sql, &[&owner, &provider])?;
        match rows.first().map(|row| row.get::<_, Value>(0)) {
            Some(held) if held == record => report.provider_keys += 1,
            _ => report
                .mismatches
                .push(format!("provider key {owner}/{provider} differs")),
        }
    }
    Ok(report)
}

/// Each collection's row count in the file against the document read
/// back, so a verify names what went missing rather than only that
/// something did.
fn diff_rows(layout: &Layout, file: &Value, held: &Value, report: &mut Report) {
    let (Ok((_, file_rows)), Ok((_, held_rows))) = (
        docs::explode(layout, file.clone()),
        docs::explode(layout, held.clone()),
    ) else {
        return;
    };
    for ((collection, a), b) in layout.collections.iter().zip(&file_rows).zip(&held_rows) {
        if a != b {
            report.mismatches.push(format!(
                "{}: {} rows in the files, {} in the database",
                collection.table,
                a.len(),
                b.len()
            ));
        }
    }
}
