//! Runs decisions against a provider and retains every observed outcome.
//!
//! The driver holds the computer's lease across a whole settle, so a second
//! owner gets [`StoreError::Busy`] instead of racing provider effects. Each
//! step reads the retained record, asks [`decide`], performs that one effect,
//! and retains its observation before deciding again.

use crate::provider::{Outcome, Provider};
use crate::store::{Lease, Result, Store, StoreError};
use crate::*;

/// Where a settle ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Settled {
    /// The computer is awake and this generation was dispatched; run the
    /// turn, then report it with [`Driver::turn_finished`].
    Dispatch {
        generation: u64,
        computer: Computer,
    },
    Waiting(WaitReason, Computer),
    /// Nothing more to do now.
    Idle(Computer),
    Refused(String, Computer),
    /// The same effect did not change the outcome (failed or unknown); the
    /// record says why. Try again later or reconcile.
    Stuck(Decision, Computer),
    /// A boot failed during this settle and was cleaned up; a later prompt
    /// may try again. One settle starts at most one boot.
    BootFailed(Computer),
}
impl Settled {
    pub fn computer(&self) -> &Computer {
        match self {
            Self::Dispatch { computer, .. }
            | Self::Waiting(_, computer)
            | Self::Idle(computer)
            | Self::Refused(_, computer)
            | Self::Stuck(_, computer)
            | Self::BootFailed(computer) => computer,
        }
    }
}

const MAX_STEPS: usize = 64;

/// An exclusive hold on one computer's record: the file [`Lease`] here, a
/// Postgres row lease in `oa-computers` (#11624). Every observation is
/// retained through [`Held::apply`] before the next decision.
#[allow(async_fn_in_trait)]
pub trait Held {
    async fn read(&self) -> Result<Computer>;
    /// Apply one command and retain the result before returning it.
    async fn apply(&self, command: &Command, now_ms: u64) -> Result<Computer>;
}

impl Held for Lease {
    async fn read(&self) -> Result<Computer> {
        Lease::read(self)
    }
    async fn apply(&self, command: &Command, now_ms: u64) -> Result<Computer> {
        Lease::apply(self, command, now_ms)
    }
}

pub struct Driver<P> {
    pub store: Store,
    pub provider: P,
}

impl<P: Provider> Driver<P> {
    pub fn new(store: Store, provider: P) -> Self {
        Self { store, provider }
    }

    /// A queued prompt wants the computer: create or restore it, apply
    /// credentials and services, finish any fenced checkpoint, then
    /// dispatch the next generation.
    pub async fn prompt(&self, id: &str, now_ms: u64) -> Result<Settled> {
        let lease = self.store.lease(id)?;
        settle(&self.provider, &lease, Trigger::Prompt, now_ms).await
    }

    /// The dispatched turn completed: fence its checkpoint and take it
    /// before any queued prompt can run.
    pub async fn turn_finished(&self, id: &str, generation: u64, now_ms: u64) -> Result<Settled> {
        let lease = self.store.lease(id)?;
        turn_finished(&self.provider, &lease, generation, now_ms).await
    }

    /// A timer or observer visit. `observed` extends the idle bound.
    pub async fn tick(&self, id: &str, observed: bool, now_ms: u64) -> Result<Settled> {
        let lease = self.store.lease(id)?;
        tick(&self.provider, &lease, observed, now_ms).await
    }

    /// The running turn's owner is alive ([`HEARTBEAT_EVERY_MS`]). Refused
    /// when this generation's turn is not the one running.
    pub async fn heartbeat(&self, id: &str, generation: u64, now_ms: u64) -> Result<()> {
        let lease = self.store.lease(id)?;
        Held::apply(&lease, &Command::Heartbeat { generation }, now_ms).await?;
        Ok(())
    }

    /// Stop the computer now (owner request), keeping its checkpoint.
    pub async fn stop(&self, id: &str, now_ms: u64) -> Result<Settled> {
        let lease = self.store.lease(id)?;
        stop(&self.provider, &lease, now_ms).await
    }

    /// Delete the computer: stop it, check the meter, delete the resource.
    pub async fn delete(&self, id: &str, now_ms: u64) -> Result<Settled> {
        let lease = self.store.lease(id)?;
        delete(&self.provider, &lease, now_ms).await
    }
}

