//! The host's side of her steering loop (#10800, [`agent_steer`]): Coder
//! V1 turns in her plain Coder session, `NAME-coder`, through
//! [`super::coder_v1`].
//!
//! Coder's events drive her: thinking until Coder runs something, running
//! or testing while a command runs (her walk to the console), and waiting
//! at the lectern while an approval her policy escalates waits for the
//! owner's CONFIRM or REJECT. Her policy confirms routine approvals and
//! refuses what she never does. Each command Coder ran, each approval and
//! its answer, a takeover, and the report go to her journal, from Coder's
//! events and never from the model's words. Her pane follows the session,
//! where her prompts are the user turns.
//!
//! [`agent_steer`]: super::super::agent_steer

use super::*;
use crate::task::agent_steer::{self, Answer, Hands, Places, Policy, TurnEnd, Turned};

/// How long she waits for a turn running in her session before she gives
/// up on a request.
const RECLAIM_LIMIT: Duration = Duration::from_secs(15 * 60);

/// Whether Coder refused a turn because another process holds the session.
fn held(why: &str) -> bool {
    why.contains("Another process is using this chat session")
}

fn command_text(input: &serde_json::Value) -> String {
    input
        .as_str()
        .map(str::to_owned)
        .or_else(|| {
            input
                .get("command")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| input.to_string())
}

/// One request's hold on Coder: her engine, her session, her pane, and
/// her policy.
struct HostHands<'a> {
    agents: &'a Agents,
    store: &'a Store,
    record: &'a Record,
    queued: &'a Queued,
    name: String,
    cwd: String,
    state: PathBuf,
    session: String,
    cancel: Arc<AtomicBool>,
    policy: Policy,
    places: Places,
    engine: Option<Box<dyn coder_v1::Engine>>,
    /// Her turns stop on the kill switch or the owner's takeover.
    stop: Arc<AtomicBool>,
    took: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    watcher: Option<std::thread::JoinHandle<()>>,
    /// The pane's step, once her pane follows the session.
    step: Option<u64>,
}

impl HostHands<'_> {
    fn write(&self, kind: Kind, line: &str, status: Option<i32>) -> Result<(), String> {
        let mut entry = Entry::new((self.agents.clock)(), kind, line);
        entry.status = status;
        self.store.append(&entry)
    }

    /// Her pane follows her session from her first prompt to the report.
    fn open_pane(&mut self) {
        if self.watcher.is_some() {
            return;
        }
        let name = self.name.clone();
        let mut pane = None;
        if self.queued.typist {
            let (reply, answer) = mpsc::channel();
            let argv = coder_v1::tui_binary()
                .map(|tui| coder_v1::follow_command(&tui, &self.session, &self.state, &self.cwd))
                .unwrap_or_default();
            let (session, cwd) = (self.session.clone(), self.cwd.clone());
            self.step = Some(self.agents.with_live(&name, |live| {
                live.step += 1;
                live.run = Some((
                    wire::Step {
                        step: live.step,
                        command: format!("Coder V1 session {session}"),
                        typist: true,
                        cwd,
                        coder: Some(wire::CoderPane { session, argv }),
                    },
                    reply,
                ));
                live.step
            }));
            pane = Some(answer);
        }
        let (stop, took, done, cancel) = (
            self.stop.clone(),
            self.took.clone(),
            self.done.clone(),
            self.cancel.clone(),
        );
        self.watcher = Some(std::thread::spawn(move || {
            let mut pane = pane;
            while !done.load(Ordering::SeqCst) {
                if cancel.load(Ordering::SeqCst) {
                    stop.store(true, Ordering::SeqCst);
                }
                match &pane {
                    Some(answer) => match answer.recv_timeout(Duration::from_millis(200)) {
                        Ok(ran) if ran.taken_back => {
                            took.store(true, Ordering::SeqCst);
                            stop.store(true, Ordering::SeqCst);
                        }
                        // A closed pane leaves Coder working.
                        Ok(_) | Err(RecvTimeoutError::Disconnected) => pane = None,
                        Err(RecvTimeoutError::Timeout) => {}
                    },
                    None => std::thread::sleep(Duration::from_millis(200)),
                }
            }
        }));
    }

    /// Lets go of her pane when the request ends.
    fn close(&mut self) {
        self.done.store(true, Ordering::SeqCst);
        if let Some(watcher) = self.watcher.take() {
            let _ = watcher.join();
        }
        let step = self.step;
        self.agents.with_live(&self.name, |live| {
            if step.is_some() && live.run.as_ref().map(|(s, _)| s.step) == step {
                live.run = None;
            }
            live.pending = None;
        });
    }

    /// The end of a turn the kill switch or the owner stopped.
    fn stopped(&self) -> TurnEnd {
        if self.took.load(Ordering::SeqCst) {
            let _ = self.write(
                Kind::Takeback,
                &format!("Coder session {}", self.session),
                None,
            );
            TurnEnd::TakenOver
        } else {
            TurnEnd::Stopped
        }
    }
}

