//! An in-process driver for tests: each computer is a directory, each
//! command a real `sh -c` with that directory as `HOME` and an emptied
//! environment. Snapshots and forks copy the directory, so file
//! persistence is real, not modelled.
//!
//! It declares the profiles it is built with ([`FakeDriver::with_profiles`])
//! and refuses the rest exactly as a real driver must. Faults can be
//! injected per operation to exercise unknown outcomes and retries.

use crate::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// A fault for the next call of one operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inject {
    /// Perform the effect, then report `Unknown` (a lost reply).
    LoseReply,
    /// Do nothing and report `Unknown` (a lost request).
    LoseRequest,
    /// Do nothing and report `Failed`.
    Fail,
}

#[derive(Clone, Debug)]
struct Machine {
    state: State,
    /// Observations left before a starting machine runs.
    boot_left: u32,
    dir: PathBuf,
    started_unix: u64,
    run_seconds: u64,
}

#[derive(Default)]
struct World {
    machines: BTreeMap<String, Machine>,
    operations: BTreeMap<String, String>,
    snapshots: BTreeMap<String, PathBuf>,
    faults: BTreeMap<&'static str, Vec<Inject>>,
    calls: Vec<String>,
    next: u64,
}

#[derive(Clone)]
pub struct FakeDriver {
    root: PathBuf,
    profiles: BTreeSet<Profile>,
    boot_observations: u32,
    world: Arc<Mutex<World>>,
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn safe_path(dir: &Path, path: &str) -> Option<PathBuf> {
    let rel = path.trim_start_matches('/');
    let ok = !rel.is_empty()
        && rel
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..");
    ok.then(|| dir.join(rel))
}

impl FakeDriver {
    /// All four profiles, computers under `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::with_profiles(root, Profile::ALL)
    }
    pub fn with_profiles(
        root: impl Into<PathBuf>,
        profiles: impl IntoIterator<Item = Profile>,
    ) -> Self {
        let mut profiles: BTreeSet<Profile> = profiles.into_iter().collect();
        profiles.insert(Profile::Core);
        Self {
            root: root.into(),
            profiles,
            boot_observations: 1,
            world: Arc::new(Mutex::new(World::default())),
        }
    }
    /// How many observations a new or resumed computer reports Starting.
    pub fn boot_observations(mut self, n: u32) -> Self {
        self.boot_observations = n;
        self
    }
    pub fn inject(&self, op: &'static str, how: Inject) {
        self.w().faults.entry(op).or_default().push(how);
    }
    /// Every call, in order (`create:<operation>`, `exec:<computer>`, …).
    pub fn calls(&self) -> Vec<String> {
        self.w().calls.clone()
    }
    /// Computers that exist (not deleted), with their state.
    pub fn live(&self) -> Vec<(String, State)> {
        self.w()
            .machines
            .iter()
            .filter(|(_, m)| m.state != State::Gone)
            .map(|(id, m)| (id.clone(), m.state))
            .collect()
    }
    fn w(&self) -> std::sync::MutexGuard<'_, World> {
        self.world.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn fault(&self, op: &'static str, target: &str) -> Option<Inject> {
        let mut w = self.w();
        w.calls.push(format!("{op}:{target}"));
        let list = w.faults.get_mut(op)?;
        (!list.is_empty()).then(|| list.remove(0))
    }
    fn declared(&self, p: Profile) -> bool {
        self.profiles.contains(&p)
    }
    fn new_machine(&self, w: &mut World, from: Option<&Path>) -> std::io::Result<String> {
        w.next += 1;
        let id = format!("fk_{}_{}", std::process::id(), w.next);
        let dir = self.root.join(&id);
        match from {
            Some(src) => copy_dir(src, &dir)?,
            None => std::fs::create_dir_all(&dir)?,
        }
        w.machines.insert(
            id.clone(),
            Machine {
                state: State::Starting,
                boot_left: self.boot_observations,
                dir,
                started_unix: now_unix(),
                run_seconds: 0,
            },
        );
        Ok(id)
    }
    fn running_dir(&self, computer: &str) -> Result<PathBuf, Outcome<()>> {
        let w = self.w();
        match w.machines.get(computer) {
            Some(m) if m.state == State::Running => Ok(m.dir.clone()),
            Some(m) if m.state == State::Gone => Err(Outcome::failed("the computer was deleted")),
            Some(_) => Err(Outcome::failed("the computer is not running")),
            None => Err(Outcome::failed("no such computer")),
        }
    }
}

fn wrap<T>(fault: Option<Inject>, effect: impl FnOnce() -> Outcome<T>) -> Outcome<T> {
    match fault {
        Some(Inject::Fail) => Outcome::failed("injected failure"),
        Some(Inject::LoseRequest) => Outcome::unknown("injected lost request"),
        Some(Inject::LoseReply) => {
            let _ = effect();
            Outcome::unknown("injected lost reply")
        }
        None => effect(),
    }
}

