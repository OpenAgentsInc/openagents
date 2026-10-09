//! Pure working-computer transitions.
//!
//! [`apply`] performs no I/O. It returns the next record or a typed
//! [`Refusal`]; callers retain the record before acting on it. Provider
//! outcomes enter only as observations, and an unknown outcome is retained
//! as unknown until a definite observation (an inspection, or an idempotent
//! repeat) reconciles it.

use crate::provider::{CheckpointEvidence, Inspection, Meter, Outcome};
use crate::*;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// Retain the create operation identity before calling the provider.
    RequestCreate {
        operation: String,
    },
    ObserveCreate {
        operation: String,
        outcome: Outcome<String>,
    },
    /// Begin a restore boot from the retained stopped resource.
    RequestRestore {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        checkpoint: Option<String>,
    },
    ObserveRestore {
        boot: u32,
        outcome: Outcome<String>,
    },
    ObserveCredentials {
        boot: u32,
        outcome: Outcome<String>,
    },
    ObserveService {
        boot: u32,
        name: String,
        outcome: Outcome<String>,
    },
    StartTurn {
        generation: u64,
    },
    /// The running turn's owner is alive.
    Heartbeat {
        generation: u64,
    },
    /// The fence: the computer quiesces for this generation's checkpoint.
    FinishTurn {
        generation: u64,
    },
    ObserveCheckpoint {
        generation: u64,
        outcome: Outcome<CheckpointEvidence>,
    },
    /// Someone is watching; extend the idle deadline within the absolute one.
    Observed,
    RequestStop {
        reason: StopReason,
    },
    ObserveShutdown {
        boot: u32,
        outcome: Outcome<String>,
    },
    ObserveStop {
        boot: u32,
        outcome: Outcome<String>,
    },
    ObserveMeter {
        boot: u32,
        outcome: Outcome<Meter>,
    },
    /// Delete the stopped provider resource (cleanup after a failed fresh
    /// boot, or as part of deleting the computer).
    RequestDeleteResource,
    ObserveDeleteResource {
        operation: String,
        outcome: Outcome<String>,
    },
    /// Delete the computer once its resource is stopped or absent.
    RequestDelete,
    /// A read-only provider view reconciling an unknown outcome.
    ObserveInspection {
        outcome: Outcome<Inspection>,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    /// Retain this record before acting on it.
    Changed(Box<Computer>),
    /// Nothing to retain (already true, or an unknown stayed unknown).
    Unchanged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    Invalid(&'static str),
    /// The command does not apply in the current phase.
    Phase(String),
    /// The observation names a different turn generation than the fence.
    StaleGeneration {
        fence: u64,
        observed: u64,
    },
    /// The observation names an earlier boot.
    StaleBoot {
        current: u32,
        observed: u32,
    },
    StaleOperation(String),
    UnknownCheckpoint(String),
    CheckpointNotDone(String),
    /// Checkpoints belong to their own computer and owner only.
    CheckpointCustody,
    NoResource,
    Deleted,
    Limit(&'static str),
}
impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(m) | Self::Limit(m) => f.write_str(m),
            Self::Phase(p) => write!(f, "The computer is {p}; that change does not apply."),
            Self::StaleGeneration { fence, observed } => write!(
                f,
                "The checkpoint is fenced to turn {fence}, not turn {observed}."
            ),
            Self::StaleBoot { current, observed } => {
                write!(f, "Boot {observed} is not the current boot {current}.")
            }
            Self::StaleOperation(op) => write!(f, "Operation {op} is not the current one."),
            Self::UnknownCheckpoint(id) => write!(f, "No checkpoint {id}."),
            Self::CheckpointNotDone(id) => write!(f, "Checkpoint {id} did not complete."),
            Self::CheckpointCustody => f.write_str(
                "A checkpoint is private to its own computer and owner; it cannot be copied, read, or saved as an environment.",
            ),
            Self::NoResource => f.write_str("The computer has no provider resource."),
            Self::Deleted => f.write_str("The computer was deleted."),
        }
    }
}
impl std::error::Error for Refusal {}