/// [`Driver::turn_finished`] on any held record.
pub async fn turn_finished<P: Provider, H: Held>(
    provider: &P,
    lease: &H,
    generation: u64,
    now_ms: u64,
) -> Result<Settled> {
    lease
        .apply(&Command::FinishTurn { generation }, now_ms)
        .await?;
    settle(provider, lease, Trigger::Tick, now_ms).await
}

/// [`Driver::tick`] on any held record.
pub async fn tick<P: Provider, H: Held>(
    provider: &P,
    lease: &H,
    observed: bool,
    now_ms: u64,
) -> Result<Settled> {
    if observed {
        lease.apply(&Command::Observed, now_ms).await?;
    }
    settle(provider, lease, Trigger::Tick, now_ms).await
}

fn running(c: &Computer) -> bool {
    matches!(
        c.phase,
        Phase::Awake | Phase::Turn { .. } | Phase::Booting | Phase::Checkpointing { .. }
    )
}

/// [`Driver::stop`] on any held record.
pub async fn stop<P: Provider, H: Held>(provider: &P, lease: &H, now_ms: u64) -> Result<Settled> {
    let c = lease.read().await?;
    if running(&c) {
        lease
            .apply(
                &Command::RequestStop {
                    reason: StopReason::Owner,
                },
                now_ms,
            )
            .await?;
    }
    settle(provider, lease, Trigger::Tick, now_ms).await
}

/// [`Driver::delete`] on any held record.
pub async fn delete<P: Provider, H: Held>(provider: &P, lease: &H, now_ms: u64) -> Result<Settled> {
    let c = lease.read().await?;
    if matches!(c.phase, Phase::Deleted) {
        return Ok(Settled::Idle(c));
    }
    if running(&c) {
        lease
            .apply(
                &Command::RequestStop {
                    reason: StopReason::Owner,
                },
                now_ms,
            )
            .await?;
    }
    let settled = settle(provider, lease, Trigger::Tick, now_ms).await?;
    let c = settled.computer();
    if !matches!(c.phase, Phase::Stopped | Phase::Failed { .. } | Phase::New) {
        return Ok(settled);
    }
    lease.apply(&Command::RequestDelete, now_ms).await?;
    settle(provider, lease, Trigger::Tick, now_ms).await
}

/// Decide, perform one effect, retain its observation, and decide again
/// until there is nothing more to do now.
pub async fn settle<P: Provider, H: Held>(
    provider: &P,
    lease: &H,
    trigger: Trigger,
    now: u64,
) -> Result<Settled> {
    let mut last: Option<(Decision, Phase)> = None;
    let mut boots = 0;
    for _ in 0..MAX_STEPS {
        let c = lease.read().await?;
        let d = decide(&c, trigger, now);
        match d {
            Decision::Skip => return Ok(Settled::Idle(c)),
            Decision::Wait(reason) => return Ok(Settled::Waiting(reason, c)),
            Decision::Refuse(m) => return Ok(Settled::Refused(m, c)),
            Decision::Dispatch { generation } => {
                let computer = lease.apply(&Command::StartTurn { generation }, now).await?;
                return Ok(Settled::Dispatch {
                    generation,
                    computer,
                });
            }
            _ => {}
        }
        let starts_boot = match &d {
            Decision::Create { .. } => matches!(c.phase, Phase::New | Phase::Failed { .. }),
            Decision::Restore { .. } => matches!(c.phase, Phase::Stopped),
            _ => false,
        };
        if starts_boot {
            if boots > 0 {
                return Ok(Settled::BootFailed(c));
            }
            boots += 1;
        }
        let key = (d, c.phase.clone());
        if last.as_ref() == Some(&key) {
            return Ok(Settled::Stuck(key.0, c));
        }
        execute(provider, lease, &c, &key.0, now).await?;
        last = Some(key);
    }
    Err(StoreError::Io("The computer did not settle."))
}

