//! The chat lifecycle on any `computer-driver` driver (#11624).
//!
//! [`OnDriver`] implements [`Provider`] with nothing but the driver
//! contract, so every conformant driver (oa-boat, the in-memory fake, and
//! later a person's own Mac or a Pylon) runs chat computers through the
//! same decide/apply core. It needs Core for everything and Suspend for
//! turn checkpoints, stop, and restore. A turn checkpoint keeps the disk;
//! drivers declaring `memory_suspend` also keep running programs and probe
//! restored services without starting another copy. A driver without Suspend is
//! refused before any state is written ([`OnDriver::admit`]).
//!
//! Credentials: a chat computer carries no per-boot credential values
//! (`credential_names` must be empty here). Secrets go per turn, in memory,
//! with one command ([`computer_driver::Exec::secret_env`]).

use crate::provider::{CheckpointEvidence, Inspection, Meter, Outcome, Provider};
use crate::{BootOrigin, Checkpoint, Computer, Fact, Health, ServiceDecl};
use computer_driver::{CreateRequest, Driver, Exec, Profile, State, Unsupported, admit};
use std::time::Duration;

/// What the lifecycle needs from a driver.
pub const NEEDS: [Profile; 2] = [Profile::Core, Profile::Suspend];

/// Whether a driver with these capabilities can run a chat computer.
pub fn check(caps: &computer_driver::Capabilities) -> Result<(), Unsupported> {
    admit(caps, &NEEDS)
}

pub struct OnDriver<D> {
    pub driver: D,
    /// The immutable image a new computer starts from (an Environment
    /// build); `None` takes the driver's base image.
    pub image: Option<String>,
    pub poll: Duration,
    /// How long a boot or a stop may take before its outcome is unknown.
    pub deadline: Duration,
}

impl<D: Driver> OnDriver<D> {
    pub fn new(driver: D, image: Option<String>) -> Self {
        Self {
            driver,
            image,
            poll: Duration::from_secs(3),
            deadline: Duration::from_secs(900),
        }
    }
    /// Refuse a driver that cannot run a chat computer, before any write.
    pub fn admit(driver: &D) -> Result<(), Unsupported> {
        check(&driver.capabilities())
    }

    /// Wait for `want`; `Failed` if the computer failed or vanished,
    /// `Unknown` at the deadline.
    async fn reach(&self, resource: &str, want: State) -> Outcome<()> {
        let until = tokio::time::Instant::now() + self.deadline;
        loop {
            match self.driver.observe(resource).await {
                Outcome::Done { value } if value.state == want => return Outcome::done(()),
                Outcome::Done { value } if value.state == State::Failed => {
                    return Outcome::failed("the computer failed");
                }
                Outcome::Done { value } if value.state == State::Gone && want != State::Gone => {
                    return Outcome::failed("the computer is gone");
                }
                _ => {}
            }
            if tokio::time::Instant::now() >= until {
                return Outcome::unknown(format!("the computer did not reach {want:?} in time"));
            }
            tokio::time::sleep(self.poll).await;
        }
    }

    async fn sh(&self, resource: &str, command: String, seconds: u64) -> Outcome<i64> {
        let mut e = Exec::new(command);
        e.timeout_seconds = seconds;
        self.driver.exec(resource, &e).await.map(|o| o.code)
    }
}

// Boot numbers and turn generations are saved before any lifecycle effect.
// They survive API restart and are never replaced by a fresh retry identity.
fn operation(c: &Computer, action: &str) -> String {
    format!(
        "{}-boot-{}-{action}",
        c.id,
        c.boot().expect("resource has a boot").number
    )
}

