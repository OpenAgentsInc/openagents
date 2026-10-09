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
        self.settle(&lease, Trigger::Prompt, now_ms).await
    }

    /// The dispatched turn completed: fence its checkpoint and take it
    /// before any queued prompt can run.
    pub async fn turn_finished(&self, id: &str, generation: u64, now_ms: u64) -> Result<Settled> {
        let lease = self.store.lease(id)?;
        lease.apply(&Command::FinishTurn { generation }, now_ms)?;
        self.settle(&lease, Trigger::Tick, now_ms).await
    }

    /// A timer or observer visit. `observed` extends the idle bound.
    pub async fn tick(&self, id: &str, observed: bool, now_ms: u64) -> Result<Settled> {
        let lease = self.store.lease(id)?;
        if observed {
            lease.apply(&Command::Observed, now_ms)?;
        }
        self.settle(&lease, Trigger::Tick, now_ms).await
    }

    /// The running turn's owner is alive ([`HEARTBEAT_EVERY_MS`]). Refused
    /// when this generation's turn is not the one running.
    pub async fn heartbeat(&self, id: &str, generation: u64, now_ms: u64) -> Result<()> {
        let lease = self.store.lease(id)?;
        lease.apply(&Command::Heartbeat { generation }, now_ms)?;
        Ok(())
    }

    /// Stop the computer now (owner request), keeping its checkpoint.
    pub async fn stop(&self, id: &str, now_ms: u64) -> Result<Settled> {
        let lease = self.store.lease(id)?;
        let c = lease.read()?;
        if matches!(
            c.phase,
            Phase::Awake | Phase::Turn { .. } | Phase::Booting | Phase::Checkpointing { .. }
        ) {
            lease.apply(
                &Command::RequestStop {
                    reason: StopReason::Owner,
                },
                now_ms,
            )?;
        }
        self.settle(&lease, Trigger::Tick, now_ms).await
    }

    /// Delete the computer: stop it, check the meter, delete the resource.
    pub async fn delete(&self, id: &str, now_ms: u64) -> Result<Settled> {
        let lease = self.store.lease(id)?;
        let c = lease.read()?;
        if matches!(
            c.phase,
            Phase::Awake | Phase::Turn { .. } | Phase::Booting | Phase::Checkpointing { .. }
        ) {
            lease.apply(
                &Command::RequestStop {
                    reason: StopReason::Owner,
                },
                now_ms,
            )?;
        }
        let settled = self.settle(&lease, Trigger::Tick, now_ms).await?;
        let c = settled.computer();
        if !matches!(c.phase, Phase::Stopped | Phase::Failed { .. } | Phase::New) {
            return Ok(settled);
        }
        lease.apply(&Command::RequestDelete, now_ms)?;
        self.settle(&lease, Trigger::Tick, now_ms).await
    }

    async fn settle(&self, lease: &Lease, trigger: Trigger, now: u64) -> Result<Settled> {
        let mut last: Option<(Decision, Phase)> = None;
        let mut boots = 0;
        for _ in 0..MAX_STEPS {
            let c = lease.read()?;
            let d = decide(&c, trigger, now);
            match d {
                Decision::Skip => return Ok(Settled::Idle(c)),
                Decision::Wait(reason) => return Ok(Settled::Waiting(reason, c)),
                Decision::Refuse(m) => return Ok(Settled::Refused(m, c)),
                Decision::Dispatch { generation } => {
                    let computer = lease.apply(&Command::StartTurn { generation }, now)?;
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
            self.execute(lease, &c, &key.0, now).await?;
            last = Some(key);
        }
        Err(StoreError::Io("The computer did not settle."))
    }

    async fn execute(&self, lease: &Lease, c: &Computer, d: &Decision, now: u64) -> Result<()> {
        let p = &self.provider;
        let resource = |c: &Computer| -> Result<String> {
            c.resource()
                .map(str::to_owned)
                .ok_or(StoreError::Refused(Refusal::NoResource))
        };
        match d {
            Decision::Create { operation } => {
                if !matches!(c.phase, Phase::Creating | Phase::Unknown { .. }) {
                    lease.apply(
                        &Command::RequestCreate {
                            operation: operation.clone(),
                        },
                        now,
                    )?;
                }
                let c = lease.read()?;
                let outcome = p.create(&c, operation).await;
                lease.apply(
                    &Command::ObserveCreate {
                        operation: operation.clone(),
                        outcome,
                    },
                    now,
                )?;
            }
            Decision::Restore { checkpoint } => {
                let c = if matches!(c.phase, Phase::Stopped) {
                    lease.apply(
                        &Command::RequestRestore {
                            checkpoint: checkpoint.clone(),
                        },
                        now,
                    )?
                } else {
                    c.clone()
                };
                let boot = c.boot().expect("restore boot").number;
                let k = checkpoint
                    .as_ref()
                    .and_then(|id| c.checkpoints.iter().find(|k| &k.id == id));
                let outcome = p.restore(&c, &resource(&c)?, k).await;
                lease.apply(&Command::ObserveRestore { boot, outcome }, now)?;
            }
            Decision::ApplyCredentials { boot } => {
                let outcome = p.apply_credentials(c, &resource(c)?).await;
                lease.apply(
                    &Command::ObserveCredentials {
                        boot: *boot,
                        outcome,
                    },
                    now,
                )?;
            }
            Decision::StartService { boot, name } => {
                let s = c
                    .services
                    .iter()
                    .find(|s| &s.name == name)
                    .expect("declared");
                let outcome = p.start_service(c, &resource(c)?, s).await;
                lease.apply(
                    &Command::ObserveService {
                        boot: *boot,
                        name: name.clone(),
                        outcome,
                    },
                    now,
                )?;
            }
            Decision::Checkpoint { generation } => {
                let outcome = p.checkpoint(c, &resource(c)?, *generation).await;
                lease.apply(
                    &Command::ObserveCheckpoint {
                        generation: *generation,
                        outcome,
                    },
                    now,
                )?;
            }
            Decision::Stop { reason } => {
                lease.apply(&Command::RequestStop { reason: *reason }, now)?;
            }
            Decision::ShutdownProcesses { boot } => {
                let outcome = p.shutdown_processes(c, &resource(c)?).await;
                lease.apply(
                    &Command::ObserveShutdown {
                        boot: *boot,
                        outcome,
                    },
                    now,
                )?;
            }
            Decision::StopResource { boot } => {
                let outcome = p.stop(c, &resource(c)?).await;
                lease.apply(
                    &Command::ObserveStop {
                        boot: *boot,
                        outcome,
                    },
                    now,
                )?;
            }
            Decision::CheckMeter { boot } => {
                let outcome = p.meter(c, &resource(c)?).await;
                lease.apply(
                    &Command::ObserveMeter {
                        boot: *boot,
                        outcome,
                    },
                    now,
                )?;
            }
            Decision::DeleteResource { operation } => {
                let requested = c
                    .creates
                    .iter()
                    .any(|a| &a.operation == operation && a.deletion.is_some());
                if !requested {
                    lease.apply(&Command::RequestDeleteResource, now)?;
                }
                let c = lease.read()?;
                let outcome = p.delete(&c, &resource(&c)?).await;
                lease.apply(
                    &Command::ObserveDeleteResource {
                        operation: operation.clone(),
                        outcome,
                    },
                    now,
                )?;
            }
            Decision::Inspect => {
                let outcome = match c.resource() {
                    Some(r) => p.inspect(c, r).await,
                    None => Outcome::failed("no resource to inspect"),
                };
                lease.apply(&Command::ObserveInspection { outcome }, now)?;
            }
            Decision::Skip
            | Decision::Wait(_)
            | Decision::Refuse(_)
            | Decision::Dispatch { .. } => {}
        }
        Ok(())
    }
}