fn phase_name(p: &Phase) -> String {
    serde_json::to_value(p)
        .ok()
        .and_then(|v| v["phase"].as_str().map(str::to_owned))
        .unwrap_or_default()
}
fn wrong(c: &Computer) -> Refusal {
    Refusal::Phase(phase_name(&c.phase))
}
fn reason(text: &str) -> String {
    let mut t: String = text
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_REASON_BYTES / 4)
        .collect();
    if t.is_empty() {
        t = "unspecified".into();
    }
    t
}
fn fact_of<T>(outcome: &Outcome<T>, evidence: impl FnOnce(&T) -> String, now: u64) -> Fact {
    match outcome {
        Outcome::Done { value } => Fact::Done {
            evidence: reason(&evidence(value)),
            at_ms: now,
        },
        Outcome::Failed { reason: r } => Fact::Failed {
            reason: reason(r),
            at_ms: now,
        },
        Outcome::Unknown { reason: r } => Fact::Unknown {
            reason: reason(r),
            at_ms: now,
        },
    }
}
fn current_boot(c: &Computer, boot: u32) -> Result<(), Refusal> {
    let current = c.boot().map(|b| b.number).ok_or(Refusal::NoResource)?;
    if current != boot {
        return Err(Refusal::StaleBoot {
            current,
            observed: boot,
        });
    }
    Ok(())
}
fn new_boot(c: &mut Computer, resource: String, origin: BootOrigin, now: u64) {
    let number = c.boots.last().map_or(1, |b| b.number + 1);
    let restore = matches!(origin, BootOrigin::Restored { .. }).then_some(Fact::Requested {
        operation: None,
        at_ms: now,
    });
    c.boots.push(Boot {
        number,
        resource,
        origin,
        started_ms: now,
        restore,
        credentials: None,
        services: BTreeMap::new(),
        idle_deadline_ms: now.saturating_add(c.bounds.idle_ms),
        absolute_deadline_ms: now.saturating_add(c.bounds.absolute_ms),
        stop_reason: None,
        shutdown: None,
        resource_stop: None,
        meter_stop: None,
    });
}
/// Reset the idle deadline after activity, within the absolute bound.
fn touch(c: &mut Computer, now: u64) {
    let idle = c.bounds.idle_ms;
    if let Some(b) = c.boot_mut() {
        b.idle_deadline_ms = now.saturating_add(idle).min(b.absolute_deadline_ms);
    }
}
/// Enter Awake once credentials are applied and every declared service has
/// reported (ready or not).
fn maybe_awake(c: &mut Computer, now: u64) {
    let Some(b) = c.boot() else { return };
    if done(&b.credentials) && c.services.iter().all(|s| b.services.contains_key(&s.name)) {
        c.phase = Phase::Awake;
        touch(c, now);
    }
}
/// Count a failed boot toward the breaker.
fn boot_failed(c: &mut Computer, now: u64) {
    c.boot_failures.push(now);
    let extra = c.boot_failures.len().saturating_sub(MAX_BOOT_FAILURES);
    c.boot_failures.drain(..extra);
}
fn latest_create_mut(c: &mut Computer) -> Option<&mut CreateAttempt> {
    c.creates.last_mut()
}