pub(crate) async fn execute<P: Provider, H: Held>(
    p: &P,
    lease: &H,
    c: &Computer,
    d: &Decision,
    now: u64,
) -> Result<()> {
    let resource = |c: &Computer| -> Result<String> {
        c.resource()
            .map(str::to_owned)
            .ok_or(StoreError::Refused(Refusal::NoResource))
    };
    match d {
        Decision::Create { operation } => {
            if !matches!(c.phase, Phase::Creating | Phase::Unknown { .. }) {
                lease
                    .apply(
                        &Command::RequestCreate {
                            operation: operation.clone(),
                        },
                        now,
                    )
                    .await?;
            }
            let c = lease.read().await?;
            let outcome = p.create(&c, operation).await;
            lease
                .apply(
                    &Command::ObserveCreate {
                        operation: operation.clone(),
                        outcome,
                    },
                    now,
                )
                .await?;
        }
        Decision::Restore { checkpoint } => {
            let c = if matches!(c.phase, Phase::Stopped) {
                lease
                    .apply(
                        &Command::RequestRestore {
                            checkpoint: checkpoint.clone(),
                        },
                        now,
                    )
                    .await?
            } else {
                c.clone()
            };
            let boot = c.boot().expect("restore boot").number;
            let k = checkpoint
                .as_ref()
                .and_then(|id| c.checkpoints.iter().find(|k| &k.id == id));
            let outcome = p.restore(&c, &resource(&c)?, k).await;
            lease
                .apply(&Command::ObserveRestore { boot, outcome }, now)
                .await?;
        }
        Decision::ApplyCredentials { boot } => {
            let outcome = p.apply_credentials(c, &resource(c)?).await;
            lease
                .apply(
                    &Command::ObserveCredentials {
                        boot: *boot,
                        outcome,
                    },
                    now,
                )
                .await?;
        }
        Decision::StartService { boot, name } => {
            let s = c
                .services
                .iter()
                .find(|s| &s.name == name)
                .expect("declared");
            let outcome = p.start_service(c, &resource(c)?, s).await;
            lease
                .apply(
                    &Command::ObserveService {
                        boot: *boot,
                        name: name.clone(),
                        outcome,
                    },
                    now,
                )
                .await?;
        }
        Decision::Checkpoint { generation } => {
            let outcome = p.checkpoint(c, &resource(c)?, *generation).await;
            lease
                .apply(
                    &Command::ObserveCheckpoint {
                        generation: *generation,
                        outcome,
                    },
                    now,
                )
                .await?;
        }
        Decision::Stop { reason } => {
            lease
                .apply(&Command::RequestStop { reason: *reason }, now)
                .await?;
        }
        Decision::ShutdownProcesses { boot } => {
            let outcome = p.shutdown_processes(c, &resource(c)?).await;
            lease
                .apply(
                    &Command::ObserveShutdown {
                        boot: *boot,
                        outcome,
                    },
                    now,
                )
                .await?;
        }
        Decision::StopResource { boot } => {
            let outcome = p.stop(c, &resource(c)?).await;
            lease
                .apply(
                    &Command::ObserveStop {
                        boot: *boot,
                        outcome,
                    },
                    now,
                )
                .await?;
        }
        Decision::CheckMeter { boot } => {
            let outcome = p.meter(c, &resource(c)?).await;
            lease
                .apply(
                    &Command::ObserveMeter {
                        boot: *boot,
                        outcome,
                    },
                    now,
                )
                .await?;
        }
        Decision::DeleteResource { operation } => {
            let requested = c
                .creates
                .iter()
                .any(|a| &a.operation == operation && a.deletion.is_some());
            if !requested {
                lease.apply(&Command::RequestDeleteResource, now).await?;
            }
            let c = lease.read().await?;
            let outcome = p.delete(&c, &resource(&c)?).await;
            lease
                .apply(
                    &Command::ObserveDeleteResource {
                        operation: operation.clone(),
                        outcome,
                    },
                    now,
                )
                .await?;
        }
        Decision::Inspect => {
            let outcome = match c.resource() {
                Some(r) => p.inspect(c, r).await,
                None => Outcome::failed("no resource to inspect"),
            };
            lease
                .apply(&Command::ObserveInspection { outcome }, now)
                .await?;
        }
        Decision::Skip | Decision::Wait(_) | Decision::Refuse(_) | Decision::Dispatch { .. } => {}
    }
    Ok(())
}