impl Hands for HostHands<'_> {
    fn say(&mut self, line: &str) {
        self.agents.say(&self.name, line);
    }

    fn journal(&mut self, kind: Kind, text: &str, status: Option<i32>) {
        let _ = self.write(kind, text, status);
        if kind == Kind::Failed {
            self.agents.set_doing(&self.name, Doing::Failed);
        }
    }

    fn coder(&mut self, prompt: &str) -> Turned {
        if self.stop.load(Ordering::SeqCst) || self.cancel.load(Ordering::SeqCst) {
            return Turned::ended(self.stopped());
        }
        let name = self.name.clone();
        let mut engine = match self.engine.take() {
            Some(engine) => engine,
            None => match (self.agents.engine)(self.record) {
                Ok((engine, standing)) => {
                    self.agents
                        .with_live(&name, |live| live.model = standing.clone());
                    engine
                }
                Err(why) => return Turned::ended(TurnEnd::NoCoder(why)),
            },
        };
        // The owner asked her, so she takes her session back from her
        // pane, or waits for a turn running in it, before each prompt.
        let (store, clock) = (self.store, self.agents.clock);
        let journal = move |kind: Kind, line: &str, status: Option<i32>| {
            let mut entry = Entry::new(clock(), kind, line);
            entry.status = status;
            store.append(&entry)
        };
        if let Err(why) =
            self.agents
                .take_back(&name, &self.state, &self.session, &self.cancel, &journal)
        {
            self.engine = Some(engine);
            return Turned::ended(match why {
                coder_v1::Unreclaimed::Stopped => TurnEnd::Stopped,
                coder_v1::Unreclaimed::Busy => {
                    TurnEnd::Busy("her Coder session stayed held".into())
                }
            });
        }
        self.open_pane();
        let turn = coder_v1::Turn {
            cwd: PathBuf::from(&self.cwd),
            state: self.state.clone(),
            session: self.session.clone(),
            prompt: prompt.to_owned(),
            // Plain Coder: nothing in her Coder session says who she is.
            instructions: None,
            approvals: true,
        };
        self.agents.set_doing(&name, Doing::Thinking);
        let mut turned = Turned::ended(TurnEnd::Stopped);
        let mut previous: Option<CoderEvent> = None;
        let agents = self.agents;
        let (policy, places, stop) = (&self.policy, &self.places, self.stop.clone());
        let cwd = PathBuf::from(&self.cwd);
        let ended = {
            let mut hear = |event: &CoderEvent| -> Option<bool> {
                match event {
                    CoderEvent::Model { model } if !model.is_empty() => {
                        agents.with_live(&name, |live| live.model = format!("Coder V1 ({model})"));
                        None
                    }
                    CoderEvent::Tool { .. } if previous.as_ref() == Some(event) => None,
                    CoderEvent::Tool {
                        name: tool,
                        input,
                        output,
                        running,
                        ..
                    } => {
                        previous = Some(event.clone());
                        if !tool.eq_ignore_ascii_case("run") {
                            if *running {
                                agents.say(&name, &format!("{name}: Coder is using {tool}"));
                            }
                            return None;
                        }
                        let command = agent::plain(&command_text(input));
                        let answered_no =
                            turned.rejected.contains(&command) || turned.never.contains(&command);
                        if *running && answered_no {
                            // Rejected; it does not run.
                            return None;
                        }
                        if *running {
                            agents.set_doing(
                                &name,
                                if agent::testing(&command) {
                                    Doing::Testing
                                } else {
                                    Doing::Running
                                },
                            );
                            agents.say(&name, &format!("{name}: $ {command}"));
                            return None;
                        }
                        let status = output
                            .get("exit")
                            .and_then(serde_json::Value::as_i64)
                            .and_then(|code| i32::try_from(code).ok());
                        let said = output
                            .get("output")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default();
                        match status {
                            Some(status) => {
                                turned.ran.push((command.clone(), Some(status)));
                                let _ = journal(
                                    Kind::Ran,
                                    &format!("{command} ({} bytes of output)", said.len()),
                                    Some(status),
                                );
                                agents.say(&name, &format!("{name}: exit {status}"));
                            }
                            None if said.starts_with("The host refuses") => {
                                turned.refused.push(command.clone());
                                let _ = journal(Kind::Refused, &command, None);
                                agents.say(&name, &format!("{name}: refused: {command}"));
                            }
                            None if said.starts_with("The owner rejected") => {}
                            None => {
                                turned.ran.push((command.clone(), None));
                                let _ =
                                    journal(Kind::Ran, &format!("{command}: did not finish"), None);
                            }
                        }
                        agents.set_doing(&name, Doing::Thinking);
                        None
                    }
                    CoderEvent::Approval { command, why, .. } => {
                        let command = agent::plain(command);
                        let confirm = match policy.answer("run", &command, &cwd, places) {
                            Answer::Never(what) => {
                                turned.never.push(command.clone());
                                let _ = journal(
                                    Kind::Rejected,
                                    &format!("{command} (I never {what})"),
                                    None,
                                );
                                agents.say(
                                    &name,
                                    &format!("{name}: I said no to {command}: I never {what}."),
                                );
                                false
                            }
                            Answer::Confirm(rule) => {
                                let _ = journal(
                                    Kind::Confirmed,
                                    &format!("{command} (by her policy: {rule})"),
                                    None,
                                );
                                agents.say(
                                    &name,
                                    &format!("{name}: confirmed {command} under my policy"),
                                );
                                true
                            }
                            Answer::Escalate => {
                                agents.set_doing(&name, Doing::Waiting);
                                let _ =
                                    journal(Kind::Proposed, &format!("{command} ({why})"), None);
                                agents.say(&name, &format!("{name}: proposed: {command}"));
                                let reason = if why.trim().is_empty() {
                                    "Coder needs it for this step".to_string()
                                } else {
                                    agent::plain(why)
                                };
                                let decided = agents.propose(&name, &command, &reason, &stop);
                                if decided == Decision::Confirm {
                                    let _ = journal(Kind::Confirmed, &command, None);
                                    true
                                } else {
                                    turned.rejected.push(command.clone());
                                    let _ = journal(Kind::Rejected, &command, None);
                                    agents.say(&name, &format!("{name}: rejected: {command}"));
                                    false
                                }
                            }
                        };
                        agents.set_doing(
                            &name,
                            if confirm {
                                Doing::Running
                            } else {
                                Doing::Thinking
                            },
                        );
                        Some(confirm)
                    }
                    _ => None,
                }
            };
            let mut ended = engine.turn(&turn, &stop, &mut hear);
            // Something took the session between her reclaim and her
            // turn: take it back once more.
            if matches!(&ended, Ended::Failed(why) if held(why))
                && coder_v1::reclaim(&self.state, &self.session, &stop, RECLAIM_LIMIT, || {})
                    .is_ok()
            {
                ended = engine.turn(&turn, &stop, &mut hear);
            }
            ended
        };
        self.engine = Some(engine);
        turned.end = match ended {
            Ended::Finished { reply, .. } => TurnEnd::Finished(reply),
            Ended::Cancelled => self.stopped(),
            Ended::Failed(why) if held(&why) => TurnEnd::Busy(why),
            Ended::Failed(why) => TurnEnd::Failed(why),
        };
        turned
    }
}

