//! Host-owned proposals. Terminal input and decisions share the terminal lock.

use super::*;
use crate::proposal::{self, Action, Effect, Entry, Page, Request, State as Phase};

pub(super) struct Stored {
    pub entry: Entry,
    head: u64,
    epoch: u64,
    before: u64,
}

impl Host {
    /// Lists, offers, or decides an exact proposal on the terminal owner.
    /// A share grants no proposal authority. An admitted input is never replayed.
    pub fn proposal(&self, principal: &str, request: &Request) -> Outcome {
        request.check(self.features())?;
        self.inner.require(principal, Right::Terminal)?;
        let terminal = if matches!(request.action, Action::Read { .. }) {
            self.inner.find(&request.terminal)?
        } else {
            self.inner.running(&request.terminal)?
        };
        let mut state = terminal.state();
        self.inner.require(principal, Right::Terminal)?;
        let body = identity(principal, request);
        if let Some(outcome) = self.inner.retry(&key(principal, &request.request), &body) {
            return outcome;
        }
        // Reconcile only block records that began after admission.
        if let Some(Ok(page)) = state.emulator.as_ref().and_then(|e| e.blocks(None, 32)) {
            for stored in state.proposals.values_mut() {
                if stored.entry.state == Phase::Executing {
                    if let Some(block) = page.blocks.iter().find(|block| {
                        block.block > stored.before
                            && block.command == stored.entry.proposal.command
                            && block.state == crate::ext::BlockState::Finished
                    }) {
                        stored.entry.state = Phase::Completed { block: block.block };
                    }
                }
            }
        }
        let value = match &request.action {
            Action::Read { limit } => {
                let mut entries = Vec::new();
                let mut candidates: Vec<_> = state.proposals.values().collect();
                candidates.sort_by_key(|stored| {
                    !matches!(stored.entry.state, Phase::Pending | Phase::Warned { .. })
                });
                for stored in candidates {
                    if entries.len() >= usize::from(*limit) {
                        break;
                    }
                    entries.push(stored.entry.clone());
                    if serde_json::to_vec(&entries).map_or(true, |bytes| bytes.len() > 10 * 1024) {
                        entries.pop();
                        break;
                    }
                }
                Value::Proposals {
                    page: Page {
                        more: entries.len() < state.proposals.len(),
                        entries,
                    },
                }
            }
            Action::Offer { proposal } => {
                if serde_json::to_vec(proposal).map_or(true, |bytes| bytes.len() > 8 * 1024) {
                    return Err(Refusal::new(
                        Reason::LimitExceeded,
                        "proposal exceeds the host bound",
                    ));
                }
                let proposal_key = proposal.key();
                if let Some(stored) = state.proposals.get(&proposal_key) {
                    if stored.entry.proposal != *proposal {
                        return Err(Refusal::new(
                            Reason::IdempotencyConflict,
                            "proposal revision changed",
                        ));
                    }
                } else {
                    if state.proposals.len() >= 64 {
                        return Err(Refusal::new(
                            Reason::LimitExceeded,
                            "terminal proposal capacity reached",
                        ));
                    }
                    if state.proposals.values().any(|s| {
                        s.entry.proposal.thread == proposal.thread
                            && s.entry.proposal.id == proposal.id
                            && s.entry.proposal.revision >= proposal.revision
                    }) {
                        return Err(stale());
                    }
                    check_prompt(&terminal, &state, &proposal.binding)?;
                    let head = state.ring.head();
                    let before = state
                        .emulator
                        .as_ref()
                        .and_then(|e| e.blocks(None, 1))
                        .and_then(Result::ok)
                        .and_then(|p| p.blocks.first().map(|b| b.block))
                        .unwrap_or(0);
                    for old in state.proposals.values_mut().filter(|s| {
                        s.entry.proposal.thread == proposal.thread
                            && s.entry.proposal.id == proposal.id
                            && matches!(s.entry.state, Phase::Pending | Phase::Warned { .. })
                    }) {
                        old.entry.state = Phase::Rejected;
                    }
                    let epoch = state.input_epoch;
                    state.proposals.insert(proposal_key.clone(), Stored { entry: Entry { proposal: proposal.clone(), effect: Effect::Destructive("This command may change this computer or publish data. Approve again to run it.".into()), state: Phase::Pending }, head, epoch, before });
                }
                Value::Proposals {
                    page: Page {
                        entries: vec![state.proposals[&proposal_key].entry.clone()],
                        more: false,
                    },
                }
            }
            Action::Decide {
                thread,
                proposal,
                revision,
                approve,
                attachment,
            } => {
                let latest = state
                    .proposals
                    .values()
                    .filter(|s| {
                        s.entry.proposal.thread == *thread && s.entry.proposal.id == *proposal
                    })
                    .map(|s| s.entry.proposal.revision)
                    .max()
                    .ok_or_else(stale)?;
                if latest != *revision {
                    return Err(stale());
                }
                let proposal_key = state
                    .proposals
                    .iter()
                    .find(|(_, s)| {
                        s.entry.proposal.thread == *thread
                            && s.entry.proposal.id == *proposal
                            && s.entry.proposal.revision == *revision
                    })
                    .map(|(key, _)| key.clone())
                    .ok_or_else(stale)?;
                // An interact attachment is required even to reject. A watch
                // screen with the terminal right cannot approve through observe.
                if !state
                    .attachments
                    .get(attachment)
                    .is_some_and(|a| a.principal == principal && a.mode == Mode::Interact)
                {
                    return Err(Refusal::new(
                        Reason::NotAdmitted,
                        "proposal decisions need this device's interact attachment",
                    ));
                }
                let stored = &state.proposals[&proposal_key];
                if !matches!(stored.entry.state, Phase::Pending | Phase::Warned { .. }) {
                    return Err(stale());
                }
                let binding = stored.entry.proposal.binding.clone();
                if *approve {
                    if state.ring.head() != stored.head || state.input_epoch != stored.epoch {
                        return Err(stale());
                    }
                    check_prompt(&terminal, &state, &binding)?;
                    // Reuse the same typist admission as ordinary input.
                    state.seat(principal, Some(attachment), true, true)?;
                    let stored = state
                        .proposals
                        .get_mut(&proposal_key)
                        .expect("found proposal");
                    let previous = match &stored.entry.state {
                        Phase::Warned { nonce } => Some(nonce.as_str()),
                        _ => None,
                    };
                    match proposal::admit(
                        &stored.entry.proposal,
                        &binding,
                        principal,
                        &request.request,
                        true,
                        previous,
                        stored.entry.effect.clone(),
                    )
                    .map_err(|why| Refusal::new(Reason::NotAdmitted, why))?
                    {
                        Some(_) => {
                            stored.entry.state = Phase::Warned {
                                nonce: request.request.clone(),
                            }
                        }
                        None => {
                            // Mark before the write. Any partial write or error is
                            // uncertain, and no later decision can replay it.
                            stored.entry.state = Phase::Uncertain;
                            let mut bytes = stored.entry.proposal.command.as_bytes().to_vec();
                            bytes.push(b'\r');
                            if terminal.process.write(&bytes).ok() == Some(bytes.len()) {
                                stored.entry.state = Phase::Executing;
                            }
                            state.activity = Instant::now();
                        }
                    }
                    if matches!(
                        state.proposals[&proposal_key].entry.state,
                        Phase::Executing | Phase::Uncertain
                    ) {
                        state.input_epoch = state.input_epoch.saturating_add(1);
                        state.last_input = Some(state.ring.head());
                    }
                } else {
                    state
                        .proposals
                        .get_mut(&proposal_key)
                        .expect("found proposal")
                        .entry
                        .state = Phase::Rejected;
                }
                let entry = state.proposals[&proposal_key].entry.clone();
                terminal.pump(&mut state, Instant::now());
                Value::Proposals {
                    page: Page {
                        entries: vec![entry],
                        more: false,
                    },
                }
            }
        };
        drop(state);
        self.inner
            .remember(&key(principal, &request.request), body, value.clone());
        Ok((Status::Accepted, value))
    }
}
fn stale() -> Refusal {
    Refusal::new(
        Reason::Stale,
        "proposal revision, disposition, or terminal context changed",
    )
}
fn check_prompt(
    terminal: &Terminal,
    state: &super::State,
    binding: &proposal::Binding,
) -> Result<(), Refusal> {
    if !state.emulator.as_ref().is_some_and(|e| {
        e.empty_prompt()
            && state
                .last_input
                .is_none_or(|input| e.prompt_through().is_some_and(|prompt| prompt > input))
    }) {
        return Err(stale());
    }
    let cwd = sys::process_cwd(terminal.process.group()).ok_or_else(|| {
        Refusal::new(
            Reason::Unavailable,
            "the host cannot inspect the shell directory",
        )
    })?;
    if cwd != binding.cwd || state.directory != binding.shell_directory {
        return Err(stale());
    }
    Ok(())
}
