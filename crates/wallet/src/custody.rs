//! Persistent shared custody and exact once-only controller handoffs.
use crate::WalletError;
use hmac::{Hmac, Mac};
use rusqlite::{Connection, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

pub const FILE: &str = "shared-custody.json";
pub const REQUIRED_FILE: &str = "shared-custody.required";
pub const SCHEMA: &str = "openagents.wallet.shared-custody.v1";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub origin: String,
    pub node: String,
    pub controller: PathBuf,
    pub writer_digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Terms {
    Pay {
        invoice: String,
        max_fee_msat: u64,
        wait_secs: u64,
    },
    Receive {
        amount_msat: u64,
        request_hash: String,
        expiry_secs: u32,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Permit {
    pub origin: String,
    pub node: String,
    pub intent: String,
    pub terms: Terms,
    pub authentication: String,
}
impl Permit {
    fn bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&(&self.origin, &self.node, &self.intent, &self.terms))
            .expect("custody terms serialize")
    }
    pub fn sign(
        origin: &str,
        node: &str,
        intent: &str,
        terms: Terms,
        writer: &str,
    ) -> Result<Self, WalletError> {
        if writer.len() != 64 || hex::decode(writer).is_err() || intent.len() != 64 {
            return Err(WalletError::Invalid(
                "Exact private custody authority required.".into(),
            ));
        }
        let mut permit = Self {
            origin: origin.into(),
            node: node.into(),
            intent: intent.into(),
            terms,
            authentication: String::new(),
        };
        let mut mac = Hmac::<Sha256>::new_from_slice(writer.as_bytes())
            .map_err(|_| WalletError::Invalid("Custody authentication unavailable.".into()))?;
        mac.update(&permit.bytes());
        permit.authentication = hex::encode(mac.finalize().into_bytes());
        Ok(permit)
    }
}
fn problem() -> WalletError {
    WalletError::Setup(
        "Shared custody is unavailable or changed; native financial operations remain disabled."
            .into(),
    )
}
fn private(path: &Path, directory: bool) -> Result<std::fs::Metadata, WalletError> {
    let m = std::fs::symlink_metadata(path).map_err(|_| problem())?;
    if m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || if directory {
            !m.is_dir()
        } else {
            !m.is_file() || m.nlink() != 1
        }
    {
        return Err(problem());
    }
    Ok(m)
}
pub fn read(home: &Path) -> Result<Option<Manifest>, WalletError> {
    match std::fs::symlink_metadata(home.join(FILE)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            for retained in [REQUIRED_FILE, "shared-handoffs.sqlite"] {
                if !matches!(std::fs::symlink_metadata(home.join(retained)),Err(e) if e.kind()==std::io::ErrorKind::NotFound)
                {
                    return Err(problem());
                }
            }
            return Ok(None);
        }
        Err(_) => return Err(problem()),
        Ok(_) => {}
    }
    let root = private(home, true)?;
    let path = home.join(FILE);
    let current = private(&path, false)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)
        .map_err(|_| problem())?;
    let opened = file.metadata().map_err(|_| problem())?;
    if opened.dev() != current.dev() || opened.ino() != current.ino() || opened.len() > 8192 {
        return Err(problem());
    }
    let mut bytes = Vec::new();
    file.take(8193)
        .read_to_end(&mut bytes)
        .map_err(|_| problem())?;
    let end = private(home, true)?;
    let end_file = private(&path, false)?;
    if root.dev() != end.dev()
        || root.ino() != end.ino()
        || opened.dev() != end_file.dev()
        || opened.ino() != end_file.ino()
    {
        return Err(problem());
    }
    let required_path = home.join(REQUIRED_FILE);
    let required_meta = private(&required_path, false)?;
    let required = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&required_path)
        .map_err(|_| problem())?;
    let opened = required.metadata().map_err(|_| problem())?;
    if opened.len() != 64
        || (opened.dev(), opened.ino()) != (required_meta.dev(), required_meta.ino())
    {
        return Err(problem());
    }
    let mut required_bytes = Vec::new();
    required
        .take(65)
        .read_to_end(&mut required_bytes)
        .map_err(|_| problem())?;
    let final_required = private(&required_path, false)?;
    if required_bytes != format!("{:x}", Sha256::digest(&bytes)).as_bytes()
        || (required_meta.dev(), required_meta.ino())
            != (final_required.dev(), final_required.ino())
    {
        return Err(problem());
    }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|_| problem())?;
    if manifest.schema != SCHEMA
        || manifest.origin.len() != 64
        || manifest.node.len() != 66
        || manifest.writer_digest.len() != 64
        || !manifest.controller.is_absolute()
    {
        return Err(problem());
    }
    Ok(Some(manifest))
}
/// Once installed, ordinary wallet paths cannot rediscover standalone spending.
pub fn refuse_raw(home: &Path) -> Result<(), WalletError> {
    if read(home)?.is_some() {
        Err(WalletError::Setup(
            "This wallet belongs to the shared spend controller; use an exact admitted intent."
                .into(),
        ))
    } else {
        Ok(())
    }
}
/// The owner calls this only after proving the resident has no funds or open payments.
pub fn install(home: &Path, manifest: &Manifest) -> Result<(), WalletError> {
    private(home, true)?;
    if let Some(old) = read(home)? {
        if &old == manifest {
            return Ok(());
        }
        return Err(problem());
    }
    let manifest_bytes = serde_json::to_vec(manifest).map_err(|_| problem())?;
    let mut required = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(home.join(REQUIRED_FILE))
        .map_err(|_| problem())?;
    required
        .write_all(format!("{:x}", Sha256::digest(&manifest_bytes)).as_bytes())
        .map_err(|_| problem())?;
    required.sync_all().map_err(|_| problem())?;
    File::open(home)
        .and_then(|f| f.sync_all())
        .map_err(|_| problem())?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(home.join(FILE))
        .map_err(|_| problem())?;
    file.write_all(&manifest_bytes).map_err(|_| problem())?;
    file.sync_all().map_err(|_| problem())?;
    File::open(home)
        .and_then(|f| f.sync_all())
        .map_err(|_| problem())?;
    read(home)?.ok_or_else(problem)?;
    Ok(())
}
/// Only the resident constructs this after durable permit consumption.
pub struct Admitted {
    permit: Permit,
    taken: AtomicBool,
    home: std::path::PathBuf,
    root: (u64, u64),
    store: (u64, u64),
    manifest: Manifest,
}
impl Admitted {
    pub(crate) fn seal_result(&self, result: &serde_json::Value) -> Result<(), WalletError> {
        let root = private(&self.home, true)?;
        let path = self.home.join("shared-handoffs.sqlite");
        let store = private(&path, false)?;
        if (root.dev(), root.ino()) != self.root
            || (store.dev(), store.ino()) != self.store
            || read(&self.home)?.as_ref() != Some(&self.manifest)
        {
            return Err(problem());
        }
        let db = Connection::open(&path).map_err(|_| problem())?;
        let bytes = serde_json::to_string(result).map_err(|_| problem())?;
        db.execute(
            "INSERT INTO outcome(intent,bytes) VALUES(?,?)",
            params![self.permit.intent, bytes],
        )
        .map_err(|_| problem())?;
        Ok(())
    }
    pub fn take(&self) -> Result<&Permit, WalletError> {
        let root = private(&self.home, true)?;
        let store = private(&self.home.join("shared-handoffs.sqlite"), false)?;
        if (root.dev(), root.ino()) != self.root
            || (store.dev(), store.ino()) != self.store
            || read(&self.home)?.as_ref() != Some(&self.manifest)
        {
            return Err(problem());
        }
        if self.taken.swap(true, Ordering::SeqCst) {
            return Err(WalletError::Invalid(
                "This exact custody handoff was already used.".into(),
            ));
        }
        Ok(&self.permit)
    }
}
pub(crate) fn admit(
    home: &Path,
    node: &str,
    permit: Permit,
    writer: &str,
) -> Result<Admitted, WalletError> {
    let root = private(home, true)?;
    let manifest = read(home)?.ok_or_else(problem)?;
    if manifest.node != node
        || permit.node != node
        || permit.origin != manifest.origin
        || format!("{:x}", Sha256::digest(writer.as_bytes())) != manifest.writer_digest
    {
        return Err(problem());
    }
    let mut mac = Hmac::<Sha256>::new_from_slice(writer.as_bytes()).map_err(|_| problem())?;
    mac.update(&permit.bytes());
    mac.verify_slice(&hex::decode(&permit.authentication).map_err(|_| problem())?)
        .map_err(|_| problem())?;
    if permit.intent.len() != 64
        || serde_json::to_vec(&permit).map_err(|_| problem())?.len() > 40 * 1024
    {
        return Err(problem());
    }
    // Recheck the exact native intent after waiting for this resident's
    // financial lock. The controller owns current native spend admission.
    use std::io::{BufRead, BufReader};
    use std::os::unix::net::UnixStream;
    let parent = manifest.controller.parent().ok_or_else(problem)?;
    private(parent, true)?;
    let socket_meta = std::fs::symlink_metadata(&manifest.controller).map_err(|_| problem())?;
    use std::os::unix::fs::FileTypeExt;
    if !socket_meta.file_type().is_socket()
        || socket_meta.uid() != unsafe { libc::geteuid() }
        || socket_meta.mode() & 0o077 != 0
    {
        return Err(problem());
    }
    let mut socket = UnixStream::connect(&manifest.controller).map_err(|_| problem())?;
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|_| problem())?;
    socket
        .set_write_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|_| problem())?;
    let body = serde_json::to_vec(&serde_json::json!({"custody_authorization":permit}))
        .map_err(|_| problem())?;
    socket
        .write_all(&body)
        .and_then(|_| socket.write_all(b"\n"))
        .map_err(|_| problem())?;
    let mut reply = String::new();
    BufReader::new(socket)
        .take(8193)
        .read_line(&mut reply)
        .map_err(|_| problem())?;
    let reply: serde_json::Value = serde_json::from_str(&reply).map_err(|_| problem())?;
    if reply["authorized"] != true
        || reply["origin"] != permit.origin
        || reply["intent"] != permit.intent
    {
        return Err(problem());
    }
    let end = std::fs::symlink_metadata(&manifest.controller).map_err(|_| problem())?;
    if (end.dev(), end.ino()) != (socket_meta.dev(), socket_meta.ino())
        || read(home)?.as_ref() != Some(&manifest)
    {
        return Err(problem());
    }
    let path = home.join("shared-handoffs.sqlite");
    match OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
    {
        Ok(f) => {
            f.sync_all().map_err(|_| problem())?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(problem()),
    }
    let store = private(&path, false)?;
    let mut db = Connection::open(&path).map_err(|_| problem())?;
    db.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|_| problem())?;
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS handoff(intent TEXT PRIMARY KEY,bytes TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS outcome(intent TEXT PRIMARY KEY REFERENCES handoff(intent),bytes TEXT NOT NULL);
         CREATE TRIGGER IF NOT EXISTS handoff_no_update BEFORE UPDATE ON handoff BEGIN SELECT RAISE(ABORT,'Original handoff is immutable'); END;
         CREATE TRIGGER IF NOT EXISTS handoff_no_delete BEFORE DELETE ON handoff BEGIN SELECT RAISE(ABORT,'Original handoff is retained'); END;
         CREATE TRIGGER IF NOT EXISTS outcome_no_update BEFORE UPDATE ON outcome BEGIN SELECT RAISE(ABORT,'Original outcome is immutable'); END;
         CREATE TRIGGER IF NOT EXISTS outcome_no_delete BEFORE DELETE ON outcome BEGIN SELECT RAISE(ABORT,'Original outcome is retained'); END;",
    )
    .map_err(|_| problem())?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_| problem())?;
    tx.execute("INSERT INTO handoff(intent,bytes) VALUES(?,?)",params![permit.intent,serde_json::to_string(&permit).map_err(|_|problem())?]).map_err(|_|WalletError::Setup("The original custody handoff is already sealed; look up its outcome without another payment or invoice.".into()))?;
    tx.commit().map_err(|_| problem())?;
    let end = private(&path, false)?;
    let end_root = private(home, true)?;
    if (end.dev(), end.ino()) != (store.dev(), store.ino())
        || (end_root.dev(), end_root.ino()) != (root.dev(), root.ino())
    {
        return Err(problem());
    }
    Ok(Admitted {
        permit,
        taken: AtomicBool::new(false),
        home: home.into(),
        root: (root.dev(), root.ino()),
        store: (store.dev(), store.ino()),
        manifest,
    })
}