/// Apply one command at `now_ms`.
pub fn apply(c: &Computer, command: &Command, now_ms: u64) -> Result<Applied, Refusal> {
    if matches!(c.phase, Phase::Deleted) {
        return Err(Refusal::Deleted);
    }
    let mut n = c.clone();
    let now = now_ms;
    let changed = match command {
        Command::RequestCreate { operation } => {
            if !valid_id(operation) {
                return Err(Refusal::Invalid("Operation IDs must be opaque identities."));
            }
            let fresh = matches!(c.phase, Phase::New)
                || (matches!(c.phase, Phase::Failed { .. }) && c.resource().is_none());
            if c.creates.iter().any(|a| &a.operation == operation) {
                return if matches!(c.phase, Phase::Creating) {
                    Ok(Applied::Unchanged)
                } else {
                    Err(Refusal::StaleOperation(operation.clone()))
                };
            }
            if !fresh {
                return Err(wrong(c));
            }
            if c.creates.len() >= MAX_CREATES {
                return Err(Refusal::Limit(
                    "The computer retains too many create attempts.",
                ));
            }
            n.creates.push(CreateAttempt {
                operation: operation.clone(),
                fact: Fact::Requested {
                    operation: Some(operation.clone()),
                    at_ms: now,
                },
                resource: None,
                deletion: None,
            });
            n.phase = Phase::Creating;
            true
        }
        Command::ObserveCreate { operation, outcome } => {
            let creating = matches!(c.phase, Phase::Creating)
                || (matches!(c.phase, Phase::Unknown { .. })
                    && c.creates.last().is_some_and(|a| a.fact.is_unknown()));
            if !creating {
                return Err(wrong(c));
            }
            if c.creates.last().map(|a| &a.operation) != Some(operation) {
                return Err(Refusal::StaleOperation(operation.clone()));
            }
            let attempt = latest_create_mut(&mut n).expect("checked");
            attempt.fact = fact_of(outcome, |r| format!("resource:{r}"), now);
            match outcome {
                Outcome::Done { value } => {
                    if !valid_id(value) {
                        return Err(Refusal::Invalid("The provider resource ID is invalid."));
                    }
                    attempt.resource = Some(value.clone());
                    new_boot(&mut n, value.clone(), BootOrigin::Created, now);
                    n.phase = Phase::Booting;
                }
                Outcome::Failed { reason: r } => {
                    boot_failed(&mut n, now);
                    n.phase = Phase::Failed { reason: reason(r) }
                }
                Outcome::Unknown { reason: r } => {
                    n.phase = Phase::Unknown {
                        reason: format!("create: {}", reason(r)),
                    }
                }
            }
            true
        }
        Command::RequestRestore { checkpoint } => {
            let resource = c.resource().ok_or(Refusal::NoResource)?.to_owned();
            if !matches!(c.phase, Phase::Stopped) {
                return Err(wrong(c));
            }
            if let Some(id) = checkpoint {
                admit_checkpoint_use(
                    c,
                    id,
                    &CheckpointUse::Restore {
                        computer: &c.id,
                        principal: &c.owner,
                    },
                )?;
                if c.latest_checkpoint().map(|k| &k.id) != Some(id) {
                    return Err(Refusal::Invalid(
                        "Only the latest checkpoint of the retained resource can be restored.",
                    ));
                }
            }
            if c.boots.len() >= MAX_BOOTS {
                return Err(Refusal::Limit("The computer retains too many boots."));
            }
            new_boot(
                &mut n,
                resource,
                BootOrigin::Restored {
                    checkpoint: checkpoint.clone(),
                },
                now,
            );
            n.phase = Phase::Booting;
            true
        }
        Command::ObserveRestore { boot, outcome } => {
            current_boot(c, *boot)?;
            let b = c.boot().expect("checked");
            let reconciling = matches!(c.phase, Phase::Unknown { .. })
                && b.restore.as_ref().is_some_and(Fact::is_unknown);
            if !(matches!(c.phase, Phase::Booting) || reconciling)
                || b.restore.as_ref().is_none_or(Fact::is_settled)
            {
                return Err(wrong(c));
            }
            n.boot_mut().unwrap().restore = Some(fact_of(outcome, Clone::clone, now));
            match outcome {
                Outcome::Done { .. } => n.phase = Phase::Booting,
                Outcome::Failed { .. } => {
                    // The resource never resumed: it stays stopped with its
                    // retained filesystem.
                    let b = n.boot_mut().unwrap();
                    b.resource_stop = Some(Fact::Done {
                        evidence: "not resumed".into(),
                        at_ms: now,
                    });
                    b.shutdown = Some(Fact::Done {
                        evidence: "not resumed".into(),
                        at_ms: now,
                    });
                    b.stop_reason = Some(StopReason::FailedBoot);
                    boot_failed(&mut n, now);
                    n.phase = Phase::Stopped;
                }
                Outcome::Unknown { reason: r } => {
                    n.phase = Phase::Unknown {
                        reason: format!("restore: {}", reason(r)),
                    }
                }
            }
            true
        }
        Command::ObserveCredentials { boot, outcome } => {
            current_boot(c, *boot)?;
            let b = c.boot().expect("checked");
            if !matches!(c.phase, Phase::Booting)
                || !(b.restore.is_none() || done(&b.restore))
                || done(&b.credentials)
            {
                return Err(wrong(c));
            }
            n.boot_mut().unwrap().credentials = Some(fact_of(outcome, Clone::clone, now));
            if let Outcome::Failed { reason: r } = outcome {
                boot_failed(&mut n, now);
                n.phase = Phase::Failed {
                    reason: format!("credentials: {}", reason(r)),
                };
            } else {
                // Unknown stays Booting: applying credentials is idempotent,
                // so the decision is to apply them again.
                maybe_awake(&mut n, now);
            }
            true
        }
        Command::ObserveService {
            boot,
            name,
            outcome,
        } => {
            current_boot(c, *boot)?;
            if !matches!(c.phase, Phase::Booting) || !done(&c.boot().unwrap().credentials) {
                return Err(wrong(c));
            }
            if !c.services.iter().any(|s| &s.name == name) {
                return Err(Refusal::Invalid("That service is not declared."));
            }
            if c.boot().unwrap().services.contains_key(name) {
                return Ok(Applied::Unchanged);
            }
            n.boot_mut()
                .unwrap()
                .services
                .insert(name.clone(), fact_of(outcome, Clone::clone, now));
            maybe_awake(&mut n, now);
            true
        }
        Command::StartTurn { generation } => {
            if let Phase::Turn { generation: g } = c.phase {
                return if g == *generation {
                    Ok(Applied::Unchanged)
                } else {
                    Err(wrong(c))
                };
            }
            if !matches!(c.phase, Phase::Awake) {
                return Err(wrong(c));
            }
            if *generation != c.turn.dispatched + 1 {
                return Err(Refusal::StaleGeneration {
                    fence: c.turn.dispatched + 1,
                    observed: *generation,
                });
            }
            n.turn.dispatched = *generation;
            n.turn.heartbeat_ms = now;
            n.phase = Phase::Turn {
                generation: *generation,
            };
            touch(&mut n, now);
            true
        }
        Command::Heartbeat { generation } => {
            let Phase::Turn { generation: g } = c.phase else {
                return Err(wrong(c));
            };
            if g != *generation {
                return Err(Refusal::StaleGeneration {
                    fence: g,
                    observed: *generation,
                });
            }
            if now <= c.turn.heartbeat_ms {
                return Ok(Applied::Unchanged);
            }
            n.turn.heartbeat_ms = now;
            true
        }
        Command::FinishTurn { generation } => {
            match c.phase {
                Phase::Turn { generation: g } if g == *generation => {}
                Phase::Checkpointing { generation: g } if g == *generation => {
                    return Ok(Applied::Unchanged);
                }
                Phase::Turn { generation: g } | Phase::Checkpointing { generation: g } => {
                    return Err(Refusal::StaleGeneration {
                        fence: g,
                        observed: *generation,
                    });
                }
                _ => return Err(wrong(c)),
            }
            if c.checkpoints.len() >= MAX_CHECKPOINTS {
                return Err(Refusal::Limit("The computer retains too many checkpoints."));
            }
            let b = c.boot().expect("a turn runs on a boot");
            n.turn.completed = *generation;
            n.checkpoints.push(Checkpoint {
                id: format!("{}-turn-{generation}", c.id),
                turn_generation: *generation,
                boot: b.number,
                resource: b.resource.clone(),
                fact: Fact::Requested {
                    operation: None,
                    at_ms: now,
                },
                custody: Custody::UserPrivate {
                    principal: c.owner.clone(),
                    computer: c.id.clone(),
                },
                may_hold_user_logins: true,
            });
            n.phase = Phase::Checkpointing {
                generation: *generation,
            };
            true
        }
        Command::ObserveCheckpoint {
            generation,
            outcome,
        } => {
            let fence = match &c.phase {
                Phase::Checkpointing { generation: g } => *g,
                Phase::Unknown { .. }
                    if c.checkpoints.last().is_some_and(|k| k.fact.is_unknown()) =>
                {
                    c.checkpoints.last().unwrap().turn_generation
                }
                _ => return Err(wrong(c)),
            };
            if fence != *generation {
                return Err(Refusal::StaleGeneration {
                    fence,
                    observed: *generation,
                });
            }
            if let Outcome::Done { value } = outcome {
                if c.checkpoints
                    .iter()
                    .any(|k| k.fact.evidence() == Some(&value.snapshot))
                {
                    return Err(Refusal::Invalid(
                        "The provider returned an earlier snapshot for a new turn.",
                    ));
                }
            }
            let k = n.checkpoints.last_mut().expect("fenced checkpoint");
            k.fact = fact_of(outcome, |e| e.snapshot.clone(), now);
            match outcome {
                Outcome::Done { value } => {
                    if let Some(stop) = &value.stopped {
                        let b = n.boot_mut().unwrap();
                        b.stop_reason = Some(StopReason::Checkpoint);
                        b.shutdown = Some(Fact::Done {
                            evidence: format!("stopped with resource: {}", reason(stop)),
                            at_ms: now,
                        });
                        b.resource_stop = Some(Fact::Done {
                            evidence: reason(stop),
                            at_ms: now,
                        });
                        n.phase = Phase::Stopped;
                    } else {
                        n.phase = Phase::Awake;
                        touch(&mut n, now);
                    }
                }
                // Nothing happened; the resource still runs this turn's files.
                Outcome::Failed { .. } => {
                    n.phase = Phase::Awake;
                    touch(&mut n, now);
                }
                Outcome::Unknown { reason: r } => {
                    n.phase = Phase::Unknown {
                        reason: format!("checkpoint: {}", reason(r)),
                    }
                }
            }
            true
        }
        Command::Observed => {
            let ext = c.bounds.observed_extension_ms;
            match c.phase {
                Phase::Awake
                | Phase::Turn { .. }
                | Phase::Booting
                | Phase::Checkpointing { .. } => {
                    let b = n.boot_mut().ok_or(Refusal::NoResource)?;
                    let extended = b
                        .idle_deadline_ms
                        .max(now.saturating_add(ext))
                        .min(b.absolute_deadline_ms);
                    if extended == b.idle_deadline_ms {
                        return Ok(Applied::Unchanged);
                    }
                    b.idle_deadline_ms = extended;
                    true
                }
                _ => return Ok(Applied::Unchanged),
            }
        }
        Command::RequestStop { reason: why } => {
            let stoppable = matches!(
                c.phase,
                Phase::Awake | Phase::Turn { .. } | Phase::Booting | Phase::Checkpointing { .. }
            ) || (matches!(c.phase, Phase::Failed { .. })
                && c.resource().is_some()
                && c.boot().is_some_and(|b| !done(&b.resource_stop)));
            if matches!(c.phase, Phase::Stopping { .. }) {
                return Ok(Applied::Unchanged);
            }
            if !stoppable {
                return Err(wrong(c));
            }
            if let Phase::Checkpointing { .. } = c.phase {
                // An abandoned checkpoint is a definite non-result.
                let k = n.checkpoints.last_mut().unwrap();
                if !k.fact.is_settled() {
                    k.fact = Fact::Failed {
                        reason: "stopped before the checkpoint completed".into(),
                        at_ms: now,
                    };
                }
            }
            let why = if matches!(c.phase, Phase::Failed { .. }) {
                StopReason::FailedBoot
            } else {
                *why
            };
            n.boot_mut().ok_or(Refusal::NoResource)?.stop_reason = Some(why);
            n.phase = Phase::Stopping { reason: why };
            true
        }
        Command::ObserveShutdown { boot, outcome } => {
            current_boot(c, *boot)?;
            if !matches!(c.phase, Phase::Stopping { .. }) {
                return Err(wrong(c));
            }
            // An unknown shutdown does not block the resource stop, which
            // ends every process anyway; it stays recorded as unknown.
            n.boot_mut().unwrap().shutdown = Some(fact_of(outcome, Clone::clone, now));
            true
        }
        Command::ObserveStop { boot, outcome } => {
            current_boot(c, *boot)?;
            let b = c.boot().unwrap();
            let reconciling = matches!(c.phase, Phase::Unknown { .. })
                && b.resource_stop.as_ref().is_some_and(Fact::is_unknown);
            if !(matches!(c.phase, Phase::Stopping { .. }) || reconciling) {
                return Err(wrong(c));
            }
            let stop_reason = b.stop_reason;
            n.boot_mut().unwrap().resource_stop = Some(fact_of(outcome, Clone::clone, now));
            n.phase = match outcome {
                Outcome::Done { .. } => after_stop(c, stop_reason),
                Outcome::Failed { .. } => Phase::Stopping {
                    reason: stop_reason.unwrap_or(StopReason::Owner),
                },
                Outcome::Unknown { reason: r } => Phase::Unknown {
                    reason: format!("stop: {}", reason(r)),
                },
            };
            true
        }
        Command::ObserveMeter { boot, outcome } => {
            current_boot(c, *boot)?;
            if !done(&c.boot().unwrap().resource_stop) {
                return Err(Refusal::Invalid(
                    "Check the meter after the resource stops.",
                ));
            }
            let fact = match outcome {
                Outcome::Done { value } if value.running => Fact::Unknown {
                    reason: "the provider still reports a running meter after stop".into(),
                    at_ms: now,
                },
                other => fact_of(other, |m| m.evidence.clone(), now),
            };
            n.boot_mut().unwrap().meter_stop = Some(fact);
            true
        }
        Command::RequestDeleteResource => {
            let resource = c.resource().ok_or(Refusal::NoResource)?;
            let stopped = c.boot().is_none_or(|b| done(&b.resource_stop));
            let ok = stopped
                && match &c.phase {
                    Phase::Failed { .. } => c.latest_checkpoint().is_none(),
                    Phase::Deleting => true,
                    _ => false,
                };
            if !ok {
                return Err(wrong(c));
            }
            let a = n
                .creates
                .iter_mut()
                .rev()
                .find(|a| a.resource.as_deref() == Some(resource))
                .unwrap();
            if a.deletion.is_some() {
                return Ok(Applied::Unchanged);
            }
            a.deletion = Some(Fact::Requested {
                operation: Some(format!("{}-delete", a.operation)),
                at_ms: now,
            });
            true
        }
        Command::ObserveDeleteResource { operation, outcome } => {
            let Some(a) = n
                .creates
                .iter_mut()
                .find(|a| &a.operation == operation && a.deletion.is_some())
            else {
                return Err(Refusal::StaleOperation(operation.clone()));
            };
            if a.deletion.as_ref().is_some_and(Fact::is_done) {
                return Ok(Applied::Unchanged);
            }
            a.deletion = Some(fact_of(outcome, Clone::clone, now));
            match outcome {
                Outcome::Done { .. } if matches!(c.phase, Phase::Deleting) => {
                    n.deletion = Some(Fact::Done {
                        evidence: "resource deleted".into(),
                        at_ms: now,
                    });
                    n.phase = Phase::Deleted;
                }
                Outcome::Done { .. } if matches!(c.phase, Phase::Unknown { .. }) => {
                    n.phase = Phase::Failed {
                        reason: "boot failed; resource deleted".into(),
                    };
                }
                Outcome::Done { .. } | Outcome::Failed { .. } => {}
                Outcome::Unknown { reason: r } => {
                    if !matches!(c.phase, Phase::Deleting) {
                        n.phase = Phase::Unknown {
                            reason: format!("delete: {}", reason(r)),
                        };
                    }
                }
            }
            true
        }
        Command::RequestDelete => {
            let stopped = c.boot().is_none_or(|b| done(&b.resource_stop));
            let ok = match &c.phase {
                Phase::New => true,
                Phase::Stopped | Phase::Failed { .. } => stopped,
                Phase::Deleting => return Ok(Applied::Unchanged),
                _ => false,
            };
            if !ok {
                return Err(wrong(c));
            }
            if c.resource().is_none() {
                n.deletion = Some(Fact::Done {
                    evidence: "no provider resource".into(),
                    at_ms: now,
                });
                n.phase = Phase::Deleted;
            } else {
                n.deletion = Some(Fact::Requested {
                    operation: None,
                    at_ms: now,
                });
                n.phase = Phase::Deleting;
            }
            true
        }
        Command::ObserveInspection { outcome } => {
            if !matches!(c.phase, Phase::Unknown { .. }) {
                return Err(wrong(c));
            }
            let Outcome::Done { value: ins } = outcome else {
                // An unknown or failed read reconciles nothing.
                return Ok(Applied::Unchanged);
            };
            if !reconcile(&mut n, ins, now) {
                return Ok(Applied::Unchanged);
            }
            true
        }
    };
    debug_assert!(changed);
    n.revision += 1;
    n.updated_ms = now;
    Ok(Applied::Changed(Box::new(n)))
}

