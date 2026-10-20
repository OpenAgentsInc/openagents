//! Serial text execution with recovery owned by the existing engine supervisor.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::{oneshot, watch};

use crate::engine::{Engine, Pending, Sampling, Turn};
use crate::online::{LaneSpec, Options};

/// A pinned text profile selected by `pylon up`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile {
    #[serde(rename = "deepseek-r1-distill-qwen-1.5b")]
    DeepSeekR1DistillQwen15B,
}

impl Profile {
    pub const MODEL: &str = "deepseek-r1-distill-qwen-1.5b";
    pub const FILE: &str = "DeepSeek-R1-Distill-Qwen-1.5B-Q4_K_M.gguf";
    pub const BYTES: u64 = 1_117_321_312;
    pub const SHA256: &str = "f3bdf9cf31dee4b57ae4e455a1cb0d01b5c2c1b50d72d3112141c195506c2840";

    /// Verify the complete file before a profile starts a worker.
    pub fn verify(self, path: &std::path::Path) -> Result<(), String> {
        use sha2::{Digest, Sha256};
        use std::io::Read;
        let mut file = std::fs::File::open(path)
            .map_err(|e| format!("cannot open the DeepSeek model at {}: {e}", path.display()))?;
        if file.metadata().map_err(|e| e.to_string())?.len() != Self::BYTES {
            return Err(
                "the DeepSeek profile needs the pinned Q4_K_M model file; its size does not match"
                    .into(),
            );
        }
        let mut hash = Sha256::new();
        let mut buffer = vec![0; 1024 * 1024];
        loop {
            let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        if format!("{:x}", hash.finalize()) != Self::SHA256 {
            return Err("the DeepSeek model failed its SHA-256 check; replace it with the pinned Q4_K_M file".into());
        }
        Ok(())
    }
}

/// Locate llama.cpp without requiring a native Psionic installation.
pub(crate) fn find_server() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("OPENAGENTS_LLAMA_CPP_BIN") {
        return Some(PathBuf::from(path));
    }
    crate::setup::find_server()
        .map(|path| path.with_file_name("llama-server"))
        .filter(|path| path.is_file())
        .or_else(|| {
            std::env::var_os("PATH").and_then(|paths| {
                std::env::split_paths(&paths)
                    .map(|dir| dir.join("llama-server"))
                    .find(|path| path.is_file())
            })
        })
}

/// An explicitly owned llama.cpp worker and its bounded recovery policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    pub server: PathBuf,
    pub generation_seconds: u64,
    pub startup_seconds: u64,
    pub stop_seconds: u64,
    pub max_restarts: u32,
    pub sampling: Sampling,
}

impl Config {
    /// Refuse invalid limits before starting any model process.
    pub fn validate(&self) -> Result<(), String> {
        if !cfg!(unix) {
            return Err("managed serial workers require a Unix host".into());
        }
        if !(1..=75).contains(&self.generation_seconds)
            || !(1..=900).contains(&self.startup_seconds)
            || !(1..=5).contains(&self.stop_seconds)
            || !(1..=10).contains(&self.max_restarts)
        {
            return Err("use generation limits of 1–75 seconds, startup limits of 1–900 seconds, stop limits of 1–5 seconds, and 1–10 restarts".into());
        }
        if !self.server.is_file() {
            return Err(format!("llama.cpp is missing at {}", self.server.display()));
        }
        Ok(())
    }
}

#[derive(Default)]
struct State {
    ready: bool,
    failed: bool,
    reason: String,
    active: Option<(u64, Instant)>,
    next: u64,
    completed: u64,
}

/// Shared execution state. HTTP callers never own the running generation.
pub(crate) struct Control {
    state: Mutex<State>,
    changed: watch::Sender<u64>,
}

