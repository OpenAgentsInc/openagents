//! Pure lifecycle decisions over the retained record.
//!
//! [`decide`] answers "what is the one next effect?" for a prompt or a timer
//! tick. It never dispatches while a checkpoint is fenced, never replaces a
//! resource whose create outcome is unknown (it repeats the same idempotent
//! operation identity), and never treats an unknown stop as stopped.
//!
//! Liveness: a running turn whose owner stopped beating for
//! [`Computer::stale_ms`] stops as [`StopReason::Stale`] through the same
//! stop path as any other stop. Nothing bounds how long a live turn runs
//! besides the absolute bound the computer was admitted with.
//!
//! Circuit breaker: after [`BREAKER_FAILURES`] failed boots within
//! [`BREAKER_WINDOW_MS`], a prompt that would boot again is refused with a
//! plain message until the window passes.

use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// A queued prompt wants the computer.
    Prompt,
    /// A timer or observer visit; no prompt is waiting.
    Tick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitReason {
    /// The previous turn is still running; prompts are serialized.
    TurnRunning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Nothing to do now.
    Skip,
    Wait(WaitReason),
    Refuse(String),
    /// Create (or reconcile, with the same identity) the provider resource.
    Create {
        operation: String,
    },
    /// Resume the retained stopped resource, naming its latest checkpoint.
    Restore {
        checkpoint: Option<String>,
    },
    ApplyCredentials {
        boot: u32,
    },
    StartService {
        boot: u32,
        name: String,
    },
    /// Dispatch the next queued prompt as this turn generation.
    Dispatch {
        generation: u64,
    },
    /// Checkpoint exactly this completed turn.
    Checkpoint {
        generation: u64,
    },
    /// Begin stopping for this reason.
    Stop {
        reason: StopReason,
    },
    ShutdownProcesses {
        boot: u32,
    },
    StopResource {
        boot: u32,
    },
    CheckMeter {
        boot: u32,
    },
    /// Delete the resource made by this create operation.
    DeleteResource {
        operation: String,
    },
    /// Read the provider's view to reconcile an unknown outcome.
    Inspect,
}

/// The refusal while the breaker is open, in plain words.
pub fn breaker_message(c: &Computer, now_ms: u64) -> Option<String> {
    let until = c.breaker_open_until(now_ms)?;
    let minutes = until.saturating_sub(now_ms).div_ceil(60_000).max(1);
    Some(format!(
        "This computer failed to start {BREAKER_FAILURES} times in the last {} minutes, so it \
         isn't trying again yet. It can try again in {minutes} minute{}.",
        BREAKER_WINDOW_MS / 60_000,
        if minutes == 1 { "" } else { "s" }
    ))
}

fn create_operation(c: &Computer) -> String {
    format!("{}-create-{}", c.id, c.creates.len() + 1)
}
fn live_create(c: &Computer) -> Option<&CreateAttempt> {
    let r = c.resource()?;
    c.creates
        .iter()
        .rev()
        .find(|a| a.resource.as_deref() == Some(r))
}

