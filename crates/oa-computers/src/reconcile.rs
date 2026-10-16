//! The reconcile loop: read desired state, claim one computer with a row
//! lease, call the driver through the lifecycle core, persist every
//! observation, repeat, and recover after any crash.
//!
//! Each visit claims a due computer (`FOR UPDATE SKIP LOCKED`), runs one
//! settle of `coder_working_computer` (decide → one driver effect →
//! retain the observation → decide again) toward its desired state, and
//! gives the lease back with when to look again: soon after a failure
//! (with backoff), on a slow revisit otherwise (idle and absolute bounds,
//! silent turns). Every observation is retained before the next decision,
//! so a process killed mid-provision resumes from the last retained fact:
//! a create whose reply was lost repeats the same operation identity and
//! finds the same computer, never a second one.
//!
//! The loop runs on a `LocalSet` (driver futures need not be `Send`).

use crate::registry::{Drivers, Refused};
use crate::store::{Desired, Entry, PgLease, Store};
use coder_working_computer::driver::{self, Settled};
use coder_working_computer::on_driver::OnDriver;
use coder_working_computer::{Phase, StopReason, Trigger};
use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Settings {
    /// Unique per process; names the lease holder.
    pub holder: String,
    pub lease: Duration,
    /// How often an idle reconciler looks for due work.
    pub poll: Duration,
    /// When to revisit a settled live computer.
    pub revisit: Duration,
    /// First retry delay; doubles per failed attempt, capped.
    pub backoff: Duration,
    pub backoff_cap: Duration,
    pub parallel: usize,
    /// How often a running turn's owner says it is alive.
    pub heartbeat: Duration,
    /// A computer stopped this long with nothing asked of it is deleted
    /// (its disk is billed while stopped).
    pub keep_stopped: Duration,
    /// The lifecycle's wait between observations and its deadline for a
    /// boot or a stop.
    pub driver_poll: Duration,
    pub driver_deadline: Duration,
}
impl Settings {
    pub fn new(holder: impl Into<String>) -> Self {
        Self {
            holder: holder.into(),
            lease: Duration::from_secs(60),
            poll: Duration::from_secs(1),
            revisit: Duration::from_secs(30),
            backoff: Duration::from_secs(5),
            backoff_cap: Duration::from_secs(300),
            parallel: 16,
            heartbeat: Duration::from_millis(coder_working_computer::HEARTBEAT_EVERY_MS),
            keep_stopped: Duration::from_secs(7 * 24 * 3600),
            driver_poll: Duration::from_secs(3),
            driver_deadline: Duration::from_secs(900),
        }
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Phases a "running" intent wakes from (create, restore, or finish one).
fn wakes_from(p: &Phase) -> bool {
    matches!(
        p,
        Phase::New
            | Phase::Creating
            | Phase::Booting
            | Phase::Stopped
            | Phase::Unknown { .. }
            | Phase::Failed { .. }
    )
}

pub struct Reconciler<D> {
    pub store: Store,
    pub driver: D,
    pub settings: Settings,
    /// Turn credentials waiting in memory (shared with the API).
    pub turns: crate::turns::Secrets,
}

impl<D: Drivers + 'static> Reconciler<D> {
    fn provider(&self, e: &Entry) -> Result<OnDriver<D::Bound>, Refused> {
        let mut p = OnDriver::new(self.driver.resolve(e)?, e.image.clone());
        p.poll = self.settings.driver_poll;
        p.deadline = self.settings.driver_deadline;
        Ok(p)
    }

    fn backoff(&self, attempts: i32) -> Duration {
        let factor = 1u32 << attempts.clamp(0, 10);
        (self.settings.backoff * factor).min(self.settings.backoff_cap)
    }

    /// Run forever: claim due computers and visit them, a bounded number at
    /// once. Call inside a `LocalSet`.
    pub async fn run(self: Rc<Self>) {
        let busy = Rc::new(Cell::new(0usize));
        let mut pruned = tokio::time::Instant::now() - Duration::from_secs(60);
        loop {
            if pruned.elapsed() >= Duration::from_secs(60) {
                if let Some(catalog) = self.driver.legacy_catalog() {
                    // Cover rows made by a previous API revision during rollout.
                    let _ = self.store.pin_legacy(catalog).await;
                }
                let _ = self.store.prune_command_output().await;
                pruned = tokio::time::Instant::now();
            }
            let mut claimed = false;
            while busy.get() < self.settings.parallel {
                match self
                    .store
                    .claim(&self.settings.holder, self.settings.lease)
                    .await
                {
                    Ok(Some(e)) => {
                        claimed = true;
                        busy.set(busy.get() + 1);
                        let me = self.clone();
                        let b = busy.clone();
                        tokio::task::spawn_local(async move {
                            me.visit(e).await;
                            b.set(b.get() - 1);
                        });
                    }
                    Ok(None) => break,
                    Err(err) => {
                        eprintln!("oa-computers: claim: {err}");
                        break;
                    }
                }
            }
            if !claimed {
                tokio::time::sleep(self.settings.poll).await;
            } else {
                tokio::task::yield_now().await;
            }
        }
    }

    /// One visit to one claimed computer.
    pub async fn visit(self: &Rc<Self>, e: Entry) {
        let now = now_ms();
        let mut e = e;
        if e.desired == Desired::Stopped && e.record.phase == Phase::Stopped {
            match self
                .store
                .expire_stopped(
                    &e.owner,
                    &e.id,
                    e.intent_generation,
                    now,
                    self.settings.keep_stopped,
                )
                .await
            {
                Ok(true) => e.desired = Desired::Deleted,
                Ok(false) => {}
                Err(error) => {
                    let _ = self
                        .store
                        .release(
                            &e.id,
                            &self.settings.holder,
                            self.backoff(e.attempts),
                            e.attempts.saturating_add(1),
                            Some(&error.to_string()),
                            false,
                        )
                        .await;
                    return;
                }
            }
        }
        let p = match self.provider(&e) {
            Ok(provider) => provider,
            Err(error) => {
                // A missing deployment is not evidence that its resource is gone.
                let _ = self
                    .store
                    .release(
                        &e.id,
                        &self.settings.holder,
                        self.backoff(e.attempts),
                        e.attempts.saturating_add(1),
                        Some(&error.to_string()),
                        false,
                    )
                    .await;
                return;
            }
        };
        let lease = PgLease {
            store: self.store.clone(),
            id: e.id.clone(),
            holder: self.settings.holder.clone(),
            lease: self.settings.lease,
        };
        let renewer = {
            let store = self.store.clone();
            let id = e.id.clone();
            let holder = self.settings.holder.clone();
            let every = self.settings.lease / 3;
            let lease = self.settings.lease;
            tokio::task::spawn_local(async move {
                loop {
                    tokio::time::sleep(every).await;
                    if !store.renew(&id, &holder, lease).await.unwrap_or(false) {
                        break;
                    }
                }
            })
        };
        let woke = e.desired == Desired::Running && wakes_from(&e.record.phase);
        // Recover public commands before the legacy lifecycle tick can expire a
        // turn after a service restart. Recovery observes the same identity.
        let active_command = if matches!(e.record.phase, Phase::Turn { .. }) {
            self.store
                .next_turn(&e.id)
                .await
                .ok()
                .flatten()
                .filter(|t| t.api_command)
        } else {
            None
        };
        let result = if let Some(t) = active_command {
            crate::commands::run(
                &self.store,
                &self.turns,
                &p,
                &lease,
                &e,
                e.record.clone(),
                t,
                self.settings.heartbeat,
            )
            .await
        } else {
            match e.desired {
                Desired::Deleted => driver::delete(&p, &lease, now).await,
                Desired::Stopped => driver::stop(&p, &lease, now).await,
                Desired::Running if woke => driver::settle(&p, &lease, Trigger::Wake, now).await,
                Desired::Running => driver::tick(&p, &lease, false, now).await,
            }
        };
        // A waiting turn runs once the computer is awake.
        let result = match result {
            Ok(Settled::Idle(c)) if e.desired == Desired::Running && c.phase == Phase::Awake => {
                crate::turns::run_next(
                    &self.store,
                    &self.turns,
                    &p,
                    &lease,
                    &e,
                    c,
                    self.settings.heartbeat,
                )
                .await
            }
            other => other,
        };
        renewer.abort();
        let (attempts, error, again, done) = match &result {
            Ok(s) => {
                let c = s.computer();
                let done = e.desired == Desired::Deleted && c.phase == Phase::Deleted;
                if done {
                    let _ = self
                        .store
                        .end_turns(&e.id, "The computer was deleted.")
                        .await;
                }
                if e.desired == Desired::Running
                    && c.phase == Phase::Stopped
                    && !woke
                    && c.boot().and_then(|b| b.stop_reason) != Some(StopReason::Checkpoint)
                {
                    // A lifecycle bound stopped it (idle, absolute, silent
                    // turn): the intent follows, so it is not woken again.
                    let _ = self
                        .store
                        .settle_desired(
                            &e.id,
                            e.intent_generation,
                            Desired::Running,
                            Desired::Stopped,
                        )
                        .await;
                }
                match s {
                    // Deleted and settled: nothing went wrong.
                    _ if done => (0, None, self.settings.revisit, true),
                    Settled::Stuck(d, _) => (
                        e.attempts + 1,
                        Some(format!("no progress on {d:?}")),
                        self.backoff(e.attempts),
                        false,
                    ),
                    Settled::BootFailed(c) => (
                        e.attempts + 1,
                        Some(match &c.phase {
                            Phase::Failed { reason } => format!("boot failed: {reason}"),
                            _ => "boot failed".into(),
                        }),
                        self.backoff(e.attempts),
                        false,
                    ),
                    Settled::Refused(m, _) => (
                        e.attempts + 1,
                        Some(m.clone()),
                        self.backoff(e.attempts),
                        done,
                    ),
                    _ => (0, None, self.settings.revisit, done),
                }
            }
            Err(err) => (
                e.attempts + 1,
                Some(err.to_string()),
                self.backoff(e.attempts),
                false,
            ),
        };
        if let Err(err) = self
            .store
            .release(
                &e.id,
                &self.settings.holder,
                again,
                attempts,
                error.as_deref(),
                done,
            )
            .await
        {
            eprintln!("oa-computers: release {}: {err}", e.id);
        }
    }
}