impl Driver for FakeDriver {
    fn capabilities(&self) -> Capabilities {
        Capabilities::new("fake", self.profiles.iter().copied(), &["small", "default"])
    }

    async fn create(&self, r: &CreateRequest) -> Outcome<String> {
        let fault = self.fault("create", &r.operation);
        wrap(fault, || {
            if r.operation.is_empty() {
                return Outcome::failed("a create needs an operation identity");
            }
            if let Some(size) = &r.size
                && !["small", "default"].contains(&size.as_str())
            {
                return Outcome::failed("unknown size");
            }
            let mut w = self.w();
            if let Some(id) = w.operations.get(&r.operation) {
                return Outcome::done(id.clone());
            }
            let from = match &r.image {
                Some(name) => match w.snapshots.get(name) {
                    Some(dir) => Some(dir.clone()),
                    None => return Outcome::failed("no such image"),
                },
                None => None,
            };
            match self.new_machine(&mut w, from.as_deref()) {
                Ok(id) => {
                    w.operations.insert(r.operation.clone(), id.clone());
                    Outcome::done(id)
                }
                Err(e) => Outcome::failed(format!("create: {e}")),
            }
        })
    }

    async fn observe(&self, computer: &str) -> Outcome<Observation> {
        let fault = self.fault("observe", computer);
        wrap(fault, || {
            let mut w = self.w();
            let Some(m) = w.machines.get_mut(computer) else {
                return Outcome::done(Observation {
                    state: State::Gone,
                    detail: None,
                });
            };
            if m.state == State::Starting {
                if m.boot_left == 0 {
                    m.state = State::Running;
                } else {
                    m.boot_left -= 1;
                }
            }
            Outcome::done(Observation {
                state: m.state,
                detail: None,
            })
        })
    }

