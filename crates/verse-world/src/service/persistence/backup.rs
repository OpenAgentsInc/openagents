//! Offline, exclusively owned exports and verified restoration into new storage.
use super::{Committed, FILE_BYTES, Store, digest, journal, migration};
use crate::service::{rewards::history::History, save};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

const MANIFEST_BYTES: usize = 16 * 1024 * 1024;
const META_BYTES: usize = 4096;
const NODE_BYTES: usize = 256 * 1024;
/// Export budgets bound this operation, not the world's reward lifetime.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    pub files: usize,
    pub bytes: u64,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            files: 65_536,
            bytes: 1024 * 1024 * 1024,
        }
    }
}
impl Budget {
    fn validate(self) -> Result<(), String> {
        if self.files == 0
            || self.files > 65_536
            || self.bytes == 0
            || self.bytes > 16 * 1024 * 1024 * 1024
        {
            return Err("Backup budget is invalid".into());
        }
        Ok(())
    }
}
/// The report contains scope and digests, never the checkpoint or character data.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema: String,
    pub content: [u8; 32],
    pub instance: u64,
    pub revision: u64,
    pub checkpoint_digest: [u8; 32],
    pub manifest_digest: [u8; 32],
    pub files: usize,
    pub bytes: u64,
    pub migrations: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Pruned {
    pub retained_nodes: usize,
    pub removed_files: usize,
    pub removed_bytes: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: String,
    bytes: usize,
    digest: [u8; 32],
}
fn hashed(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn private(path: &Path, directory: bool) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| "Cannot inspect backup input")?;
    if (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
        return Err("Backup input is not a regular file or directory".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("Backup inputs require owner-only permissions".into());
        }
    }
    Ok(())
}
fn ancestors(path: &Path) -> Result<(), String> {
    for part in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
        match std::fs::symlink_metadata(part) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err("Backup paths cannot contain symlinks".into());
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("Cannot inspect backup path".into()),
        }
    }
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("Backup paths cannot contain parent traversal".into());
    }
    Ok(())
}
/// Admits an existing private source before a caller acquires its store.
pub fn validate_source_path(path: &Path) -> Result<(), String> {
    ancestors(path)?;
    private(path, true)
}
fn new_dir(path: &Path) -> Result<(), String> {
    ancestors(path)?;
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .map_err(|_| "Backup or restore destination must be a new directory")?;
    sync(
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )
}
fn sync(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|_| "Cannot sync backup directory".into())
}
fn read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    private(path, false)?;
    let mut bytes = vec![];
    File::open(path)
        .map_err(|_| "Cannot open backup file")?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read backup file")?;
    if bytes.len() > limit {
        return Err("Backup file exceeds byte budget".into());
    }
    Ok(bytes)
}
fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| "Backup destination file already exists or cannot be created")?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Cannot write and sync backup file".into())
}
fn node(name: &str) -> Result<[u8; 32], String> {
    if name.len() != 64
        || !name
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err("Backup history name is invalid".into());
    }
    let mut hash = [0; 32];
    for (index, byte) in hash.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&name[index * 2..index * 2 + 2], 16)
            .map_err(|_| "Invalid backup history digest")?;
    }
    Ok(hash)
}
fn allowed(name: &str) -> Result<usize, String> {
    if name == "chamber.json" {
        return Ok(FILE_BYTES);
    }
    let parts: Vec<_> = name.split('/').collect();
    if parts.len() == 2 && parts[0] == "rewards" {
        node(parts[1])?;
        return Ok(NODE_BYTES);
    }
    if parts.len() == 3 && parts[0] == "migrations" {
        node(parts[1])?;
        return match parts[2] {
            "before.json" | "after.json" => Ok(FILE_BYTES),
            "record.json" | "seal.json" => Ok(NODE_BYTES),
            _ => Err("Unexpected migration backup file".into()),
        };
    }
    Err("Backup manifest contains an unexpected path".into())
}
fn archive(root: &Path, id: &str, history: &History, instance: u64) -> Result<(), String> {
    let (record, before, after) = migration::load_record(root, id)?;
    read(
        &root.join("migrations").join(id).join("seal.json"),
        NODE_BYTES,
    )?;
    migration::sealed(&root.join("migrations").join(id), &record)?;
    for (saved, content) in [
        (before, record.review.source.content),
        (after, record.review.target.content),
    ] {
        save::decode_with_history(
            saved.checkpoint.as_bytes(),
            content,
            instance,
            Some(history.clone()),
        )?;
    }
    Ok(())
}
impl Store {
    fn backup_roots(&self, budget: Budget) -> Result<BTreeSet<[u8; 32]>, String> {
        fn collect(
            state: &serde_json::Value,
            history: &History,
            nodes: &mut BTreeSet<[u8; 32]>,
            limit: usize,
        ) -> Result<(), String> {
            if !state["ledger"].is_null() {
                let ledger: crate::service::rewards::Checkpoint =
                    serde_json::from_value(state["ledger"].clone())
                        .map_err(|_| "Invalid backup reward ledger")?;
                for root in ledger.history_roots() {
                    history.reachable(root, nodes, limit)?;
                }
            }
            Ok(())
        }
        let mut nodes = BTreeSet::new();
        collect(
            self.state
                .as_ref()
                .ok_or("No durable checkpoint to export")?,
            &self.history,
            &mut nodes,
            budget.files,
        )?;
        if std::fs::symlink_metadata(self.root.join("migrations")).is_ok() {
            private(&self.root.join("migrations"), true)?;
            for (n, entry) in std::fs::read_dir(self.root.join("migrations"))
                .map_err(|_| "Cannot enumerate migration roots")?
                .enumerate()
            {
                if n >= budget.files / 4 {
                    return Err("Migration archive count exceeds backup budget".into());
                }
                let entry = entry.map_err(|_| "Cannot read migration archive entry")?;
                let id = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| "Invalid migration identity")?;
                node(&id)?;
                archive(&self.root, &id, &self.history, self.instance)?;
                let (_, before, after) = migration::load_record(&self.root, &id)?;
                for saved in [before, after] {
                    collect(
                        &journal::expand(saved.checkpoint.as_bytes())?,
                        &self.history,
                        &mut nodes,
                        budget.files,
                    )?;
                }
            }
        }
        Ok(nodes)
    }
    /// Removes only files unreachable from current state and all migration archives.
    /// Requires an unopened recovered authority and refuses a tree above the scan budget.
    pub fn prune_history(&self, budget: Budget) -> Result<Pruned, String> {
        budget.validate()?;
        ancestors(&self.root)?;
        if self.poisoned || self.recovered.is_none() {
            return Err(
                "History pruning requires exclusively opened offline recovered storage".into(),
            );
        }
        let retained = self.backup_roots(budget)?;
        // Plan before deletion, so unknown files, symlinks, and budget failures change nothing.
        let mut removal = vec![];
        let mut bytes = 0u64;
        for (n, entry) in std::fs::read_dir(self.root.join("rewards"))
            .map_err(|_| "Cannot enumerate reward retention")?
            .enumerate()
        {
            if n >= budget.files {
                return Err("History scan exceeds file budget".into());
            }
            let entry = entry.map_err(|_| "Cannot read reward retention entry")?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "Invalid reward retention filename")?;
            private(&entry.path(), false)?;
            let temporary = name.strip_prefix("next-").is_some_and(|suffix| {
                suffix.len() == 32
                    && suffix
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            });
            if !temporary && retained.contains(&node(&name)?) {
                continue;
            }
            let size = std::fs::symlink_metadata(entry.path())
                .map_err(|_| "Cannot inspect reward retention file")?
                .len();
            bytes = bytes
                .checked_add(size)
                .filter(|bytes| *bytes <= budget.bytes)
                .ok_or("History retention exceeds byte budget")?;
            removal.push(entry.path());
        }
        for path in &removal {
            std::fs::remove_file(path)
                .map_err(|_| "Cannot remove unreferenced reward history file")?;
        }
        sync(&self.root.join("rewards"))?;
        Ok(Pruned {
            retained_nodes: retained.len(),
            removed_files: removal.len(),
            removed_bytes: bytes,
        })
    }
    /// Holds this store's writer lock throughout export of the last durable revision.
    pub fn export_backup(&self, destination: &Path, budget: Budget) -> Result<Report, String> {
        budget.validate()?;
        ancestors(&self.root)?;
        if self.poisoned {
            return Err("Cannot export unavailable storage".into());
        }
        let checkpoint = String::from_utf8(journal::contract(
            self.state
                .as_ref()
                .ok_or("No durable checkpoint to export")?,
        )?)
        .map_err(|_| "Invalid durable checkpoint encoding")?;
        let committed = Committed {
            version: 1,
            revision: self.revision,
            digest: digest(self.revision, &checkpoint),
            checkpoint,
        };
        save::decode_with_history(
            committed.checkpoint.as_bytes(),
            self.content,
            self.instance,
            Some(self.history.clone()),
        )?;
        new_dir(destination)?;
        write(
            &destination.join("backup.pending"),
            b"verse.backup.pending.v1\n",
        )?;
        new_dir(&destination.join("rewards"))?;
        let mut manifest = Vec::new();
        let mut report = Report {
            schema: "verse.backup.v1".into(),
            content: self.content,
            instance: self.instance,
            revision: self.revision,
            checkpoint_digest: committed.digest,
            manifest_digest: [0; 32],
            files: 0,
            bytes: 0,
            migrations: 0,
        };
        let mut copy = |name: &str, bytes: Vec<u8>| -> Result<(), String> {
            let limit = allowed(name)?;
            if bytes.len() > limit
                || report.files >= budget.files
                || report
                    .bytes
                    .checked_add(bytes.len() as u64)
                    .is_none_or(|n| n > budget.bytes)
            {
                return Err("Backup exceeds its file or byte budget".into());
            }
            let entry = Entry {
                path: name.into(),
                bytes: bytes.len(),
                digest: hashed(&bytes),
            };
            let line = serde_json::to_vec(&entry).map_err(|_| "Cannot encode backup manifest")?;
            if manifest.len() + line.len() + 1 > MANIFEST_BYTES {
                return Err("Backup manifest exceeds byte budget".into());
            }
            write(&destination.join(name), &bytes)?;
            manifest.extend(line);
            manifest.push(b'\n');
            report.files += 1;
            report.bytes += bytes.len() as u64;
            Ok(())
        };
        copy(
            "chamber.json",
            serde_json::to_vec(&committed).map_err(|_| "Cannot encode backup checkpoint")?,
        )?;
        for hash in self.backup_roots(budget)? {
            let name = hash
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            copy(
                &format!("rewards/{name}"),
                read(&self.root.join("rewards").join(name), NODE_BYTES)?,
            )?;
        }
        if std::fs::symlink_metadata(self.root.join("migrations")).is_ok() {
            private(&self.root.join("migrations"), true)?;
            new_dir(&destination.join("migrations"))?;
            for entry in std::fs::read_dir(self.root.join("migrations"))
                .map_err(|_| "Cannot enumerate migration archives")?
            {
                let entry = entry.map_err(|_| "Cannot read migration archive entry")?;
                let id = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| "Invalid migration archive name")?;
                node(&id)?;
                archive(&self.root, &id, &self.history, self.instance)?;
                new_dir(&destination.join("migrations").join(&id))?;
                for name in ["before.json", "after.json", "record.json", "seal.json"] {
                    copy(
                        &format!("migrations/{id}/{name}"),
                        read(
                            &entry.path().join(name),
                            allowed(&format!("migrations/{id}/{name}"))?,
                        )?,
                    )?;
                }
                sync(&destination.join("migrations").join(&id))?;
                report.migrations += 1;
            }
            sync(&destination.join("migrations"))?;
        }
        sync(&destination.join("rewards"))?;
        report.manifest_digest = hashed(&manifest);
        write(&destination.join("files.jsonl"), &manifest)?;
        write(
            &destination.join("backup.json"),
            &serde_json::to_vec(&report).map_err(|_| "Cannot encode backup report")?,
        )?;
        sync(destination)?;
        std::fs::remove_file(destination.join("backup.pending"))
            .map_err(|_| "Cannot seal backup")?;
        sync(destination)?;
        verify(destination, self.content, self.instance, budget)
    }
}
fn entries(root: &Path, report: &Report, budget: Budget) -> Result<Vec<Entry>, String> {
    let manifest = read(&root.join("files.jsonl"), MANIFEST_BYTES)?;
    if hashed(&manifest) != report.manifest_digest || !manifest.ends_with(b"\n") {
        return Err("Backup manifest digest is invalid".into());
    }
    let mut entries = vec![];
    let mut names = BTreeSet::new();
    let mut bytes = 0u64;
    for line in manifest
        .split(|c| *c == b'\n')
        .filter(|line| !line.is_empty())
    {
        if line.len() > 1024 || entries.len() >= budget.files {
            return Err("Backup manifest exceeds record budget".into());
        }
        let entry: Entry =
            serde_json::from_slice(line).map_err(|_| "Invalid backup manifest record")?;
        if entry.bytes > allowed(&entry.path)? || !names.insert(entry.path.clone()) {
            return Err("Backup manifest path or size is invalid".into());
        }
        bytes = bytes
            .checked_add(entry.bytes as u64)
            .filter(|b| *b <= budget.bytes)
            .ok_or("Backup exceeds byte budget")?;
        let data = read(&root.join(&entry.path), entry.bytes)?;
        if data.len() != entry.bytes || hashed(&data) != entry.digest {
            return Err("Backup file checksum is invalid".into());
        }
        entries.push(entry);
    }
    if entries.len() != report.files || bytes != report.bytes || !names.contains("chamber.json") {
        return Err("Backup manifest totals are invalid".into());
    }
    // Refuse unlisted files, nested paths, and symlinks rather than silently copying them.
    fn scan(
        root: &Path,
        current: &Path,
        names: &BTreeSet<String>,
        depth: usize,
        count: &mut usize,
    ) -> Result<(), String> {
        if depth > 3 {
            return Err("Backup directory depth exceeded".into());
        }
        private(current, true)?;
        for entry in std::fs::read_dir(current).map_err(|_| "Cannot enumerate backup files")? {
            let entry = entry.map_err(|_| "Cannot read backup directory entry")?;
            let path = entry.path();
            let name = path
                .strip_prefix(root)
                .map_err(|_| "Invalid backup path")?
                .to_str()
                .ok_or("Invalid backup path encoding")?
                .replace('\\', "/");
            if depth == 0
                && matches!(
                    name.as_str(),
                    "backup.json" | "files.jsonl" | "restore.pending" | "writer.lock"
                )
            {
                private(&path, false)?;
                continue;
            }
            let metadata =
                std::fs::symlink_metadata(&path).map_err(|_| "Cannot inspect backup entry")?;
            if metadata.is_dir() {
                if !(name == "rewards"
                    || name == "migrations"
                    || (name.starts_with("migrations/")
                        && name.split('/').count() == 2
                        && node(name.split('/').nth(1).unwrap()).is_ok()))
                {
                    return Err("Unexpected backup directory".into());
                }
                if name.starts_with("migrations/") {
                    let prefix = format!("{name}/");
                    if names
                        .range(prefix.clone()..)
                        .next()
                        .is_none_or(|entry| !entry.starts_with(&prefix))
                    {
                        return Err("Unlisted migration backup directory".into());
                    }
                }
                scan(root, &path, names, depth + 1, count)?;
            } else {
                private(&path, false)?;
                if !names.contains(&name) {
                    return Err("Unlisted backup file".into());
                }
                *count += 1;
            }
        }
        Ok(())
    }
    let mut count = 0;
    scan(root, root, &names, 0, &mut count)?;
    if count != entries.len() {
        return Err("Backup file count is invalid".into());
    }
    Ok(entries)
}
/// Verifies every file and recovers receipt, character, migration, and world state.
pub fn verify(
    root: &Path,
    content: [u8; 32],
    instance: u64,
    budget: Budget,
) -> Result<Report, String> {
    verify_inner(root, content, instance, budget, false)
}
fn verify_inner(
    root: &Path,
    content: [u8; 32],
    instance: u64,
    budget: Budget,
    restoring: bool,
) -> Result<Report, String> {
    budget.validate()?;
    ancestors(root)?;
    private(root, true)?;
    if std::fs::symlink_metadata(root.join("backup.pending")).is_ok()
        || (!restoring && std::fs::symlink_metadata(root.join("restore.pending")).is_ok())
    {
        return Err("Backup is incomplete".into());
    }
    let report: Report = serde_json::from_slice(&read(&root.join("backup.json"), META_BYTES)?)
        .map_err(|_| "Invalid backup report")?;
    if report.schema != "verse.backup.v1"
        || report.instance != instance
        || report.content != content
        || report.revision == 0
    {
        return Err("Backup context is incompatible".into());
    }
    let list = entries(root, &report, budget)?;
    private(&root.join("rewards"), true)?;
    let history = History::open(&root.join("rewards"))?;
    for entry in &list {
        if let Some(name) = entry.path.strip_prefix("rewards/") {
            history.verify_node(node(name)?)?;
        }
    }
    let saved: Committed = serde_json::from_slice(&read(&root.join("chamber.json"), FILE_BYTES)?)
        .map_err(|_| "Invalid backup checkpoint")?;
    if saved.version != 1
        || saved.revision != report.revision
        || saved.digest != report.checkpoint_digest
        || saved.digest != digest(saved.revision, &saved.checkpoint)
    {
        return Err("Backup checkpoint checksum is invalid".into());
    }
    save::decode_with_history(
        saved.checkpoint.as_bytes(),
        content,
        instance,
        Some(history.clone()),
    )?;
    let ids: BTreeSet<_> = list
        .iter()
        .filter(|e| e.path.starts_with("migrations/"))
        .map(|e| e.path.split('/').nth(1).unwrap())
        .collect();
    if ids.len() != report.migrations {
        return Err("Backup migration count is invalid".into());
    }
    for id in ids {
        archive(root, id, &history, instance)?;
    }
    Ok(report)
}
/// Reserves a new destination and holds its writer lock until verified publication.
/// An interrupted restore retains `restore.pending`, which host admission refuses.
pub fn restore(
    source: &Path,
    destination: &Path,
    content: [u8; 32],
    instance: u64,
    budget: Budget,
) -> Result<Report, String> {
    restore_observed(source, destination, content, instance, budget, |_| {})
}
fn restore_observed(
    source: &Path,
    destination: &Path,
    content: [u8; 32],
    instance: u64,
    budget: Budget,
    boundary: impl Fn(&str),
) -> Result<Report, String> {
    let report = verify(source, content, instance, budget)?;
    new_dir(destination)?;
    write(&destination.join("writer.lock"), b"")?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(destination.join("writer.lock"))
        .map_err(|_| "Cannot open restore writer lock")?;
    lock.try_lock()
        .map_err(|_| "Restore destination already has a writer")?;
    // A racing host must not have created any files before this lock was acquired.
    if std::fs::read_dir(destination)
        .map_err(|_| "Cannot inspect restore reservation")?
        .count()
        != 1
    {
        return Err("Restore destination changed before reservation".into());
    }
    write(
        &destination.join("restore.pending"),
        b"verse.restore.pending.v1\n",
    )?;
    sync(destination)?;
    boundary("reserved");
    new_dir(&destination.join("rewards"))?;
    let list = entries(source, &report, budget)?;
    let mut directories = BTreeSet::new();
    for entry in &list {
        let target = destination.join(&entry.path);
        if entry.path.starts_with("migrations/") {
            if directories.insert("migrations".to_string()) {
                new_dir(&destination.join("migrations"))?;
            }
            let name = format!("migrations/{}", entry.path.split('/').nth(1).unwrap());
            if directories.insert(name.clone()) {
                new_dir(&destination.join(name))?;
            }
        }
        let bytes = read(&source.join(&entry.path), entry.bytes)?;
        if bytes.len() != entry.bytes || hashed(&bytes) != entry.digest {
            return Err("Backup changed during restore".into());
        }
        write(&target, &bytes)?;
    }
    write(
        &destination.join("files.jsonl"),
        &read(&source.join("files.jsonl"), MANIFEST_BYTES)?,
    )?;
    write(
        &destination.join("backup.json"),
        &serde_json::to_vec(&report).map_err(|_| "Cannot encode restored backup report")?,
    )?;
    if verify_inner(destination, content, instance, budget, true)? != report {
        return Err("Restored backup identity changed".into());
    }
    boundary("copied");
    for name in directories.iter().rev() {
        sync(&destination.join(name))?;
    }
    sync(&destination.join("rewards"))?;
    sync(destination)?;
    boundary("durable");
    std::fs::remove_file(destination.join("files.jsonl"))
        .map_err(|_| "Cannot finalize restored manifest")?;
    std::fs::remove_file(destination.join("backup.json"))
        .map_err(|_| "Cannot finalize restored backup")?;
    sync(destination)?;
    boundary("before_publish");
    std::fs::remove_file(destination.join("restore.pending"))
        .map_err(|_| "Cannot finalize restore reservation")?;
    sync(destination)?;
    boundary("published");
    drop(lock);
    Ok(report)
}

#[cfg(test)]
mod tests;
