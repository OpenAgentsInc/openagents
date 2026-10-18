//! Read-only machine facts for any agent, with edge-triggered resource alerts.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::{agent_builds, machine};

pub const DISK_FLOOR_GIB: u64 = 100;
const TAIL_BYTES: u64 = 64 * 1024;

#[derive(Debug, Serialize)]
pub struct MachineFacts {
    pub agent: String,
    pub computer: String,
    pub observed_at_ms: u64,
    pub disk_free_gib: Option<f64>,
    pub disk_floor_gib: u64,
    pub disk_below_floor: Option<bool>,
    /// Held slots and the app build gate's limit.
    pub build_leases: Option<(u64, u64)>,
    pub build_waiting: Option<usize>,
    pub cargo_jobs: Option<u64>,
    pub builds_over_cap: Option<bool>,
    /// The sweep records its timestamp in Chicago time.
    pub sweep_last_run: Option<String>,
    pub sweep: Option<Value>,
    pub sweep_recent_evictions: Vec<String>,
    pub granted_scopes: Vec<machine::Scope>,
    pub recent_machine_actions: Vec<machine::Entry>,
    pub errors: BTreeMap<String, String>,
}

/// Collect local files without creating directories, taking leases, or pruning.
/// Process and disk readings are supplied so callers and tests share one parser.
pub fn collect(
    home: &Path,
    root: &Path,
    agent: &str,
    computer: &str,
    now_ms: u64,
    free_bytes: Result<u64, String>,
    cargo: Result<String, String>,
) -> MachineFacts {
    crate::refuse_real_home(root);
    let mut errors = BTreeMap::new();
    let mut take = |key: &str, value: Result<_, String>| match value {
        Ok(value) => Some(value),
        Err(error) => {
            errors.insert(key.to_owned(), error);
            None
        }
    };
    let free = take("disk", free_bytes);
    let cargo_jobs = match cargo.and_then(|text| cargo_count(&text)) {
        Ok(n) => Some(n),
        Err(error) => {
            errors.insert("cargo".into(), error);
            None
        }
    };
    let status = match agent_builds::status_with(root, home, DISK_FLOOR_GIB << 30, &|_| {
        free.ok_or_else(|| std::io::Error::other("disk reading unavailable"))
    }) {
        Ok(status) => Some(status),
        Err(error) => {
            errors.insert("leases".into(), error.to_string());
            None
        }
    };
    let layout = agent_builds::sweep::Layout::at(home, root);
    let sweep = match read_json(&layout.last) {
        Ok(value) => value,
        Err(error) => {
            errors.insert("sweep".into(), error);
            None
        }
    };
    let sweep_log = match tail(&layout.log) {
        Ok(text) => text,
        Err(error) => {
            errors.insert("sweep_log".into(), error);
            String::new()
        }
    };
    let sweep_last_run = sweep
        .as_ref()
        .and_then(|v| v["at"].as_str())
        .map(str::to_owned)
        .or_else(|| {
            sweep_log.lines().rev().find_map(|line| {
                line.split_once("  sweep  ")
                    .or_else(|| line.split_once("  dry run  "))
                    .map(|(at, _)| at.to_owned())
            })
        });
    let sweep_recent_evictions = sweep_log
        .lines()
        .filter(|line| line.contains("  removed  "))
        .rev()
        .take(20)
        .map(str::to_owned)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let granted_scopes = match machine::Grants::load(root) {
        Ok(grants) if grants.schema == machine::GRANTS_SCHEMA => grants.scopes_of(agent, now_ms),
        Ok(_) => {
            errors.insert("grants".into(), "unknown machine grants format".into());
            Vec::new()
        }
        Err(error) => {
            errors.insert("grants".into(), error);
            Vec::new()
        }
    };
    let recent_machine_actions = match tail(&root.join(machine::LOG_FILE)) {
        Ok(text) => {
            let mut rows: Vec<machine::Entry> = text
                .lines()
                .filter_map(|line| serde_json::from_str::<machine::Entry>(line).ok())
                .filter(|entry| entry.agent == agent)
                .collect();
            rows.drain(..rows.len().saturating_sub(20));
            rows
        }
        Err(error) => {
            errors.insert("actions".into(), error);
            Vec::new()
        }
    };
    let build_leases = status.as_ref().map(|s| (s.running, s.limit));
    let limit = agent_builds::limit(root);
    let builds_over_cap = if cargo_jobs.is_some_and(|jobs| jobs > limit)
        || build_leases.is_some_and(|(held, _)| held > limit)
    {
        Some(true)
    } else if cargo_jobs.is_some() && build_leases.is_some() {
        Some(false)
    } else {
        None
    };
    MachineFacts {
        agent: agent.into(),
        computer: computer.into(),
        observed_at_ms: now_ms,
        disk_free_gib: free.map(|bytes| bytes as f64 / (1_u64 << 30) as f64),
        disk_floor_gib: DISK_FLOOR_GIB,
        disk_below_floor: free.map(|bytes| bytes < DISK_FLOOR_GIB << 30),
        build_leases,
        build_waiting: status.map(|s| s.waiting),
        cargo_jobs,
        builds_over_cap,
        sweep_last_run,
        sweep,
        sweep_recent_evictions,
        granted_scopes,
        recent_machine_actions,
        errors,
    }
}

/// The operating system's computer name, without starting a process.
pub fn computer_name() -> Result<String, String> {
    let mut bytes = [0u8; 256];
    // SAFETY: the buffer is writable for the supplied length.
    if unsafe { libc::gethostname(bytes.as_mut_ptr().cast(), bytes.len()) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    Ok(String::from_utf8_lossy(&bytes[..end]).into_owned())
}

/// Parse only the PID list from `pgrep -x cargo`; wrappers never count.
pub fn cargo_count(text: &str) -> Result<u64, String> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .try_fold(0, |n, line| {
            line.trim()
                .parse::<u32>()
                .ok()
                .filter(|pid| *pid > 0)
                .map(|_| n + 1)
                .ok_or_else(|| "couldn't read the cargo process list".into())
        })
}

fn read_json(path: &Path) -> Result<Option<Value>, String> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Read a bounded suffix of a log, dropping a partial first line.
fn tail(path: &Path) -> Result<String, String> {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(e) => return Err(e.to_string()),
    };
    let len = file.metadata().map_err(|e| e.to_string())?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    file.seek(SeekFrom::Start(start))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(TAIL_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if start > 0 {
        let end = bytes
            .iter()
            .position(|&b| b == b'\n')
            .map_or(bytes.len(), |n| n + 1);
        bytes.drain(..end);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceAlert {
    DiskLow,
    BuildsOverCap,
}

/// Keep one instance in the host. Unknown readings preserve the last state.
#[derive(Default)]
pub struct Alerts(BTreeMap<String, (bool, bool)>);

impl Alerts {
    pub fn observe(&mut self, facts: &MachineFacts) -> Vec<ResourceAlert> {
        let state = self.0.entry(facts.agent.clone()).or_default();
        let mut out = Vec::new();
        for (reading, was, event) in [
            (facts.disk_below_floor, &mut state.0, ResourceAlert::DiskLow),
            (
                facts.builds_over_cap,
                &mut state.1,
                ResourceAlert::BuildsOverCap,
            ),
        ] {
            if let Some(low) = reading {
                if low && !*was {
                    out.push(event);
                }
                *was = low;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests;