/// The next lifecycle effect for `trigger` at `now_ms`.
pub fn decide(c: &Computer, trigger: Trigger, now_ms: u64) -> Decision {
    let prompt = trigger == Trigger::Prompt;
    let boot = c.boot();
    let absolute_passed = boot.is_some_and(|b| now_ms >= b.absolute_deadline_ms);
    match &c.phase {
        Phase::Deleted => Decision::Refuse("The computer was deleted.".into()),
        Phase::New if prompt => Decision::Create {
            operation: create_operation(c),
        },
        Phase::New => Decision::Skip,
        // The request may or may not have reached the provider: repeat the
        // same identity, which returns the same resource.
        Phase::Creating => Decision::Create {
            operation: c.creates.last().expect("creating").operation.clone(),
        },
        Phase::Booting => {
            let b = boot.expect("booting has a boot");
            if absolute_passed {
                return Decision::Stop {
                    reason: StopReason::Absolute,
                };
            }
            if b.restore.as_ref().is_some_and(|f| !f.is_done()) {
                let BootOrigin::Restored { checkpoint } = &b.origin else {
                    unreachable!("only restored boots restore")
                };
                return Decision::Restore {
                    checkpoint: checkpoint.clone(),
                };
            }
            if !done(&b.credentials) {
                return Decision::ApplyCredentials { boot: b.number };
            }
            match c
                .services
                .iter()
                .find(|s| !b.services.contains_key(&s.name))
            {
                Some(s) => Decision::StartService {
                    boot: b.number,
                    name: s.name.clone(),
                },
                None => Decision::Skip,
            }
        }
        Phase::Awake => {
            let b = boot.expect("awake has a boot");
            if absolute_passed {
                Decision::Stop {
                    reason: StopReason::Absolute,
                }
            } else if prompt {
                Decision::Dispatch {
                    generation: c.turn.dispatched + 1,
                }
            } else if now_ms >= b.idle_deadline_ms {
                Decision::Stop {
                    reason: StopReason::Idle,
                }
            } else {
                Decision::Skip
            }
        }
        // Idle never fires during a turn; the absolute bound still does.
        Phase::Turn { .. } if absolute_passed => Decision::Stop {
            reason: StopReason::Absolute,
        },
        // A turn whose owner went silent ends; a live one runs on.
        Phase::Turn { .. } if c.turn_silent(now_ms) => Decision::Stop {
            reason: StopReason::Stale,
        },
        Phase::Turn { .. } if prompt => Decision::Wait(WaitReason::TurnRunning),
        Phase::Turn { .. } => Decision::Skip,
        // The fence: a queued prompt first finishes this turn's checkpoint.
        Phase::Checkpointing { generation } => Decision::Checkpoint {
            generation: *generation,
        },
        Phase::Stopping { .. } => {
            let b = boot.expect("stopping has a boot");
            if b.shutdown.is_none() {
                Decision::ShutdownProcesses { boot: b.number }
            } else if !done(&b.resource_stop) {
                Decision::StopResource { boot: b.number }
            } else {
                Decision::Skip
            }
        }
        Phase::Stopped => {
            let b = boot.expect("stopped has a boot");
            if prompt && let Some(message) = breaker_message(c, now_ms) {
                Decision::Refuse(message)
            } else if prompt {
                Decision::Restore {
                    checkpoint: c.latest_checkpoint().map(|k| k.id.clone()),
                }
            } else if !done(&b.meter_stop) {
                Decision::CheckMeter { boot: b.number }
            } else {
                Decision::Skip
            }
        }
        Phase::Unknown { .. } => {
            if let Some(a) = c.creates.last().filter(|a| a.fact.is_unknown()) {
                return Decision::Create {
                    operation: a.operation.clone(),
                };
            }
            if let Some(a) = c
                .creates
                .iter()
                .rev()
                .find(|a| a.deletion.as_ref().is_some_and(Fact::is_unknown))
            {
                return Decision::DeleteResource {
                    operation: a.operation.clone(),
                };
            }
            Decision::Inspect
        }
        Phase::Failed { .. } => {
            let Some(a) = live_create(c) else {
                if prompt && let Some(message) = breaker_message(c, now_ms) {
                    return Decision::Refuse(message);
                }
                return if prompt {
                    Decision::Create {
                        operation: create_operation(c),
                    }
                } else {
                    Decision::Skip
                };
            };
            let b = boot.expect("a resource has a boot");
            if !done(&b.resource_stop) {
                Decision::Stop {
                    reason: StopReason::FailedBoot,
                }
            } else if !done(&b.meter_stop) {
                Decision::CheckMeter { boot: b.number }
            } else if c.latest_checkpoint().is_none() {
                // A failed fresh boot holds nothing worth keeping.
                Decision::DeleteResource {
                    operation: a.operation.clone(),
                }
            } else {
                Decision::Skip
            }
        }
        Phase::Deleting => match live_create(c) {
            Some(a) => Decision::DeleteResource {
                operation: a.operation.clone(),
            },
            None => Decision::Skip,
        },
    }
}