fn health_probe(service: &ServiceDecl) -> String {
    match &service.health {
        Health::Http { port, path } => format!(
            "curl -fsS -o /dev/null http://127.0.0.1:{port}{}",
            quote(path)
        ),
        Health::Command { command } => command.clone(),
    }
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

impl<D: Driver> Provider for OnDriver<D> {
    async fn create(&self, c: &Computer, operation: &str) -> Outcome<String> {
        if !c.credential_names.is_empty() {
            return Outcome::failed("chat computers take credentials per turn, not per boot");
        }
        self.driver
            .create(&CreateRequest {
                operation: operation.into(),
                size: Some(c.size.clone()),
                image: self.image.clone(),
                ttl_seconds: Some((c.bounds.absolute_ms / 1000).max(1)),
                labels: [("computer".to_owned(), c.id.clone())].into(),
            })
            .await
    }

    async fn restore(
        &self,
        c: &Computer,
        resource: &str,
        _checkpoint: Option<&Checkpoint>,
    ) -> Outcome<String> {
        if self.driver.capabilities().memory_suspend {
            // State can change before teardown or wake finishes. Repeating the
            // saved operation waits for the original provider operation too.
            if let other @ (Outcome::Failed { .. } | Outcome::Unknown { .. }) = self
                .driver
                .resume_operation(resource, &operation(c, "resume"))
                .await
            {
                return other;
            }
            return self
                .reach(resource, State::Running)
                .await
                .map(|()| format!("resumed:{resource}"));
        }
        match self.driver.observe(resource).await {
            Outcome::Done { value } if value.state == State::Gone => {
                return Outcome::failed("the computer is gone");
            }
            Outcome::Done { value } if matches!(value.state, State::Stopped | State::Stopping) => {
                if value.state == State::Stopping {
                    if let o @ (Outcome::Failed { .. } | Outcome::Unknown { .. }) =
                        self.reach(resource, State::Stopped).await
                    {
                        return o.map(|()| String::new());
                    }
                }
                if let o @ (Outcome::Failed { .. } | Outcome::Unknown { .. }) = self
                    .driver
                    .resume_operation(resource, &operation(c, "resume"))
                    .await
                {
                    return o;
                }
            }
            Outcome::Done { .. } => {}
            Outcome::Failed { reason } | Outcome::Unknown { reason } => {
                return Outcome::unknown(format!("read before resume: {reason}"));
            }
        }
        self.reach(resource, State::Running)
            .await
            .map(|()| format!("resumed:{resource}"))
    }

    async fn apply_credentials(&self, c: &Computer, resource: &str) -> Outcome<String> {
        // Nothing to apply per boot; this is where a fresh boot is awaited.
        if let other @ (Outcome::Failed { .. } | Outcome::Unknown { .. }) =
            self.reach(resource, State::Running).await
        {
            return other.map(|()| String::new());
        }
        if !self.driver.capabilities().memory_suspend {
            return Outcome::done("applied:".into());
        }
        if !matches!(
            self.sh(resource, ":".into(), 30).await,
            Outcome::Done { value: 0 }
        ) {
            return Outcome::unknown("The computer is running, but commands aren't ready yet.");
        }
        if c.boot()
            .is_some_and(|boot| matches!(boot.origin, BootOrigin::Restored { .. }))
        {
            for service in &c.services {
                if !matches!(
                    self.sh(resource, health_probe(service), 30).await,
                    Outcome::Done { value: 0 }
                ) {
                    return Outcome::unknown(format!("{} isn't ready after wake.", service.name));
                }
            }
        }
        Outcome::done("command service ready".into())
    }

    async fn start_service(
        &self,
        c: &Computer,
        resource: &str,
        s: &ServiceDecl,
    ) -> Outcome<String> {
        let start = format!(
            "mkdir -p \"$HOME/.oa-services\" && cd {} && (nohup sh -c {} > \"$HOME/.oa-services/{}.log\" 2>&1 < /dev/null &)",
            quote(&s.cwd),
            quote(&s.command),
            s.name
        );
        let restored = self.driver.capabilities().memory_suspend
            && c.boot()
                .is_some_and(|boot| matches!(boot.origin, BootOrigin::Restored { .. }));
        if !restored {
            match self.sh(resource, start, 60).await {
                Outcome::Done { value: 0 } => {}
                Outcome::Done { value } => return Outcome::failed(format!("start exited {value}")),
                other => return other.map(|_| String::new()),
            }
        }
        let probe = health_probe(s);
        let until =
            tokio::time::Instant::now() + Duration::from_secs(s.ready_within_seconds.into());
        loop {
            if let Outcome::Done { value: 0 } = self.sh(resource, probe.clone(), 30).await {
                return Outcome::done(format!("ready:{}", s.name));
            }
            if tokio::time::Instant::now() >= until {
                return Outcome::failed(format!("{} was not ready in time", s.name));
            }
            tokio::time::sleep(self.poll).await;
        }
    }

    async fn checkpoint(
        &self,
        c: &Computer,
        resource: &str,
        generation: u64,
    ) -> Outcome<CheckpointEvidence> {
        let stop = match self
            .driver
            .suspend_operation(resource, &operation(c, &format!("checkpoint-{generation}")))
            .await
        {
            Outcome::Done { value } => value,
            other => return other.map(|_| unreachable!()),
        };
        self.reach(resource, State::Stopped)
            .await
            .map(|()| CheckpointEvidence {
                snapshot: format!("{resource}@turn-{generation}"),
                stopped: Some(stop),
            })
    }

    async fn shutdown_processes(&self, _c: &Computer, resource: &str) -> Outcome<String> {
        if self.driver.capabilities().memory_suspend {
            // Stop preserves ordinary services. The command owner must finish
            // explicit cancellation before allowing this suspension.
            return Outcome::done("programs kept for suspension".into());
        }
        match self.driver.observe(resource).await {
            Outcome::Done { value } if value.state == State::Running => {
                let _ = self
                    .sh(
                        resource,
                        "pkill -f '[.]oa-services' 2>/dev/null; true".into(),
                        30,
                    )
                    .await;
                Outcome::done("shutdown".into())
            }
            Outcome::Done { .. } => Outcome::done("not running".into()),
            Outcome::Failed { reason } | Outcome::Unknown { reason } => Outcome::unknown(reason),
        }
    }

    async fn stop(&self, c: &Computer, resource: &str) -> Outcome<String> {
        if let Outcome::Done { value } = self.driver.observe(resource).await
            && value.state == State::Gone
        {
            return Outcome::done(format!("gone:{resource}"));
        }
        if !self.driver.capabilities().memory_suspend
            && let Outcome::Done { value } = self.driver.observe(resource).await
            && matches!(value.state, State::Stopped | State::Gone)
        {
            return Outcome::done(format!("stopped:{resource}"));
        }
        let op = match self
            .driver
            .suspend_operation(resource, &operation(c, "stop"))
            .await
        {
            Outcome::Done { value } => value,
            other => return other,
        };
        self.reach(resource, State::Stopped).await.map(|()| op)
    }

    async fn meter(&self, _c: &Computer, resource: &str) -> Outcome<Meter> {
        self.driver.meter(resource).await
    }

    async fn delete(&self, _c: &Computer, resource: &str) -> Outcome<String> {
        match self.driver.delete(resource).await {
            Outcome::Done { .. } => self
                .reach(resource, State::Gone)
                .await
                .map(|()| format!("deleted:{resource}")),
            other => other.map(|()| String::new()),
        }
    }

    async fn inspect(&self, c: &Computer, resource: &str) -> Outcome<Inspection> {
        // Actual deletion settles uncertainty even if the provider no longer
        // retains the earlier operation. It never authorizes another create.
        if let Outcome::Done { value } = self.driver.observe(resource).await
            && value.state == State::Gone
        {
            return Outcome::done(Inspection {
                running: None,
                stop: None,
                latest_snapshot: None,
            });
        }
        let mut stop = None;
        let mut latest_snapshot = None;
        if self.driver.capabilities().memory_suspend {
            if c.boot()
                .is_some_and(|b| b.restore.as_ref().is_some_and(Fact::is_unknown))
            {
                if let other @ (Outcome::Failed { .. } | Outcome::Unknown { .. }) =
                    self.restore(c, resource, None).await
                {
                    return other.map(|_| unreachable!());
                }
            } else if let Some(k) = c.checkpoints.last().filter(|k| k.fact.is_unknown()) {
                match self.checkpoint(c, resource, k.turn_generation).await {
                    Outcome::Done { value } => {
                        stop = value.stopped;
                        latest_snapshot = Some(value.snapshot);
                    }
                    other => return other.map(|_| unreachable!()),
                }
            } else if c
                .boot()
                .is_some_and(|b| b.resource_stop.as_ref().is_some_and(Fact::is_unknown))
            {
                match self.stop(c, resource).await {
                    Outcome::Done { value } => stop = Some(value),
                    other => return other.map(|_| unreachable!()),
                }
            }
        }
        self.driver.observe(resource).await.map(|o| Inspection {
            running: match o.state {
                State::Gone => None,
                State::Running | State::Starting => Some(true),
                _ => Some(false),
            },
            stop,
            latest_snapshot,
        })
    }
}
