//! Account and character leases share the realm host's private lock directory.
use crate::error::{Error, Result};
use crate::world::World;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::Duration;

/// The kernel releases a lease when its owning file or SSH stream closes.
pub enum Lease {
    Local(File),
    Remote {
        child: Child,
        input: Option<ChildStdin>,
    },
}
impl Lease {
    /// Returns `None` when another episode holds the named lease.
    pub fn try_acquire(key: &str, host: &str) -> Result<Option<Self>> {
        let local = if std::env::var("VOYAGER_WOW_LEASE_LOCAL").as_deref() == Ok("1") {
            if cfg!(test) {
                return Err(Error::episode(
                    "tests must pass an explicit temporary lease root",
                ));
            }
            Some(
                std::env::var_os("VOYAGER_WOW_LEASE_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                            .join("wow-gym/leases")
                    }),
            )
        } else {
            None
        };
        Self::at(key, host, local.as_deref())
    }
    fn at(key: &str, host: &str, local: Option<&Path>) -> Result<Option<Self>> {
        if key.is_empty()
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || host.is_empty()
            || host.starts_with('-')
        {
            return Err(Error::episode("invalid lease key or coordinator"));
        }
        if let Some(root) = local {
            std::fs::create_dir_all(root)?;
            let mut options = std::fs::OpenOptions::new();
            options.create(true).read(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options
                    .mode(0o600)
                    .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
            }
            let file = options.open(root.join(format!("{key}.lock")))?;
            #[cfg(unix)]
            {
                use std::os::fd::AsRawFd;
                // The descriptor stays owned by the guard, and flock borrows it.
                let result =
                    unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
                if result != 0 {
                    let e = std::io::Error::last_os_error();
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        return Ok(None);
                    }
                    return Err(e.into());
                }
                return Ok(Some(Self::Local(file)));
            }
            #[cfg(not(unix))]
            return Err(Error::episode("local realm leases require Unix"));
        }
        let script = format!(
            "umask 077; mkdir -p \"$HOME/wow-gym/leases\"; exec flock -n \"$HOME/wow-gym/leases/{key}.lock\" sh -c 'echo acquired; cat >/dev/null'"
        );
        let mut child = Command::new("ssh")
            .args([
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=10",
                "-o",
                "ServerAliveInterval=5",
                "-o",
                "ServerAliveCountMax=2",
                host,
                &script,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| Error::episode(format!("lease coordinator did not start: {e}")))?;
        let input = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let _ = BufReader::new(stdout).read_line(&mut line);
            let _ = tx.send(line);
        });
        match rx.recv_timeout(Duration::from_secs(15)) {
            Ok(line) if line == "acquired\n" => Ok(Some(Self::Remote { child, input })),
            Ok(_) => {
                let status = child.wait()?;
                if status.code() == Some(1) {
                    Ok(None)
                } else {
                    Err(Error::episode("realm lease coordinator is unavailable"))
                }
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(Error::episode("realm lease coordinator timed out"))
            }
        }
    }
    /// A lost coordinator connection revokes execution authority.
    pub fn alive(&mut self) -> bool {
        match self {
            Self::Local(_) => true,
            Self::Remote { child, .. } => child.try_wait().ok() == Some(None),
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Self::Local(file) = self {
            use std::os::fd::AsRawFd;
            // Explicit unlock also releases a descriptor briefly inherited by a fork.
            unsafe {
                libc::flock(file.as_raw_fd(), libc::LOCK_UN);
            }
        }
        if let Self::Remote { child, input } = self {
            input.take();
            for _ in 0..30 {
                if child.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// An episode holds both guards until helper cleanup and shutdown finish.
pub struct AccountLease {
    pub account: String,
    pub character: String,
    capacity_guard: Lease,
    account_guard: Lease,
    character_guard: Lease,
}
impl AccountLease {
    /// Chooses the first free account from the declared pool.
    pub fn acquire(world: &World) -> Result<Self> {
        let wow = world
            .wow
            .as_ref()
            .ok_or_else(|| Error::episode("WoW lease requires a WoW world"))?;
        let mut capacity = None;
        for slot in 1..=2 {
            if let Some(guard) = Lease::try_acquire(&format!("capacity-{slot}"), &wow.lease_host)? {
                capacity = Some(guard);
                break;
            }
        }
        let capacity_guard = capacity.ok_or_else(|| {
            Error::episode("the shared realm reached its measured two-episode capacity")
        })?;
        for account in &wow.accounts {
            let upper = account.to_uppercase();
            let id = upper
                .strip_prefix("GYM")
                .and_then(|s| s.parse::<usize>().ok())
                .filter(|n| (1..=20).contains(n))
                .ok_or_else(|| Error::episode("pool accounts must be GYM1 through GYM20"))?;
            if upper != format!("GYM{id}") {
                return Err(Error::episode(
                    "pool account names must be canonical GYM1 through GYM20",
                ));
            }
            let Some(account_guard) =
                Lease::try_acquire(&format!("account-{upper}"), &wow.lease_host)?
            else {
                continue;
            };
            let character = format!(
                "{}{}{}",
                wow.character.chars().take(10).collect::<String>(),
                (b'a' + ((id - 1) / 26) as u8) as char,
                (b'a' + ((id - 1) % 26) as u8) as char
            );
            let Some(character_guard) = Lease::try_acquire(
                &format!("character-{}", character.to_lowercase()),
                &wow.lease_host,
            )?
            else {
                continue;
            };
            return Ok(Self {
                account: account.clone(),
                character,
                capacity_guard,
                account_guard,
                character_guard,
            });
        }
        Err(Error::episode(
            "all declared WoW accounts or character names are leased",
        ))
    }
    pub fn alive(&mut self) -> bool {
        self.capacity_guard.alive() && self.account_guard.alive() && self.character_guard.alive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exclusive_leases_recover_after_owner_drop() {
        let root = tempfile::tempdir().unwrap();
        let guard = Lease::at("account-GYM1", "local", Some(root.path()))
            .unwrap()
            .unwrap();
        assert!(
            Lease::at("account-GYM1", "local", Some(root.path()))
                .unwrap()
                .is_none()
        );
        assert!(
            Lease::at("account-GYM2", "local", Some(root.path()))
                .unwrap()
                .is_some()
        );
        drop(guard);
        assert!(
            Lease::at("account-GYM1", "local", Some(root.path()))
                .unwrap()
                .is_some()
        );
    }
    #[test]
    fn lease_keys_cannot_escape_the_private_directory() {
        let root = tempfile::tempdir().unwrap();
        assert!(Lease::at("../GYM1", "local", Some(root.path())).is_err());
        assert!(Lease::at("GYM1;touch-x", "local", Some(root.path())).is_err());
    }
}

/// Runs a bounded number of episodes, with at most two helpers active.
pub fn run(
    world: &World,
    plan: &crate::episode::Plan,
    episodes: usize,
    parallel: usize,
    progress: impl Fn(&str) + Sync,
) -> Result<serde_json::Value> {
    let wow = world
        .wow
        .as_ref()
        .ok_or_else(|| Error::episode("parallel episodes require a WoW world"))?;
    if episodes == 0
        || episodes > 100
        || parallel == 0
        || parallel > episodes
        || parallel > wow.max_parallel
        || parallel > wow.accounts.len()
    {
        return Err(Error::episode(
            "invalid episode count or unmeasured concurrency; initial cap is two",
        ));
    }
    let root = plan.runs.join(format!(
        "pool-{}-{}",
        std::process::id(),
        atif::document::now_ms()
    ));
    std::fs::create_dir_all(&root)?;
    let mut plan = plan.clone();
    plan.runs = root.clone();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    let started = std::time::Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..parallel {
            let next = &next;
            let results = &results;
            let plan = &plan;
            let progress = &progress;
            scope.spawn(move || {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= episodes {
                        break;
                    }
                    let result = crate::episode::run(world, plan, |m| {
                        progress(&format!("episode {}: {m}", i + 1))
                    });
                    results.lock().unwrap().push((i, result));
                }
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|(i, _)| *i);
    let mut quests = 0u64;
    let mut deaths = 0u64;
    let mut failed = 0usize;
    let mut cost_known = true;
    let mut rows = Vec::new();
    for (i, result) in results {
        match result {
            Ok(report) => {
                if !report.all_ok() {
                    failed += 1;
                }
                let metrics: serde_json::Value = serde_json::from_slice(&std::fs::read(
                    report.run_dir.join("wow-metrics.json"),
                )?)?;
                quests += metrics["quests_completed"].as_u64().unwrap_or(0);
                deaths += metrics["deaths"].as_u64().unwrap_or(0);
                cost_known &= metrics["cost_per_completed_quest"] == 0;
                rows.push(serde_json::json!({"episode":i+1,"passed":report.all_ok(),"trace":report.trace,"metrics":metrics}));
            }
            Err(e) => {
                failed += 1;
                cost_known = false;
                rows.push(serde_json::json!({"episode":i+1,"passed":false,"error":e.to_string()}));
            }
        }
    }
    let seconds = started.elapsed().as_secs_f64();
    let metrics = serde_json::json!({"schema":"voyager.wow.pool/v1","episodes":episodes,"parallel":parallel,"failed":failed,
        "seconds":seconds,"quests_completed":quests,"quests_per_hour":quests as f64*3600.0/seconds.max(0.001),"deaths":deaths,
        "model_cost_per_completed_quest":if quests>0 && cost_known {serde_json::json!(0)}else{serde_json::Value::Null},
        "cost_scope":"model calls only; infrastructure costs excluded","runs":rows,"run_dir":root});
    std::fs::write(
        root.join("pool-metrics.json"),
        serde_json::to_vec_pretty(&metrics)?,
    )?;
    Ok(metrics)
}
