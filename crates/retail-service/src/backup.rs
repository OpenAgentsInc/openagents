//! Bounded offline snapshots and rollback checks. Never rewind active money
//! records: restore goes only to new paths, and binary rollback keeps live state.
use crate::{
    Error, Result,
    package::{self, Host, Identity},
    store::Store,
};
use route_contract::Digest;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path},
    process::Command,
};
const SCHEMA: &str = "openagents.cloud.retail-checkpoint.v1";
const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FILES: usize = 512;
fn payload_expiry(connection: &rusqlite::Connection) -> Result<Option<i64>> {
    Ok(connection.query_row("SELECT MIN(r.expires_at) FROM retained_artifact a JOIN retention r USING(execution) WHERE a.bytes IS NOT NULL", [], |r| r.get(0))?)
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRecord {
    pub digest: Digest,
    pub bytes: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub schema: String,
    pub identity: Identity,
    pub created_at: i64,
    pub expires_at: i64,
    pub files: BTreeMap<String, FileRecord>,
}
fn regular(path: &Path) -> Result<Vec<u8>> {
    crate::store::check_path(path)?;
    let mut f = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let m = f.metadata()?;
    if !m.is_file()
        || m.uid() != unsafe { libc::geteuid() }
        || m.nlink() != 1
        || m.len() > MAX_BYTES
    {
        return Err(Error::Invalid(
            "checkpoint files must be owned, unshared, and bounded",
        ));
    }
    let mut b = Vec::new();
    Read::by_ref(&mut f)
        .take(MAX_BYTES + 1)
        .read_to_end(&mut b)?;
    if b.len() as u64 > MAX_BYTES {
        return Err(Error::Invalid("checkpoint file exceeds its bound"));
    }
    Ok(b)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(parent)?;
        }
        crate::store::private_dir(parent)?;
    }
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    if let Some(parent) = path.parent() {
        for ancestor in parent.ancestors() {
            fs::File::open(ancestor)?.sync_all()?;
        }
    }
    Ok(())
}
fn names(root: &Path, relative: &Path, out: &mut Vec<String>) -> Result<()> {
    let m = fs::symlink_metadata(root.join(relative))?;
    if m.is_dir() {
        crate::store::private_dir(&root.join(relative))?;
        for entry in fs::read_dir(root.join(relative))? {
            let entry = entry?;
            let name = entry.file_name();
            let s = name
                .to_str()
                .ok_or(Error::Invalid("checkpoint filename is not UTF-8"))?;
            if s == "service.lock"
                || s.ends_with("-journal")
                || s.ends_with("-wal")
                || s.ends_with("-shm")
            {
                continue;
            }
            if s.len() > 128 || s.chars().any(|c| c.is_control()) {
                return Err(Error::Invalid("checkpoint filename exceeds its bound"));
            }
            names(root, &relative.join(name), out)?;
        }
    } else if m.is_file() && m.nlink() == 1 {
        out.push(
            relative
                .to_str()
                .ok_or(Error::Invalid("checkpoint path is not UTF-8"))?
                .into(),
        );
        if out.len() > MAX_FILES {
            return Err(Error::Invalid("checkpoint file count exceeds its bound"));
        }
    } else {
        return Err(Error::Invalid(
            "checkpoint cannot retain links or special files",
        ));
    }
    Ok(())
}
/// Stop the runtime first. Its private lock plus SQLite exclusive transactions
/// quiesce all three durable writers while the consistent files are copied.
pub fn snapshot(
    host: &Host,
    identity: Identity,
    destination: &Path,
    now: i64,
) -> Result<Checkpoint> {
    crate::store::check_path(destination)?;
    if destination.starts_with(&host.customer.state) {
        return Err(Error::Conflict("checkpoint must stay outside live state"));
    }
    if destination.exists() {
        return Err(Error::Conflict("checkpoint destination already exists"));
    }
    for p in [
        &host.customer.ledger,
        &host.customer.state.join("transport.sqlite"),
        &host.customer.state.join("lifecycle.sqlite"),
    ] {
        package::private_read(p, MAX_BYTES)?;
    }
    let store = Store::open(&host.customer.state, &host.customer.ledger)?;
    if fs::read_dir(host.customer.state.join("credentials"))?
        .next()
        .is_some()
    {
        return Err(Error::Conflict(
            "checkpoint waits until transient customer credentials are removed",
        ));
    }
    let paths = [
        host.customer.ledger.clone(),
        host.customer.state.join("transport.sqlite"),
        host.customer.state.join("lifecycle.sqlite"),
    ];
    let mut connections = Vec::new();
    for path in &paths {
        let c = rusqlite::Connection::open(path)?;
        c.busy_timeout(std::time::Duration::from_secs(5))?;
        let mode: String = c.query_row("PRAGMA journal_mode", [], |r| r.get(0))?;
        if mode != "delete" {
            return Err(Error::Conflict(
                "offline checkpoint requires SQLite delete-journal mode",
            ));
        }
        c.execute_batch("BEGIN EXCLUSIVE;")?;
        connections.push(c);
    }
    // A backup must not extend the customer's original artifact retention.
    let expires_at = now
        .checked_add(retail_cloud::contract::RETENTION_DAYS as i64 * 86_400)
        .ok_or(Error::Invalid("checkpoint expiry overflow"))?
        .min(payload_expiry(&connections[2])?.unwrap_or(i64::MAX));
    if expires_at <= now {
        return Err(Error::Conflict(
            "purge expired native artifacts before taking a checkpoint",
        ));
    }
    store.check()?;
    fs::DirBuilder::new().mode(0o700).create(destination)?;
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    let mut state_files = Vec::new();
    names(&host.customer.state, Path::new(""), &mut state_files)?;
    let mut source_files = vec![("ledger.sqlite".to_owned(), host.customer.ledger.clone())];
    source_files.extend(
        state_files
            .into_iter()
            .map(|p| (format!("state/{p}"), host.customer.state.join(p))),
    );
    for (relative, path) in source_files {
        let bytes = regular(&path)?;
        total = total
            .checked_add(bytes.len() as u64)
            .ok_or(Error::Invalid("checkpoint size overflow"))?;
        if total > MAX_BYTES || files.len() >= MAX_FILES {
            return Err(Error::Invalid("checkpoint exceeds its bounded package"));
        }
        store.check()?;
        write_new(&destination.join(&relative), &bytes)?;
        files.insert(
            relative,
            FileRecord {
                digest: Digest::of_bytes(&bytes),
                bytes: bytes.len() as u64,
            },
        );
    }
    store.check()?;
    let checkpoint = Checkpoint {
        schema: SCHEMA.into(),
        identity,
        created_at: now,
        expires_at,
        files,
    };
    write_new(
        &destination.join("checkpoint.json"),
        &serde_json::to_vec(&checkpoint)?,
    )?;
    fs::File::open(destination)?.sync_all()?;
    drop(connections);
    Ok(checkpoint)
}
pub fn inspect(snapshot: &Path, now: i64) -> Result<Checkpoint> {
    crate::store::check_path(snapshot)?;
    let m = fs::symlink_metadata(snapshot)?;
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o077 != 0 {
        return Err(Error::Invalid(
            "checkpoint directory must already be private and owned",
        ));
    }
    let c: Checkpoint = serde_json::from_slice(&package::private_read(
        &snapshot.join("checkpoint.json"),
        1024 * 1024,
    )?)?;
    if c.schema != SCHEMA
        || c.files.is_empty()
        || c.files.len() > MAX_FILES
        || c.expires_at <= now
        || c.created_at > now
        || c.expires_at <= c.created_at
        || c.expires_at
            .checked_sub(c.created_at)
            .is_none_or(|age| age > retail_cloud::contract::RETENTION_DAYS as i64 * 86_400)
    {
        return Err(Error::Invalid(
            "checkpoint schema, bounds, or retention is invalid",
        ));
    }
    let mut total = 0u64;
    for (name, record) in &c.files {
        if name != "ledger.sqlite" && !name.starts_with("state/")
            || Path::new(name).is_absolute()
            || Path::new(name)
                .components()
                .any(|p| !matches!(p, Component::Normal(_)))
        {
            return Err(Error::Invalid("checkpoint path is not admitted"));
        }
        let bytes = package::private_read(&snapshot.join(name), MAX_BYTES)?;
        total = total
            .checked_add(bytes.len() as u64)
            .ok_or(Error::Invalid("checkpoint size overflow"))?;
        if total > MAX_BYTES
            || bytes.len() as u64 != record.bytes
            || Digest::of_bytes(&bytes) != record.digest
        {
            return Err(Error::Conflict("checkpoint bytes changed"));
        }
    }
    if !c.files.contains_key("ledger.sqlite")
        || !c.files.contains_key("state/transport.sqlite")
        || !c.files.contains_key("state/lifecycle.sqlite")
    {
        return Err(Error::Invalid("checkpoint omits a durable owner"));
    }
    let lifecycle = rusqlite::Connection::open_with_flags(
        snapshot.join("state/lifecycle.sqlite"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    if payload_expiry(&lifecycle)?.is_some_and(|at| c.expires_at > at) {
        return Err(Error::Invalid(
            "checkpoint extends native artifact retention",
        ));
    }
    Ok(c)
}
/// Restore only to absent paths. This cannot replace active or newer accounting.
pub fn restore(snapshot: &Path, state: &Path, ledger: &Path, now: i64) -> Result<Checkpoint> {
    let c = inspect(snapshot, now)?;
    crate::store::check_path(state)?;
    crate::store::check_path(ledger)?;
    if state.exists()
        || ledger.exists()
        || state.starts_with(snapshot)
        || ledger.starts_with(snapshot)
        || ledger.starts_with(state)
    {
        return Err(Error::Conflict(
            "restore requires separate absent state and ledger paths",
        ));
    }
    fs::DirBuilder::new().mode(0o700).create(state)?;
    for (name, record) in &c.files {
        let bytes = package::private_read(&snapshot.join(name), MAX_BYTES)?;
        if Digest::of_bytes(&bytes) != record.digest {
            return Err(Error::Conflict("checkpoint changed during restore"));
        }
        let target = if name == "ledger.sqlite" {
            ledger.to_owned()
        } else {
            state.join(name.strip_prefix("state/").unwrap())
        };
        write_new(&target, &bytes)?;
    }
    fs::File::open(state)?.sync_all()?;
    Ok(c)
}
/// Check the exact selected executable against the unchanged current checkpoint.
/// Operators then replace only the binary link; these live files stay in place.
pub fn rollback_check(
    host: &Host,
    config: &Path,
    snapshot: &Path,
    candidate: &Path,
    candidate_digest: &Digest,
    now: i64,
) -> Result<serde_json::Value> {
    let c = inspect(snapshot, now)?;
    let store = Store::open(&host.customer.state, &host.customer.ledger)?;
    let check = || -> Result<()> {
        store.check()?;
        for (name, record) in &c.files {
            let path = if name == "ledger.sqlite" {
                host.customer.ledger.clone()
            } else {
                host.customer
                    .state
                    .join(name.strip_prefix("state/").unwrap())
            };
            if Digest::of_bytes(&regular(&path)?) != record.digest {
                return Err(Error::Conflict(
                    "live accounting changed since the checkpoint; take a fresh checkpoint",
                ));
            }
        }
        Ok(())
    };
    check()?;
    crate::store::check_path(candidate)?;
    let candidate_bytes = package::executable_bytes(candidate)?;
    if Digest::of_bytes(&candidate_bytes) != *candidate_digest {
        return Err(Error::Conflict(
            "rollback executable differs from its selected digest",
        ));
    }
    // Execute these exact validated bytes, even if the public candidate path
    // changes before dispatch. The private copy leaves with the supervisor.
    let staged = tempfile::Builder::new()
        .prefix("rollback-")
        .tempdir_in(&host.customer.state)?;
    fs::set_permissions(staged.path(), fs::Permissions::from_mode(0o700))?;
    let executable = staged.path().join("retail-service");
    write_new(&executable, &candidate_bytes)?;
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))?;
    store.check()?;
    let mut command = Command::new(&executable);
    command
        .args(["inspect", "--config"])
        .arg(config)
        .env_clear();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let mut limits = supervise::Limits::within(std::time::Duration::from_secs(15));
    limits.stream_max = 1024 * 1024;
    let output = runtime.block_on(supervise::Job::from_command(command).bounded(limits).run());
    if !output.ending.success() || output.stdout.truncated {
        return Err(Error::Conflict(
            "rollback executable cannot inspect this configuration and state",
        ));
    }
    let report: serde_json::Value = serde_json::from_str(&output.stdout.text)?;
    if report["identity"]["storage_schema"] != package::STORAGE_SCHEMA
        || report["identity"]["customer_schema"] != crate::types::SCHEMA
        || report["identity"]["executable"] != serde_json::to_value(candidate_digest)?
        || report["identity"]["configuration"] != serde_json::to_value(&c.identity.configuration)?
    {
        return Err(Error::Conflict(
            "rollback runtime has another storage contract, configuration, or executable",
        ));
    }
    check()?;
    staged.close()?;
    Ok(
        serde_json::json!({"schema":SCHEMA,"candidate":report["identity"],"checkpoint":Digest::of_bytes(&serde_json::to_vec(&c)?),"accounting_preserved":true,"paid_activation":"requires approval of this exact running identity; checkpoint does not grant funding or deployment authority"}),
    )
}
