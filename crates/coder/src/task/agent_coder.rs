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
use crate::task::agent_spend::{self, Meter};
use crate::task::agent_steer::{self, Answer, Delegated, Hands, Places, Policy, TurnEnd, Turned};
use crate::task::capacity;

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
    /// Her spend records and budgets for this request.
    meter: Meter,
    /// Coder delegates her coding to Codex: her engine is Codex, and the
    /// capacity book gives Codex capacity.
    codex: bool,
}

/// What she adds to each prompt when Codex does her coding: Coder, on its
/// own model, hands the coding to Codex and checks what it did.
pub(crate) const CODEX_DIRECTIVE: &str = "Delegate the coding in this step to the codex agent \
     with acp_subagent: give it the task, the files involved, and how to check the result. \
     Then read its changes and run the checks yourself before you answer. If Codex is \
     unavailable or out of capacity, do the work yourself and say so.";

/// The signed-in Codex login a book entry is kept for; a unit test reads
/// no login.
fn login(provider: capacity::Provider) -> Option<String> {
    if cfg!(test) {
        None
    } else {
        microcoder_loop::account::identify(provider)
    }
}

/// Her one sentence when Codex has no capacity, from the capacity book or
/// a refusal now.
fn codex_out(name: &str, until: Option<u64>) -> String {
    match until {
        Some(at) => format!(
            "{name}: Codex is out of capacity until {}, so Coder works on its own model.",
            capacity::utc(at)
        ),
        None => format!("{name}: Codex is out of capacity, so Coder works on its own model."),
    }
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

    fn spent(&mut self, call: &agent_spend::Call) -> Option<String> {
        match self.meter.record(call, (self.agents.clock)()) {
            Ok(used) => used,
            Err(why) => {
                // The call still counts against her budgets.
                let _ = self.write(
                    Kind::Control,
                    &format!("my spend record wasn't kept ({})", agent::plain(&why)),
                    None,
                );
                self.meter.stop()
            }
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
                coder_v1::Unreclaimed::Busy => TurnEnd::Busy(format!(
                    "{} Coder session stayed held",
                    self.record.refer().their()
                )),
            });
        }
        self.open_pane();
        let codex = self.codex;
        let turn = coder_v1::Turn {
            cwd: PathBuf::from(&self.cwd),
            state: self.state.clone(),
            session: self.session.clone(),
            // Her words, and on Codex the one line that hands the coding
            // to it; the session shows both.
            prompt: if codex {
                format!("{prompt}\n\n{CODEX_DIRECTIVE}")
            } else {
                prompt.to_owned()
            },
            // Plain Coder: nothing in her Coder session says who she is.
            instructions: None,
            approvals: true,
            codex_writes: codex,
        };
        if codex {
            self.agents.with_live(&name, |live| {
                live.model = "Coder V1, coding on Codex".into()
            });
        }
        self.agents.set_doing(&name, Doing::Thinking);
        let mut turned = Turned::ended(TurnEnd::Stopped);
        let mut previous: Option<CoderEvent> = None;
        let mut ended_delegations: Vec<String> = Vec::new();
        let mut codex_refused: Option<String> = None;
        let agents = self.agents;
        let (policy, places, stop) = (&self.policy, &self.places, self.stop.clone());
        let cwd = PathBuf::from(&self.cwd);
        let their = self.record.refer().their().to_string();
        let ended = {
            let mut hear = |event: &CoderEvent| -> Option<bool> {
                match event {
                    CoderEvent::Model { model } if !model.is_empty() => {
                        agents.with_live(&name, |live| {
                            live.model = if codex {
                                format!("Coder V1 ({model}), coding on Codex")
                            } else {
                                format!("Coder V1 ({model})")
                            };
                        });
                        turned.model = Some(model.clone());
                        None
                    }
                    CoderEvent::Delegation {
                        id,
                        agent: delegate,
                        running,
                        output,
                    } => {
                        if *running {
                            if !ended_delegations.contains(id) && previous.as_ref() != Some(event) {
                                agents.say(
                                    &name,
                                    &format!("{name}: Coder handed the work to {delegate}"),
                                );
                            }
                            previous = Some(event.clone());
                            return None;
                        }
                        if ended_delegations.contains(id) {
                            return None;
                        }
                        ended_delegations.push(id.clone());
                        let delegated = Delegated::from_output(delegate, output);
                        let error = output
                            .get("error")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default();
                        if delegated.is_codex() && !error.is_empty() {
                            codex_refused = Some(error.to_owned());
                        }
                        let _ = journal(
                            Kind::Control,
                            &if error.is_empty() {
                                format!(
                                    "{delegate} finished its delegation ({} tokens)",
                                    delegated
                                        .total_tokens
                                        .or_else(|| delegated
                                            .input_tokens
                                            .zip(delegated.output_tokens)
                                            .map(|(i, o)| i + o))
                                        .map_or_else(|| "no".into(), |t| t.to_string())
                                )
                            } else {
                                format!("{delegate} stopped: {}", agent::plain(error))
                            },
                            None,
                        );
                        turned.delegated.push(delegated);
                        None
                    }
                    CoderEvent::Tool { .. } if previous.as_ref() == Some(event) => None,
                    CoderEvent::Tool {
                        name: tool,
                        input,
                        output,
                        running,
                        delegation,
                    } => {
                        let by = if delegation.is_some() && codex {
                            "Codex "
                        } else {
                            ""
                        };
                        previous = Some(event.clone());
                        if !tool.eq_ignore_ascii_case("run") {
                            if *running {
                                agents.say(&name, &format!("{name}: {by}Coder is using {tool}"));
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
                            agents.say(&name, &format!("{name}: {by}$ {command}"));
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
                                agents.say(&name, &format!("{name}: {by}exit {status}"));
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
                                    &format!(
                                        "{command} (by {their} policy: {})",
                                        rule.text(&their)
                                    ),
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
        // A Codex limit goes in the capacity book, as Coder books it, and
        // her later prompts in this request leave Codex out.
        if let Some(error) = codex_refused {
            let now = (self.agents.clock)();
            if let Some(refusal) = capacity::detect(capacity::Provider::Codex, &error, now) {
                let _ = capacity::record_with(&self.agents.tasks, refusal.clone(), login);
                self.codex = false;
                let line = codex_out(&name, Some(refusal.until));
                let _ = self.write(Kind::Control, &line, None);
                self.agents.say(&name, &line);
            }
        }
        turned.end = match ended {
            Ended::Finished { reply, tokens } => {
                // Zero is what Coder says when it counted nothing.
                turned.tokens = Some(tokens).filter(|t| *t > 0);
                TurnEnd::Finished(reply)
            }
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
        // Her budget is the owner's grant, checked against her records
        // before she spends anything.
        let meter = match Meter::open(store, record, clock()) {
            Ok(meter) => meter,
            Err(why) => {
                return fail(
                    "I couldn't read my budget or my spend records, so I didn't start.".into(),
                    &why,
                    "no budget",
                );
            }
        };
        if let Some(why) = &meter.unsealed {
            let _ = journal(Kind::Control, &format!("spend: {why}"), None);
        }
        if let Err(why) = meter.admit() {
            return fail(
                "I've used today's budget, so I didn't start.".into(),
                &why,
                "over budget",
            );
        }
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
        let codex = record.codes_on_codex() && self.codex_has_capacity(store, &name);
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
            meter,
            codex,
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
        let (request, today) = (hands.meter.request(), hands.meter.today());
        if request.records > 0 {
            let _ = journal(
                Kind::Control,
                &format!(
                    "spend: this request {}; today {}",
                    request.words(),
                    today.words()
                ),
                None,
            );
        }
        hands.close();
        steered.report
    }

    /// Whether Codex can take her coding now: the capacity book in the
    /// host's task store holds no Codex limit. When it does, she says so in
    /// one sentence and Coder works on its own model.
    pub(super) fn codex_has_capacity(&self, store: &Store, name: &str) -> bool {
        let now = (self.clock)();
        let book = capacity::Book::load_with(&self.tasks, login);
        let Some(refusal) = book.blocking(capacity::Provider::Codex, now) else {
            return true;
        };
        let line = codex_out(name, Some(refusal.until));
        let _ = store.append(&Entry::new(now, Kind::Control, &line));
        self.say(name, &line);
        false
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
