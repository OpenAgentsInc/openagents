//! Disk accounting by lease holder: what a `build` lease's slot and
//! worktree hold when it ends, which session last used each slot, whether a
//! session still lives, and the per-session totals `openagents lease du`
//! reports.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, Receipt};

/// The schema of a slot's last-use record, [`SlotUse`].
pub const SLOT_USE_SCHEMA: &str = "openagents.lease.slot-use.v1";

/// What a `build` lease's slot and worktree held on disk when it ended, as
/// allocated blocks, each hard-linked file and each APFS clone family
/// counted once across both.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskUse {
    /// The target slot the lease took, when it took one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<PathBuf>,
    /// The slot's allocated bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot_bytes: Option<u64>,
    /// The holder's linked worktree, when the command ran in one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<PathBuf>,
    /// The worktree's allocated bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_bytes: Option<u64>,
    /// The slot and the worktree together, clones and hard links once.
    pub allocated_bytes: u64,
}

/// Which lease last used a target slot, written beside the slot as
/// `<slot>.lease.json` when a `build` lease that took it ends. The disk
/// cleanup reads it to reclaim the slot of a session that ended
/// (`crates/background`, class 1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotUse {
    /// [`SLOT_USE_SCHEMA`].
    pub schema: String,
    /// The slot.
    pub slot: PathBuf,
    /// The lease table the lease was in.
    pub lease_root: PathBuf,
    /// The lease's identifier.
    pub lease: String,
    /// The holder's session.
    pub session: String,
    /// The agent process the session runs in, when one was found among the
    /// holder's ancestors, so a session whose name carries no process can
    /// still be seen to live.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_pid: Option<u32>,
    /// When the lease ended, in Unix milliseconds.
    pub released_at_ms: u64,
}

impl SlotUse {
    /// The record's file beside `slot`: `<slot>.lease.json`.
    #[must_use]
    pub fn path_of(slot: &Path) -> PathBuf {
        let mut name = slot.as_os_str().to_owned();
        name.push(".lease.json");
        PathBuf::from(name)
    }

    /// Writes the record beside its slot.
    ///
    /// # Errors
    /// The file can't be written.
    pub fn write(&self) -> std::io::Result<()> {
        let mut bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        bytes.push(b'\n');
        crate::table::write_atomic(&Self::path_of(&self.slot), &bytes)
    }

    /// The record beside `slot`, when there is one that names it.
    #[must_use]
    pub fn read(slot: &Path) -> Option<SlotUse> {
        let path = Self::path_of(slot);
        if !std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_file()) {
            return None;
        }
        let record: SlotUse = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        (record.schema == SLOT_USE_SCHEMA && record.slot == slot).then_some(record)
    }
}

/// Why `session` still lives, or `None` when it has ended: a lease in the
/// table at `lease_root` names it, or the process its identity names
/// (`codex:4242`, `process:77`) or `agent_pid` still runs.
///
/// # Errors
/// The lease table can't be read.
pub fn session_live(
    lease_root: &Path,
    session: &str,
    agent_pid: Option<u32>,
) -> Result<Option<String>, Error> {
    if crate::scratch::live_sessions(lease_root)?.contains(session) {
        return Ok(Some(format!("session {session} holds a lease")));
    }
    for pid in [crate::scratch::session_pid(session), agent_pid]
        .into_iter()
        .flatten()
    {
        if process_running(pid) {
            return Ok(Some(format!("session {session} is still running")));
        }
    }
    Ok(None)
}

/// Whether process `pid` exists. A process this user may not signal still
/// exists.
#[must_use]
pub fn process_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        let Ok(pid) = libc::pid_t::try_from(pid) else {
            return true;
        };
        if pid <= 1 {
            return true;
        }
        // SAFETY: signal 0 checks only that the process exists.
        if unsafe { libc::kill(pid, 0) } == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