impl Agents {
    /// Runs `text` for her to its report: she plans, steers Coder in her
    /// Coder session in `cwd`, judges, follows up, and reports
    /// ([`agent_steer::run`]).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn coder_turn(
        &self,
        store: &Store,
        record: &Record,
        queued: &Queued,
        cwd: &str,
        text: &str,
        briefing: &str,
        carried: &[crate::task::agent_recall::Ref],
    ) -> Report {
        let name = record.name.clone();
        let clock = self.clock;
        let journal = |kind: Kind, line: &str, status: Option<i32>| {
            let mut entry = Entry::new(clock(), kind, line);
            entry.status = status;
            if kind == Kind::Request {
                entry.from = Some(queued.from.clone());
            }
            store.append(&entry)
        };
        // Her reply is one plain sentence; the cause goes to her journal.
        let fail = |reply: String, why: &str, headline: &str| {
            let line = if why.is_empty() {
                reply.clone()
            } else {
                format!("{reply} ({})", agent::plain(why))
            };
            let _ = journal(Kind::Failed, &line, None);
            self.set_doing(&name, Doing::Failed);
            Report {
                outcome: Outcome::Failed,
                reply,
                headline: headline.into(),
            }
        };
        let request = text.trim();
        if request.is_empty() {
            return fail("I need a request to work on.".into(), "", "no request");
        }
        if let Err(why) = journal(Kind::Request, request, None) {
            return fail(
                "I couldn't write my journal, so I didn't start.".into(),
                &why.to_string(),
                "no journal",
            );
        }
        // Recall: her core profile and the scored briefing, or nothing at
        // all when her engram store can't be read.
        let opened = crate::task::agent_engrams::EngramStore::read(store, &self.screen);
        let (core, briefing, note) = match opened.carried_core() {
            Ok(core) => {
                if let Some(receipt) = crate::task::agent_recall::receipt(carried) {
                    let _ = journal(Kind::Memory, &receipt, None);
                }
                (core.map(str::to_owned), briefing.to_string(), None)
            }
            Err(why) => {
                let _ = journal(
                    Kind::Memory,
                    &format!(
                        "my engram store can't be read ({}), so I carried nothing from memory",
                        agent::plain(why)
                    ),
                    None,
                );
                (
                    None,
                    String::new(),
                    Some("I couldn't read my memory store, so I worked without my memory."),
                )
            }
        };
        let policy = Policy::load(store).unwrap_or_else(|why| {
            let _ = journal(
                Kind::Control,
                &format!(
                    "my approval policy can't be read ({}), so every approval goes to the owner",
                    agent::plain(&why)
                ),
                None,
            );
            Policy::escalate_all()
        });
        let mut mind = match (self.mind)(record) {
            Ok(mind) => mind,
            Err(why) => {
                return fail(
                    "I couldn't reach a model to plan with, so I didn't start.".into(),
                    &why,
                    "no model",
                );
            }
        };
        let cancel = self
            .lock()
            .live
            .get(&name)
            .map(|l| l.cancel.clone())
            .unwrap_or_default();
        let state = self
            .coder_state
            .clone()
            .or_else(coder_v1::default_state)
            .unwrap_or_else(|| self.root.join("coder-new"));
        self.set_doing(&name, Doing::Thinking);
        let mut hands = HostHands {
            agents: self,
            store,
            record,
            queued,
            name: name.clone(),
            cwd: cwd.to_string(),
            state,
            session: coder_v1::coder_session_for(&name),
            cancel,
            places: Places::on_host(&self.root, cwd),
            policy: policy.clone(),
            engine: None,
            stop: Arc::new(AtomicBool::new(false)),
            took: Arc::new(AtomicBool::new(false)),
            done: Arc::new(AtomicBool::new(false)),
            watcher: None,
            step: None,
        };
        let input = agent_steer::Input {
            record,
            request,
            briefing: &briefing,
            core: core.as_deref(),
            cwd,
            policy: &policy,
            note,
        };
        let steered = agent_steer::run(&mut hands, &mut mind, &input);
        hands.close();
        steered.report
    }

    /// Takes her Coder session back from whatever holds it: her pane after
    /// the owner typed in it, or a process that quit. When a turn runs
    /// there, she says so once and runs this request after it.
    fn take_back(
        &self,
        name: &str,
        state: &Path,
        session: &str,
        cancel: &AtomicBool,
        journal: &dyn Fn(Kind, &str, Option<i32>) -> Result<(), String>,
    ) -> Result<(), coder_v1::Unreclaimed> {
        if !coder_v1::session_held(state, session) {
            return Ok(());
        }
        let _ = journal(
            Kind::Control,
            &format!("took Coder session {session} back from its holder for this request"),
            None,
        );
        let queued = || {
            self.set_doing(name, Doing::Thinking);
            self.say(
                name,
                &format!("{name}: I'll run this as soon as the turn in my session finishes."),
            );
        };
        coder_v1::reclaim(state, session, cancel, RECLAIM_LIMIT, queued)
    }

    /// Holds `command` for the owner's CONFIRM or REJECT at her lectern,
    /// until they answer, `stop`, or [`DECISION_LIMIT`].
    fn propose(&self, name: &str, command: &str, why: &str, stop: &AtomicBool) -> Decision {
        if stop.load(Ordering::SeqCst) {
            return Decision::Reject;
        }
        let (answer, decision) = mpsc::channel();
        self.with_live(name, |live| {
            live.step += 1;
            live.pending = Some((
                wire::Proposal {
                    step: live.step,
                    command: command.to_string(),
                    why: why.to_string(),
                },
                answer,
            ));
        });
        let decided = wait(&decision, stop, DECISION_LIMIT);
        self.with_live(name, |live| live.pending = None);
        decided.unwrap_or(Decision::Reject)
    }
}

#[cfg(test)]
#[path = "agent_coder_tests.rs"]
mod tests;