impl Control {
    pub(crate) fn new() -> Arc<Self> {
        let (changed, _) = watch::channel(0);
        Arc::new(Self {
            state: Mutex::new(State::default()),
            changed,
        })
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn notify(&self) {
        self.changed.send_modify(|n| *n = n.wrapping_add(1));
    }

    fn block(&self, reason: &str, failed: bool) {
        {
            let mut s = self.state();
            s.ready = false;
            s.failed = failed;
            s.reason = reason.into();
        }
        self.notify();
    }

    fn ready(&self) -> bool {
        self.state().ready
    }
    pub(crate) fn is_ready(&self) -> bool {
        self.ready()
    }
    pub(crate) fn busy(&self) -> bool {
        !self.idle()
    }
    fn idle(&self) -> bool {
        self.state().active.is_none()
    }
    fn completed(&self) -> u64 {
        self.state().completed
    }

    pub(crate) fn failure(&self) -> Option<String> {
        let s = self.state();
        s.failed.then(|| s.reason.clone())
    }

    fn admit(&self) -> Result<u64, String> {
        let mut s = self.state();
        if !s.ready {
            return Err("the model is restarting or stopped".into());
        }
        if s.active.is_some() {
            return Err("the model is busy".into());
        }
        s.next = s.next.wrapping_add(1);
        let id = s.next;
        s.active = Some((id, Instant::now()));
        drop(s);
        self.notify();
        Ok(id)
    }

    fn finish(&self, id: u64, completed: bool) {
        let mut s = self.state();
        if s.active.is_some_and(|(active, _)| active == id) {
            s.active = None;
            s.completed += u64::from(completed);
        }
        drop(s);
        self.notify();
    }

    fn expired(&self, limit: Duration) -> bool {
        self.state()
            .active
            .is_some_and(|(_, since)| since.elapsed() >= limit)
    }

    async fn invalidated(&self, id: u64) -> String {
        let mut changed = self.changed.subscribe();
        loop {
            changed.borrow_and_update();
            {
                let s = self.state();
                if !s.ready || s.active.is_none_or(|(active, _)| active != id) {
                    return s.reason.clone();
                }
            }
            if changed.changed().await.is_err() {
                return "the worker stopped".into();
            }
        }
    }
}

/// One controller per configured home, held across every worker replacement.
#[cfg(unix)]
pub(crate) fn ownership(home: &std::path::Path) -> Result<std::fs::File, String> {
    use std::os::fd::AsRawFd;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(home.join("serial-worker.lock"))
        .map_err(|e| e.to_string())?;
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err("another supervisor owns this serial worker".into());
    }
    Ok(file)
}

#[cfg(not(unix))]
pub(crate) fn ownership(_: &std::path::Path) -> Result<std::fs::File, String> {
    Err("managed serial workers require a Unix host".into())
}

struct GenerationGuard {
    control: Arc<Control>,
    id: u64,
    finished: bool,
}

impl Drop for GenerationGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.control.block("model request interrupted", false);
        }
        self.control.finish(self.id, false);
    }
}

/// The serial engine used by a managed text worker.
pub(crate) struct SerialEngine {
    inner: Arc<dyn Engine>,
    control: Arc<Control>,
}

impl SerialEngine {
    pub(crate) fn wrap(inner: Arc<dyn Engine>, control: Arc<Control>) -> Arc<dyn Engine> {
        Arc::new(Self { inner, control })
    }
}

impl Engine for SerialEngine {
    fn model(&self) -> &str {
        self.inner.model()
    }
    fn healthy(&self) -> Pending<'_, bool> {
        Box::pin(async { self.control.ready() })
    }
    fn generate<'a>(
        &'a self,
        turns: &'a [Turn],
        max_tokens: u32,
    ) -> Pending<'a, Result<crate::engine::Generation, String>> {
        Box::pin(async move {
            let id = self.control.admit()?;
            let control = Arc::clone(&self.control);
            let inner = Arc::clone(&self.inner);
            let turns = turns.to_vec();
            let (reply, response) = oneshot::channel();
            // Dropping the buyer/provider future leaves this task and its slot
            // alive until completion or the supervisor invalidates the worker.
            tokio::spawn(async move {
                let mut guard = GenerationGuard {
                    control: Arc::clone(&control),
                    id,
                    finished: false,
                };
                let result = tokio::select! {
                    biased;
                    reason = control.invalidated(id) => Err(format!("model worker stopped: {reason}")),
                    result = inner.generate(&turns, max_tokens) => result,
                };
                if result.is_err() && inner.recovery_required() {
                    control.block("model connection failed before completion", false);
                }
                control.finish(id, result.is_ok());
                guard.finished = true;
                let _ = reply.send(result);
            });
            response
                .await
                .map_err(|_| "the model request stopped".to_string())?
        })
    }
}

