//! A bounded native text recipient. Registration grants no session authority.
use crate::Result;
#[cfg(unix)]
use serde_json::json;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::time::Instant;
#[cfg(unix)]
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::PathBuf,
};

pub struct FrameContext {
    pub id: String,
    pub session: String,
    pub participant: String,
    pub epoch: u64,
    pub sequence: u64,
    pub deadline: Instant,
    pub retain_until: u64,
}
pub enum Delivery {
    Delivered,
    Refused(&'static str),
    Unknown,
}
pub trait Sink {
    fn binding(&self) -> Vec<u8>;
    fn media(&self) -> &'static str;
    fn push(&mut self, context: &FrameContext, bytes: &[u8]) -> Delivery;
    fn expire(&mut self, now: u64) -> Result<()>;
    fn withdraw(&mut self) -> Result<()> {
        Err("cannot_enforce: media recipient cleanup".into())
    }
}

#[cfg(unix)]
pub struct Mailbox {
    root: PathBuf,
    identity: (u64, u64),
    _owner: File,
}
#[cfg(unix)]
impl Mailbox {
    /// Open one operator-selected private directory and hold its writer lock.
    pub fn open(root: PathBuf) -> Result<Self> {
        if !root.exists() {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&root)
                .map_err(|e| e.to_string())?;
        }
        let meta = fs::symlink_metadata(&root).map_err(|e| e.to_string())?;
        if !meta.is_dir() || meta.permissions().mode() & 0o077 != 0 {
            return Err("not_admitted: mailbox directory must be private".into());
        }
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let owner = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(root.join("writer.lock"))
            .map_err(|e| e.to_string())?;
        owner
            .try_lock()
            .map_err(|_| "conflict: mailbox already has a writer")?;
        let mut mailbox = Self {
            root,
            identity: (meta.dev(), meta.ino()),
            _owner: owner,
        };
        mailbox.expire(crate::now())?;
        Ok(mailbox)
    }
    fn check(&self) -> Result<()> {
        let meta = fs::symlink_metadata(&self.root).map_err(|e| e.to_string())?;
        if !meta.is_dir()
            || (meta.dev(), meta.ino()) != self.identity
            || meta.permissions().mode() & 0o077 != 0
        {
            return Err("not_admitted: mailbox identity changed".into());
        }
        Ok(())
    }
    fn entries(&self) -> Result<Vec<(PathBuf, String, u64, u64)>> {
        self.check()?;
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_name() == "writer.lock" {
                continue;
            }
            let name = entry
                .file_name()
                .to_str()
                .ok_or("malformed: mailbox entry")?
                .to_string();
            let parts: Vec<_> = name.split('.').collect();
            if parts.len() != 3
                || parts[2] != "txt"
                || parts[0].len() != 64
                || !parts[0]
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            {
                return Err("malformed: mailbox entry".into());
            }
            let until = parts[1].parse().map_err(|_| "malformed: mailbox expiry")?;
            let meta = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
            if !meta.is_file() {
                return Err("not_admitted: mailbox entry type".into());
            }
            entries.push((entry.path(), parts[0].into(), until, meta.len()));
            if entries.len() > 4096 {
                return Err("limit_exceeded: mailbox entries".into());
            }
        }
        Ok(entries)
    }
}
#[cfg(unix)]
impl Sink for Mailbox {
    fn binding(&self) -> Vec<u8> {
        nostr::contracts::jcs(&json!({"profile":"openagents.livehost-text-mailbox.v1",
            "source":nostr::contracts::digest_bytes(self.root.as_os_str().as_encoded_bytes()),"identity":self.identity})).unwrap_or_default()
    }
    fn media(&self) -> &'static str {
        "text"
    }
    fn push(&mut self, context: &FrameContext, bytes: &[u8]) -> Delivery {
        let Some(id) = context.id.strip_prefix("sha256:").filter(|s| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        }) else {
            return Delivery::Refused("malformed");
        };
        if bytes.len() > 8192 || std::str::from_utf8(bytes).is_err() {
            return Delivery::Refused("malformed");
        }
        let Ok(entries) = self.entries() else {
            return Delivery::Refused("unavailable");
        };
        if entries.iter().any(|(_, prior, _, _)| prior == id) {
            return Delivery::Unknown;
        }
        if entries.len() >= 4096
            || entries.iter().map(|(_, _, _, size)| size).sum::<u64>() + bytes.len() as u64
                > 1_048_576
        {
            return Delivery::Refused("limit_exceeded");
        }
        if Instant::now() >= context.deadline {
            return Delivery::Refused("stale");
        }
        let path = self.root.join(format!("{id}.{}.txt", context.retain_until));
        let Ok(mut file) = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        else {
            return Delivery::Unknown;
        };
        if Instant::now() >= context.deadline {
            return if fs::remove_file(path).is_ok() {
                Delivery::Refused("stale")
            } else {
                Delivery::Unknown
            };
        }
        if file
            .write_all(bytes)
            .and_then(|_| file.sync_all())
            .and_then(|_| File::open(&self.root)?.sync_all())
            .is_err()
        {
            return Delivery::Unknown;
        }
        Delivery::Delivered
    }
    fn withdraw(&mut self) -> Result<()> {
        for (path, _, _, _) in self.entries()? {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        File::open(&self.root)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())
    }
    fn expire(&mut self, now: u64) -> Result<()> {
        for (path, _, until, _) in self.entries()? {
            if until <= now {
                fs::remove_file(path).map_err(|e| e.to_string())?;
            }
        }
        File::open(&self.root)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())
    }
}