fn after_stop(c: &Computer, reason: Option<StopReason>) -> Phase {
    match reason {
        Some(StopReason::FailedBoot) if c.latest_checkpoint().is_none() => Phase::Failed {
            reason: "boot failed; resource stopped".into(),
        },
        _ => Phase::Stopped,
    }
}

/// Apply a definite provider view to whichever outcome is unknown. Returns
/// false when the view does not settle it, so it stays unknown.
fn reconcile(n: &mut Computer, ins: &Inspection, now: u64) -> bool {
    let gone = ins.running.is_none();
    if gone {
        // The provider reports no resource: nothing runs or meters.
        if let Some(b) = n.boot_mut() {
            for f in [&mut b.resource_stop, &mut b.meter_stop] {
                if !done(f) {
                    *f = Some(Fact::Done {
                        evidence: "provider reports the resource gone".into(),
                        at_ms: now,
                    });
                }
            }
            if b.restore.as_ref().is_some_and(Fact::is_unknown) {
                b.restore = Some(Fact::Failed {
                    reason: "provider reports the resource gone".into(),
                    at_ms: now,
                });
            }
        }
        if let Some(k) = n.checkpoints.last_mut().filter(|k| k.fact.is_unknown()) {
            k.fact = Fact::Failed {
                reason: "provider reports the resource gone".into(),
                at_ms: now,
            };
        }
        if let Some(a) = n.creates.iter_mut().rev().find(|a| a.resource.is_some()) {
            a.deletion = Some(Fact::Done {
                evidence: "provider reports the resource gone".into(),
                at_ms: now,
            });
        }
        n.phase = Phase::Failed {
            reason: "the provider resource is gone".into(),
        };
        return true;
    }
    let running = ins.running == Some(true);
    let stop = ins.stop.as_deref().filter(|_| !running);
    let b = n.boots.last().cloned().expect("a resource has a boot");

    if b.restore.as_ref().is_some_and(Fact::is_unknown) {
        let bm = n.boot_mut().unwrap();
        if running {
            bm.restore = Some(Fact::Done {
                evidence: "provider reports the resource running".into(),
                at_ms: now,
            });
            n.phase = Phase::Booting;
        } else {
            bm.restore = Some(Fact::Failed {
                reason: "provider reports the resource still stopped".into(),
                at_ms: now,
            });
            bm.resource_stop = Some(Fact::Done {
                evidence: "not resumed".into(),
                at_ms: now,
            });
            bm.shutdown = Some(Fact::Done {
                evidence: "not resumed".into(),
                at_ms: now,
            });
            n.phase = Phase::Stopped;
        }
        return true;
    }

    if let Some(k) = n
        .checkpoints
        .last()
        .filter(|k| k.fact.is_unknown())
        .cloned()
    {
        // Accept only a snapshot no earlier checkpoint used: nothing else ran
        // between the fenced turn and now, so a new one is this turn's.
        let fresh = ins.latest_snapshot.as_ref().filter(|s| {
            !n.checkpoints
                .iter()
                .any(|o| o.fact.evidence() == Some(s.as_str()))
        });
        if !running && stop.is_none() {
            return false;
        }
        let km = n.checkpoints.iter_mut().find(|o| o.id == k.id).unwrap();
        km.fact = match fresh {
            Some(s) => Fact::Done {
                evidence: s.clone(),
                at_ms: now,
            },
            None => Fact::Failed {
                reason: "no new snapshot exists for this turn".into(),
                at_ms: now,
            },
        };
        if let Some(stop) = stop {
            let bm = n.boot_mut().unwrap();
            bm.stop_reason.get_or_insert(StopReason::Checkpoint);
            bm.shutdown = Some(Fact::Done {
                evidence: format!("stopped with resource: {stop}"),
                at_ms: now,
            });
            bm.resource_stop = Some(Fact::Done {
                evidence: stop.into(),
                at_ms: now,
            });
            n.phase = Phase::Stopped;
        } else {
            n.phase = Phase::Awake;
            touch(n, now);
        }
        return true;
    }

    if b.resource_stop.as_ref().is_some_and(Fact::is_unknown) {
        if running {
            // Definitely not stopped: stop again.
            n.boot_mut().unwrap().resource_stop = Some(Fact::Failed {
                reason: "provider reports the resource still running".into(),
                at_ms: now,
            });
            n.phase = Phase::Stopping {
                reason: b.stop_reason.unwrap_or(StopReason::Owner),
            };
            return true;
        }
        let Some(stop) = stop else {
            // Not running, but no stop evidence: still unknown.
            return false;
        };
        n.boot_mut().unwrap().resource_stop = Some(Fact::Done {
            evidence: stop.into(),
            at_ms: now,
        });
        n.phase = after_stop(n, b.stop_reason);
        return true;
    }
    false
}