#[cfg(unix)]
struct OwnedChild {
    child: tokio::process::Child,
    group: i32,
}

#[cfg(unix)]
impl Drop for OwnedChild {
    fn drop(&mut self) {
        // The command creates a new process group owned by this supervisor.
        unsafe {
            libc::kill(-self.group, libc::SIGKILL);
        }
        let _ = self.child.start_kill();
    }
}

#[cfg(unix)]
impl OwnedChild {
    async fn stop(&mut self, limit: Duration) -> Result<(), String> {
        unsafe {
            libc::kill(-self.group, libc::SIGKILL);
        }
        tokio::time::timeout(limit, self.child.wait())
            .await
            .map_err(|_| "cannot confirm model process exit".to_string())?
            .map_err(|e| format!("cannot reap model process: {e}"))?;
        let until = Instant::now() + limit;
        loop {
            let status = unsafe { libc::kill(-self.group, 0) };
            if status == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                return Ok(());
            }
            if Instant::now() >= until {
                return Err("model process group has not exited; serving stays paused".into());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
}

/// Manage only the explicitly spawned child group. Never adopt a listener.
#[cfg(unix)]
pub(crate) async fn supervise(
    lane: LaneSpec,
    home: PathBuf,
    options: Arc<Options>,
    control: Arc<Control>,
    record: impl Fn(Option<u32>, bool, Option<String>, bool),
    log: Arc<dyn Fn(String) + Send + Sync>,
    mut kill: watch::Receiver<bool>,
) {
    use std::os::unix::process::CommandExt;
    let config = lane.serial.as_ref().expect("serial configuration");
    let mut failures = 0;
    let mut delay = options.backoff;
    loop {
        if *kill.borrow() {
            control.block("worker stopped", false);
            return;
        }
        control.block("model worker is starting", false);
        // A stale or unrelated listener must never be used as readiness proof.
        if let Err(error) = std::net::TcpListener::bind(("127.0.0.1", lane.port)) {
            let reason = format!("model port {} is already in use: {error}", lane.port);
            control.block(&reason, true);
            record(None, false, Some(reason), false);
            let _ = kill.changed().await;
            return;
        }
        let command = options.engines.command(&home, &lane);
        let spawned = command.and_then(|mut command| {
            command.as_std_mut().process_group(0);
            command
                .kill_on_drop(true)
                .spawn()
                .map_err(|e| format!("model worker did not start: {e}"))
        });
        let started = Instant::now();
        let completed = control.completed();
        let reason;
        match spawned {
            Ok(child) => {
                let pid = child.id().expect("new child process");
                let mut owned = OwnedChild {
                    child,
                    group: pid as i32,
                };
                record(Some(pid), false, None, false);
                log(format!(
                    "serial model started (process {pid}); waiting for readiness"
                ));
                let mut ready = false;
                let mut changes = control.changed.subscribe();
                let tick = options.every.min(Duration::from_millis(250));
                reason = loop {
                    tokio::select! {
                        biased;
                        _ = kill.changed() => {
                            control.block("worker stopped", false);
                            let stopped = owned.stop(Duration::from_secs(config.stop_seconds)).await;
                            record(stopped.is_err().then_some(pid), false, stopped.err(), false);
                            return;
                        }
                        status = owned.child.wait() => break format!("model worker exited: {status:?}"),
                        _ = changes.changed() => {}
                        _ = tokio::time::sleep(tick) => {}
                    }
                    if control.expired(Duration::from_secs(config.generation_seconds)) {
                        break "model generation exceeded its execution deadline".into();
                    }
                    if ready && !control.ready() {
                        break control.state().reason.clone();
                    }
                    if !ready && started.elapsed() >= Duration::from_secs(config.startup_seconds) {
                        break "model worker exceeded its startup deadline".into();
                    }
                    // Readiness is separate from progress: a healthy HTTP
                    // endpoint cannot reset the active generation's deadline.
                    let answers =
                        tokio::time::timeout(Duration::from_secs(1), options.engines.ready(&lane))
                            .await
                            .unwrap_or(false);
                    if answers && !ready {
                        if !control.idle() {
                            break "previous model request did not stop".into();
                        }
                        {
                            let mut s = control.state();
                            s.ready = true;
                            s.reason.clear();
                        }
                        control.notify();
                        ready = true;
                        record(Some(pid), true, None, false);
                        log(format!("serial model ready (process {pid})"));
                    } else if !answers && ready {
                        break "model worker stopped answering health checks".into();
                    }
                };
                control.block(&reason, false);
                record(Some(pid), false, Some(reason.clone()), false);
                if let Err(error) = owned.stop(Duration::from_secs(config.stop_seconds)).await {
                    control.block(&error, true);
                    record(Some(pid), false, Some(error), false);
                    let _ = kill.changed().await;
                    return;
                }
                let until = Instant::now() + Duration::from_secs(config.stop_seconds);
                while !control.idle() && Instant::now() < until {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                if !control.idle() {
                    let reason = "previous model request did not stop; serving stays paused";
                    control.block(reason, true);
                    record(None, false, Some(reason.into()), false);
                    let _ = kill.changed().await;
                    return;
                }
            }
            Err(error) => {
                reason = error;
                control.block(&reason, false);
            }
        }
        if control.completed() > completed {
            failures = 0;
            delay = options.backoff;
        }
        failures += 1;
        record(None, false, Some(reason.clone()), false);
        if failures > config.max_restarts {
            let reason = format!(
                "model stopped after repeated failures: {reason}; restart the pylon after fixing the worker"
            );
            control.block(&reason, true);
            record(None, false, Some(reason), false);
            let _ = kill.changed().await;
            return;
        }
        record(None, false, None, true);
        log(format!(
            "serial model stopped: {reason}; restarting in {}s",
            delay.as_secs_f32()
        ));
        tokio::select! {
            _ = kill.changed() => { control.block("worker stopped", false); return; }
            _ = tokio::time::sleep(delay) => {}
        }
        delay = (delay * 2).min(options.max_backoff);
    }
}

#[cfg(not(unix))]
pub(crate) async fn supervise(
    _lane: LaneSpec,
    _home: PathBuf,
    _options: Arc<Options>,
    control: Arc<Control>,
    _record: impl Fn(Option<u32>, bool, Option<String>, bool),
    _log: Arc<dyn Fn(String) + Send + Sync>,
    mut kill: watch::Receiver<bool>,
) {
    control.block("managed serial workers require a Unix host", true);
    let _ = kill.changed().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Never;
    impl Engine for Never {
        fn model(&self) -> &str {
            "never"
        }
        fn healthy(&self) -> Pending<'_, bool> {
            Box::pin(async { true })
        }
        fn generate<'a>(
            &'a self,
            _: &'a [Turn],
            _: u32,
        ) -> Pending<'a, Result<crate::engine::Generation, String>> {
            Box::pin(std::future::pending())
        }
    }

    #[tokio::test]
    async fn dropping_the_caller_keeps_the_generation_slot_until_invalidation() {
        let control = Control::new();
        control.state().ready = true;
        let engine = SerialEngine::wrap(Arc::new(Never), Arc::clone(&control));
        let caller_engine = Arc::clone(&engine);
        let caller = tokio::spawn(async move { caller_engine.generate(&[], 1).await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while control.idle() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        caller.abort();
        let _ = caller.await;
        assert!(!control.idle());
        assert!(engine.generate(&[], 1).await.unwrap_err().contains("busy"));
        control.block("test worker stopping", false);
        tokio::time::timeout(Duration::from_secs(2), async {
            while !control.idle() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert!(!engine.healthy().await);
    }

    #[cfg(unix)]
    #[test]
    fn only_one_supervisor_can_own_a_serial_home() {
        let dir = tempfile::tempdir().unwrap();
        let first = ownership(dir.path()).unwrap();
        assert!(ownership(dir.path()).is_err());
        drop(first);
        assert!(ownership(dir.path()).is_ok());
    }
}