/// One folder a session's builds measured, as last measured.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PathUse {
    /// The slot or worktree.
    pub path: PathBuf,
    /// `slot` or `worktree`.
    pub kind: &'static str,
    /// Its allocated bytes when last measured.
    pub bytes: u64,
    /// When, in Unix milliseconds.
    pub measured_at_ms: u64,
}

/// One session's disk and build use, from its receipts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SessionUsage {
    /// The session.
    pub session: String,
    /// Its agent kind, from its latest receipt.
    pub agent: String,
    /// Whether a lease names it or its process still runs.
    pub live: bool,
    /// Leases it released.
    pub leases: u64,
    /// `build` leases among them.
    pub builds: u64,
    /// How long it held leases, in milliseconds.
    pub held_ms: u64,
    /// The sum of [`PathUse::bytes`]: each folder counted once, as last
    /// measured.
    pub allocated_bytes: u64,
    /// The folders its builds measured.
    pub paths: Vec<PathUse>,
    /// When its latest lease ended, in Unix milliseconds.
    pub last_released_at_ms: u64,
}

/// Disk and build use per session, from the receipts under `root`, the
/// largest first. A folder several receipts measured counts once, at its
/// latest measurement; a folder another session measured later counts for
/// that session. A receipt that can't be read is skipped.
///
/// # Errors
/// The receipts directory can't be listed, or the table can't be read.
pub fn usage(root: &Path) -> Result<Vec<SessionUsage>, Error> {
    let mut receipts: Vec<Receipt> = Vec::new();
    match std::fs::read_dir(root.join("receipts")) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|ext| ext != "json") {
                    continue;
                }
                if let Some(receipt) = std::fs::read(&path)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<Receipt>(&bytes).ok())
                {
                    receipts.push(receipt);
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    receipts.sort_by_key(|receipt| receipt.released_at_ms);
    // Each folder belongs to the session that measured it last.
    let mut latest: BTreeMap<PathBuf, (String, PathUse)> = BTreeMap::new();
    let mut sessions: BTreeMap<String, SessionUsage> = BTreeMap::new();
    for receipt in &receipts {
        let session = sessions
            .entry(receipt.holder.session.clone())
            .or_insert_with(|| SessionUsage {
                session: receipt.holder.session.clone(),
                agent: String::new(),
                live: false,
                leases: 0,
                builds: 0,
                held_ms: 0,
                allocated_bytes: 0,
                paths: Vec::new(),
                last_released_at_ms: 0,
            });
        session.agent.clone_from(&receipt.holder.agent);
        session.leases += 1;
        session.builds += u64::from(receipt.resource == "build");
        session.held_ms = session.held_ms.saturating_add(receipt.held_ms);
        session.last_released_at_ms = session.last_released_at_ms.max(receipt.released_at_ms);
        let Some(disk) = &receipt.disk else { continue };
        for (kind, path, bytes) in [
            ("slot", &disk.slot, disk.slot_bytes),
            ("worktree", &disk.worktree, disk.worktree_bytes),
        ] {
            if let (Some(path), Some(bytes)) = (path, bytes) {
                latest.insert(
                    path.clone(),
                    (
                        receipt.holder.session.clone(),
                        PathUse {
                            path: path.clone(),
                            kind,
                            bytes,
                            measured_at_ms: receipt.released_at_ms,
                        },
                    ),
                );
            }
        }
    }
    for (session, used) in latest.into_values() {
        if let Some(found) = sessions.get_mut(&session) {
            found.allocated_bytes = found.allocated_bytes.saturating_add(used.bytes);
            found.paths.push(used);
        }
    }
    let live = crate::scratch::live_sessions(root)?;
    let mut out: Vec<SessionUsage> = sessions
        .into_values()
        .map(|mut session| {
            session.live = live.contains(&session.session)
                || crate::scratch::session_pid(&session.session).is_some_and(process_running);
            session
        })
        .collect();
    out.sort_by(|a, b| {
        b.allocated_bytes
            .cmp(&a.allocated_bytes)
            .then_with(|| b.last_released_at_ms.cmp(&a.last_released_at_ms))
    });
    Ok(out)
}