/// Read a sealed original result without authorizing another financial effect.
pub(crate) fn result(
    home: &Path,
    node: &str,
    intent: &str,
    writer: &str,
) -> Result<Option<serde_json::Value>, WalletError> {
    use rusqlite::OptionalExtension;
    let root = private(home, true)?;
    let manifest = read(home)?.ok_or_else(problem)?;
    if manifest.node != node
        || intent.len() != 64
        || format!("{:x}", Sha256::digest(writer.as_bytes())) != manifest.writer_digest
    {
        return Err(problem());
    }
    let path = home.join("shared-handoffs.sqlite");
    if !path.exists() {
        return Ok(None);
    }
    let store = private(&path, false)?;
    let db = Connection::open_with_flags(
        &path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| problem())?;
    let found=db.query_row("SELECT handoff.bytes,outcome.bytes FROM handoff LEFT JOIN outcome USING(intent) WHERE intent=?",[intent],|r|Ok((r.get::<_,String>(0)?,r.get::<_,Option<String>>(1)?))).optional().map_err(|_|problem())?;
    let end = private(&path, false)?;
    let end_root = private(home, true)?;
    if (root.dev(), root.ino()) != (end_root.dev(), end_root.ino())
        || (store.dev(), store.ino()) != (end.dev(), end.ino())
        || read(home)?.as_ref() != Some(&manifest)
    {
        return Err(problem());
    }
    match found {
        Some((permit, Some(result))) => Ok(Some(
            serde_json::json!({"permit":serde_json::from_str::<Permit>(&permit).map_err(|_|problem())?,"result":serde_json::from_str::<serde_json::Value>(&result).map_err(|_|problem())?}),
        )),
        _ => Ok(None),
    }
}