    async fn exec(&self, computer: &str, e: &Exec) -> Outcome<ExecOutput> {
        let fault = self.fault("exec", computer);
        if matches!(fault, Some(Inject::Fail)) {
            return Outcome::failed("injected failure");
        }
        if matches!(fault, Some(Inject::LoseRequest)) {
            return Outcome::unknown("injected lost request");
        }
        let dir = match self.running_dir(computer) {
            Ok(d) => d,
            Err(o) => return o.map(|()| unreachable!()),
        };
        if e.secret_env.keys().any(|k| !env_name_ok(k)) {
            return Outcome::failed("invalid environment variable name");
        }
        let cwd = match e.cwd.as_deref() {
            None | Some("") | Some(".") => dir.clone(),
            Some(c) if c.starts_with('/') => PathBuf::from(c),
            Some(c) => match safe_path(&dir, c) {
                Some(p) => p,
                None => return Outcome::failed("invalid working directory"),
            },
        };
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c")
            .arg(&e.command)
            .env_clear()
            .env("HOME", &dir)
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .current_dir(cwd)
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true);
        for (k, v) in &e.secret_env {
            cmd.env(k, v.expose());
        }
        let limit = std::time::Duration::from_secs(e.timeout_seconds.clamp(1, 3600));
        let out = match tokio::time::timeout(limit, cmd.output()).await {
            Ok(Ok(o)) => o,
            Ok(Err(err)) => return Outcome::failed(format!("exec: {err}")),
            Err(_) => {
                return Outcome::done(ExecOutput {
                    code: 124,
                    stdout: String::new(),
                    stderr: "timed out".into(),
                });
            }
        };
        let result = ExecOutput {
            code: i64::from(out.status.code().unwrap_or(-1)),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        };
        if matches!(fault, Some(Inject::LoseReply)) {
            return Outcome::unknown("injected lost reply");
        }
        Outcome::done(result)
    }

    async fn put_file(&self, computer: &str, path: &str, bytes: &[u8]) -> Outcome<()> {
        let fault = self.fault("put_file", computer);
        wrap(fault, || {
            let dir = match self.running_dir(computer) {
                Ok(d) => d,
                Err(o) => return o,
            };
            let Some(target) = safe_path(&dir, path) else {
                return Outcome::failed("invalid path");
            };
            if let Some(parent) = target.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::write(target, bytes) {
                Ok(()) => Outcome::done(()),
                Err(e) => Outcome::failed(format!("write: {e}")),
            }
        })
    }

    async fn get_file(&self, computer: &str, path: &str) -> Outcome<Vec<u8>> {
        let fault = self.fault("get_file", computer);
        wrap(fault, || {
            let dir = match self.running_dir(computer) {
                Ok(d) => d,
                Err(o) => return o.map(|()| Vec::new()),
            };
            let Some(target) = safe_path(&dir, path) else {
                return Outcome::failed("invalid path");
            };
            match std::fs::read(target) {
                Ok(b) => Outcome::done(b),
                Err(e) => Outcome::failed(format!("read: {e}")),
            }
        })
    }

    async fn meter(&self, computer: &str) -> Outcome<Meter> {
        let fault = self.fault("meter", computer);
        wrap(fault, || {
            let w = self.w();
            let Some(m) = w.machines.get(computer) else {
                return Outcome::failed("no such computer");
            };
            let running = matches!(m.state, State::Running | State::Starting);
            let seconds = m.run_seconds
                + if running {
                    now_unix().saturating_sub(m.started_unix)
                } else {
                    0
                };
            Outcome::done(Meter {
                running,
                evidence: format!("fake:{computer}"),
                seconds: Some(seconds),
            })
        })
    }

    async fn delete(&self, computer: &str) -> Outcome<()> {
        let fault = self.fault("delete", computer);
        wrap(fault, || {
            let mut w = self.w();
            if let Some(m) = w.machines.get_mut(computer) {
                if m.state != State::Gone {
                    let _ = std::fs::remove_dir_all(&m.dir);
                    if matches!(m.state, State::Running | State::Starting) {
                        m.run_seconds += now_unix().saturating_sub(m.started_unix);
                    }
                    m.state = State::Gone;
                }
            }
            Outcome::done(())
        })
    }

    async fn suspend(&self, computer: &str) -> Outcome<String> {
        if !self.declared(Profile::Suspend) {
            return unsupported(Profile::Suspend);
        }
        let fault = self.fault("suspend", computer);
        wrap(fault, || {
            let mut w = self.w();
            match w.machines.get_mut(computer) {
                Some(m) if matches!(m.state, State::Running | State::Starting) => {
                    m.run_seconds += now_unix().saturating_sub(m.started_unix);
                    m.state = State::Stopped;
                    Outcome::done(format!("stop:{computer}"))
                }
                Some(m) if m.state == State::Stopped => Outcome::done(format!("stop:{computer}")),
                _ => Outcome::failed("the computer cannot be stopped"),
            }
        })
    }

    async fn resume(&self, computer: &str) -> Outcome<String> {
        if !self.declared(Profile::Suspend) {
            return unsupported(Profile::Suspend);
        }
        let fault = self.fault("resume", computer);
        let boot = self.boot_observations;
        wrap(fault, || {
            let mut w = self.w();
            match w.machines.get_mut(computer) {
                Some(m) if m.state == State::Stopped => {
                    m.state = State::Starting;
                    m.boot_left = boot;
                    m.started_unix = now_unix();
                    Outcome::done(format!("resume:{computer}"))
                }
                Some(m) if matches!(m.state, State::Running | State::Starting) => {
                    Outcome::done(format!("resume:{computer}"))
                }
                _ => Outcome::failed("the computer cannot be resumed"),
            }
        })
    }

    async fn snapshot(&self, computer: &str, name: &str) -> Outcome<String> {
        if !self.declared(Profile::Snapshot) {
            return unsupported(Profile::Snapshot);
        }
        let fault = self.fault("snapshot", computer);
        wrap(fault, || {
            let mut w = self.w();
            let Some(m) = w.machines.get(computer).cloned() else {
                return Outcome::failed("no such computer");
            };
            if m.state == State::Gone {
                return Outcome::failed("the computer was deleted");
            }
            if w.snapshots.contains_key(name) {
                return Outcome::failed("that snapshot name is taken");
            }
            let dir = self.root.join(format!("snap-{name}"));
            if let Err(e) = copy_dir(&m.dir, &dir) {
                return Outcome::failed(format!("snapshot: {e}"));
            }
            w.snapshots.insert(name.into(), dir);
            Outcome::done(name.into())
        })
    }

    async fn snapshot_ready(&self, name: &str) -> Outcome<bool> {
        if !self.declared(Profile::Snapshot) {
            return unsupported(Profile::Snapshot);
        }
        Outcome::done(self.w().snapshots.contains_key(name))
    }

    async fn delete_snapshot(&self, name: &str) -> Outcome<()> {
        if !self.declared(Profile::Snapshot) {
            return unsupported(Profile::Snapshot);
        }
        if let Some(dir) = self.w().snapshots.remove(name) {
            let _ = std::fs::remove_dir_all(dir);
        }
        Outcome::done(())
    }

    async fn fork(&self, computer: &str, operation: &str) -> Outcome<String> {
        if !self.declared(Profile::Fork) {
            return unsupported(Profile::Fork);
        }
        let fault = self.fault("fork", computer);
        wrap(fault, || {
            let mut w = self.w();
            if let Some(id) = w.operations.get(operation) {
                return Outcome::done(id.clone());
            }
            let Some(m) = w.machines.get(computer).cloned() else {
                return Outcome::failed("no such computer");
            };
            if m.state == State::Gone {
                return Outcome::failed("the computer was deleted");
            }
            match self.new_machine(&mut w, Some(&m.dir)) {
                Ok(id) => {
                    w.operations.insert(operation.into(), id.clone());
                    Outcome::done(id)
                }
                Err(e) => Outcome::failed(format!("fork: {e}")),
            }
        })
    }
}
