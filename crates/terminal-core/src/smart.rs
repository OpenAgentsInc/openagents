//! Request preview and the injected typed bridge to the shared chat client.

use super::layout::PaneId;
use crate::bridge::Message;
use crate::{bridge::Request, context::Context, proposals::Book};
use std::sync::mpsc::Receiver;

pub struct Draft {
    pub pane: PaneId,
    pub text: String,
    pub scroll: usize,
    pub context: Context,
}

pub struct Worker {
    pub pane: PaneId,
    pub request: Request,
    pub events: Receiver<Message>,
    pub eof: bool,
    process: Box<dyn crate::bridge::Process>,
}
impl Worker {
    pub fn start(
        transport: &dyn crate::pty::Transport,
        pane: PaneId,
        request: Request,
    ) -> Result<Self, String> {
        let connection = transport.request(&request)?;
        Ok(Self {
            pane,
            request,
            events: connection.events,
            process: connection.process,
            eof: false,
        })
    }
    pub fn ended(&mut self) -> Option<bool> {
        self.process.ended()
    }
}

/// Corrections offered for the command block `block` in `pane`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Correction {
    pub pane: PaneId,
    pub block: u64,
    pub choices: crate::correct::Choices,
    /// The choice the next request types.
    pub next: usize,
    /// The choice typed into the shell last, still on its prompt.
    pub typed: Option<String>,
}

#[derive(Default)]
pub struct Smart {
    pub draft: Option<Draft>,
    pub selected: Option<(PaneId, u64)>,
    pub git: Option<Receiver<(PaneId, String, String)>>,
    pub book: Book,
    pub policy: Policies,
    /// The read-only auto-run opt-in ([`crate::autorun`]); off unless the
    /// mount loads an admission.
    pub autorun: crate::autorun::AutoRun,
    pub pending: Option<(PaneId, String)>,
    pub workers: Vec<Worker>,
    pub threads: std::collections::BTreeMap<PaneId, String>,
    pub results: std::collections::BTreeMap<String, (String, String)>,
    pub execution: Option<(PaneId, String, String, u64)>,
    pub proposal_scroll: usize,
    pub enter_down: bool,
    /// A line put back at the prompt as a command: Enter runs it as typed.
    pub shell_override: Option<(PaneId, String)>,
    /// A command the shell did not find: Enter on the empty prompt asks.
    pub offer_ask: Option<(PaneId, String)>,
    /// Corrections for that command, which the person may type at the
    /// prompt ([`crate::correct`]).
    pub correction: Option<Correction>,
    offered: Option<(PaneId, u64)>,
    /// Requests handed to a workshop agent (`@alice ...`), waiting for the
    /// host's receipt.
    pub agent_asks: Vec<Receiver<Result<String, String>>>,
}

/// The key the terminal sends instead of Enter for a line routed to a
/// request; the zsh hook binds it to a widget that hands the line over.
pub const ASK_KEY: &[u8] = b"\x1b[24242~";

pub fn id() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    crate::proposals::digest(&(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |value| value.as_nanos()),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    ))[..32]
        .to_owned()
}

/// Scrubs common credential-bearing lines before their preview and submission.
pub fn scrub(text: &str) -> String {
    let mut private_key = false;
    let mut out = Vec::new();
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("-----begin") && lower.contains("private key") {
            private_key = true;
        }
        let sensitive = private_key
            || [
                "authorization:",
                "api_key",
                "api-key",
                "access_token",
                "password=",
                "secret=",
            ]
            .iter()
            .any(|needle| lower.contains(needle));
        if sensitive {
            out.push("[redacted]".into());
        } else {
            let words = line
                .split_inclusive(char::is_whitespace)
                .map(|word| {
                    let core = word
                        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-');
                    if [
                        "sk-",
                        "oak_",
                        "sess_",
                        "nsec1",
                        "ghp_",
                        "github_pat_",
                        "gho_",
                        "ghs_",
                    ]
                    .iter()
                    .any(|prefix| core.starts_with(prefix))
                    {
                        "[redacted] ".to_owned()
                    } else {
                        word.to_owned()
                    }
                })
                .collect::<String>();
            out.push(words);
        }
        if lower.contains("-----end") && lower.contains("private key") {
            private_key = false;
        }
    }
    out.join("\n")
}

#[derive(Default)]
pub struct Policies(pub std::collections::BTreeMap<String, crate::proposals::Effect>);
impl crate::proposals::Policy for Policies {
    fn effect(&self, command: &str) -> crate::proposals::Effect {
        self.0.get(command).cloned().unwrap_or_else(|| {
            crate::proposals::Effect::Denied("no host policy admitted this command".into())
        })
    }
}

impl super::Overlay {
    pub fn ask(&mut self, text: String) {
        let Some(pane_id) = self.focus_id() else {
            return;
        };
        let Some(pane) = self.panes.get(&pane_id) else {
            return;
        };
        let mut context = Context::default();
        if let Some(binding) = pane.session.binding(String::new()) {
            context.directory = Some(binding.cwd);
        }
        if let Some(block) = self
            .smart
            .selected
            .filter(|(selected_pane, _)| *selected_pane == pane_id)
            .and_then(|(_, id)| pane.session.blocks.get(id))
            .or_else(|| {
                pane.session
                    .blocks
                    .records
                    .back()
                    .filter(|block| block.status.is_some_and(|code| code != 0))
            })
        {
            context.attach(block, &scrub);
        }
        if let Some(directory) = &context.directory {
            self.smart.git = Some(self.sessions().0.git_summary(pane_id, directory.clone()));
        }
        self.smart.draft = Some(Draft {
            pane: pane_id,
            text,
            scroll: 0,
            context,
        });
    }

    pub fn smart_key(&mut self, key: &super::KeyIn) -> bool {
        use crate::input::KeyCode;
        let focus = self.focus_id();
        if let Some(draft) = &mut self.smart.draft {
            if Some(draft.pane) != focus {
                return false;
            }
            match key.code {
                KeyCode::Escape => self.smart.draft = None,
                KeyCode::ArrowUp => {
                    // The routing guessed wrong: run the line as a command instead.
                    let pane = draft.pane;
                    let text = std::mem::take(&mut draft.text);
                    self.smart.draft = None;
                    if !text.trim().is_empty() {
                        let bytes = self.panes.get(&pane).map(|p| p.session.vt.paste(&text));
                        if let Some(bytes) = bytes {
                            self.send_to(pane, &bytes);
                        }
                        self.smart.shell_override = Some((pane, text));
                        self.notice =
                            Some("Back at the prompt as a command; Enter runs it.".into());
                    }
                }
                KeyCode::Backspace => {
                    draft.text.pop();
                }
                KeyCode::KeyD if self.mods.control_key() => {
                    draft.context.clear();
                    self.smart.git = None;
                }
                KeyCode::PageDown => draft.scroll = draft.scroll.saturating_add(10),
                KeyCode::PageUp => draft.scroll = draft.scroll.saturating_sub(10),
                KeyCode::Enter | KeyCode::NumpadEnter => self.submit_draft(),
                _ => {
                    if let Some(text) = &key.text {
                        draft.text.extend(text.chars().filter(|c| !c.is_control()));
                        draft.text.truncate(draft.text.floor_char_boundary(8192));
                    }
                }
            }
            return true;
        }
        if let Some((pane_id, proposal_key)) = self.smart.pending.clone() {
            if self.focus_id() != Some(pane_id) {
                return false;
            }
            if key.code == KeyCode::Escape {
                self.smart.pending = None;
                return true;
            }
            if key.code == KeyCode::PageDown {
                self.smart.proposal_scroll = self.smart.proposal_scroll.saturating_add(10);
                return true;
            }
            if key.code == KeyCode::PageUp {
                self.smart.proposal_scroll = self.smart.proposal_scroll.saturating_sub(10);
                return true;
            }
            if matches!(key.code, KeyCode::Enter | KeyCode::NumpadEnter) {
                let entry = &self.smart.book.entries[&proposal_key];
                let Some(pane) = self.panes.get(&pane_id) else {
                    return true;
                };
                let Some(binding) = pane
                    .session
                    .binding(entry.proposal.binding.context_digest.clone())
                else {
                    return true;
                };
                if !pane.session.blocks.at_prompt
                    || pane
                        .session
                        .blocks
                        .buffer
                        .as_deref()
                        .is_some_and(|buffer| !buffer.is_empty())
                {
                    self.notice =
                        Some("Return to an empty shell prompt before approving a proposal.".into());
                    return true;
                }
                match self.smart.book.enter(
                    &proposal_key,
                    &binding,
                    "local-user",
                    &id(),
                    &self.smart.policy,
                ) {
                    Ok(crate::proposals::Approval::Warning(warning)) => self.notice = Some(warning),
                    Ok(crate::proposals::Approval::Input { identity, bytes }) => {
                        let after = pane
                            .session
                            .blocks
                            .records
                            .back()
                            .map_or(0, |block| block.id);
                        self.smart.execution = Some((pane_id, proposal_key, identity, after));
                        self.smart.pending = None;
                        self.send_to(pane_id, &bytes);
                    }
                    Err(why) => self.notice = Some(why.into()),
                }
                return true;
            }
            // A pending proposal is a modal preview, so unrelated keys cannot edit the shell silently.
            return true;
        }
        self.route_enter(key)
    }

    /// What Enter would do with the focused pane's prompt line, when the
    /// shell hook reports one: run it, or hand it to a request.
    #[must_use]
    pub fn routing(&self, pane_id: PaneId) -> Option<crate::route::Decision> {
        let pane = self.panes.get(&pane_id)?;
        let blocks = &pane.session.blocks;
        if !blocks.at_prompt || pane.session.vt.alternate_screen() {
            return None;
        }
        let buffer = blocks.buffer.as_deref()?;
        if buffer.trim().is_empty() {
            return None;
        }
        if self
            .smart
            .shell_override
            .as_ref()
            .is_some_and(|(pane, line)| *pane == pane_id && line == buffer)
        {
            return Some(crate::route::Decision {
                route: crate::route::Route::Shell,
                sure: true,
            });
        }
        let word = crate::route::Word::parse(blocks.word.as_deref().unwrap_or(""));
        Some(crate::route::classify(buffer, word))
    }

    /// Enter at a prompt: a request goes to the hook's ask widget; a command
    /// goes to the shell as typed.
    fn route_enter(&mut self, key: &super::KeyIn) -> bool {
        use crate::input::KeyCode;
        let Some(pane_id) = self.focus_id() else {
            return false;
        };
        let enter = matches!(key.code, KeyCode::Enter | KeyCode::NumpadEnter)
            && !self.mods.control_key()
            && !self.mods.alt_key()
            && !self.mods.shift_key();
        if !enter {
            if self.smart.offer_ask.is_some() && !is_modifier_key(key.code) {
                self.smart.offer_ask = None;
            }
            return false;
        }
        let at_empty_prompt = self.panes.get(&pane_id).is_some_and(|pane| {
            pane.session.blocks.at_prompt
                && !pane.session.vt.alternate_screen()
                && pane
                    .session
                    .blocks
                    .buffer
                    .as_deref()
                    .is_some_and(|buffer| buffer.trim().is_empty())
        });
        if at_empty_prompt
            && let Some((pane, line)) = self.smart.offer_ask.take()
            && pane == pane_id
        {
            self.ask(line);
            return true;
        }
        self.smart.offer_ask = None;
        match self.routing(pane_id) {
            Some(decision) if decision.route == crate::route::Route::Ask => {
                self.send_to(pane_id, ASK_KEY);
                true
            }
            Some(_) => {
                self.smart.shell_override = None;
                false
            }
            None => false,
        }
    }

    /// Offers a request once when the shell did not find a command.
    fn offer_after_missing_command(&mut self) {
        let Some(pane_id) = self.focus_id() else {
            return;
        };
        let Some(block) = self
            .panes
            .get(&pane_id)
            .and_then(|pane| pane.session.blocks.records.back())
            .filter(|block| block.status == Some(127))
        else {
            return;
        };
        if self.smart.offered == Some((pane_id, block.id)) {
            return;
        }
        self.smart.offered = Some((pane_id, block.id));
        self.smart.offer_ask = Some((pane_id, block.command.clone()));
        let (id, command) = (block.id, block.command.clone());
        let table = self
            .panes
            .get(&pane_id)
            .and_then(|pane| pane.session.blocks.table.as_deref())
            .map(crate::route::Table::parse)
            .unwrap_or_default();
        self.smart.correction =
            crate::correct::choices(&table, &command).map(|choices| Correction {
                pane: pane_id,
                block: id,
                choices,
                next: 0,
                typed: None,
            });
        self.notice = Some(match &self.smart.correction {
            Some(correction) => format!(
                "`{command}` is not a command here. Ctrl+B then t types `{}` without running it{}; Enter on the empty prompt asks OpenAgents.",
                correction.choices.lines[0],
                more(&correction.choices),
            ),
            None => format!(
                "`{command}` is not a command here. Press Enter on the empty prompt to ask OpenAgents instead."
            ),
        });
    }

    /// The next correction for the focused pane's missing command, cycling
    /// through the choices, or `None` when none is offered for it.
    pub fn next_correction(&mut self) -> Option<String> {
        let focus = self.focus_id()?;
        let latest = self
            .panes
            .get(&focus)?
            .session
            .blocks
            .records
            .back()
            .map(|block| block.id);
        let correction =
            self.smart.correction.as_mut().filter(|correction| {
                correction.pane == focus && Some(correction.block) == latest
            })?;
        let line = correction.choices.lines[correction.next].clone();
        correction.next = (correction.next + 1) % correction.choices.lines.len();
        Some(line)
    }

    /// Types the next correction at the focused shell's prompt without
    /// pressing Enter, replacing the one typed before.
    pub fn type_correction(&mut self) {
        let Some(pane_id) = self.focus_id() else {
            return;
        };
        let Some(pane) = self.panes.get(&pane_id) else {
            return;
        };
        if !pane.session.blocks.at_prompt || pane.session.vt.alternate_screen() {
            self.notice = Some("A correction types only at the shell prompt.".into());
            return;
        }
        let typed = self
            .smart
            .correction
            .as_ref()
            .and_then(|correction| correction.typed.clone());
        let buffer = pane.session.blocks.buffer.clone();
        if let Some(buffer) = &buffer
            && !buffer.is_empty()
            && Some(buffer) != typed.as_ref()
        {
            self.notice = Some("Clear the prompt first; a correction replaces only itself.".into());
            return;
        }
        let Some(line) = self.next_correction() else {
            self.notice = Some("No correction is offered for this pane.".into());
            return;
        };
        let mut bytes = Vec::new();
        if let Some(typed) = &typed {
            // Erase the choice typed before, one character at a time.
            bytes.extend(std::iter::repeat_n(0x7f, typed.chars().count()));
        }
        if let Some(pane) = self.panes.get(&pane_id) {
            bytes.extend(pane.session.vt.paste(&line));
        }
        self.send_to(pane_id, &bytes);
        if let Some(correction) = &mut self.smart.correction {
            correction.typed = Some(line.clone());
        }
        self.notice = Some(format!(
            "Typed `{line}`; nothing ran. Enter runs it as a new command."
        ));
    }

    fn submit_draft(&mut self) {
        if !self.smart.workers.is_empty()
            || self.smart.pending.is_some()
            || self.smart.execution.is_some()
        {
            self.notice =
                Some("Finish or dismiss the current request before sending another.".into());
            return;
        }
        let Some(draft) = self.smart.draft.take() else {
            return;
        };
        // `@alice ...` goes to that workshop agent through the host, not
        // to the chat; she reports in her own thread and at her desk.
        if let Some((agent, text)) = crate::route::agent_request(&draft.text) {
            let receiver =
                self.sessions()
                    .0
                    .ask_agent(agent, text, draft.context.directory.as_deref());
            self.smart.agent_asks.push(receiver);
            self.notice = Some(format!("Asked {agent} once; waiting for the host."));
            return;
        }
        let Some(pane) = self.panes.get(&draft.pane) else {
            return;
        };
        let Some(binding) = pane.session.binding(draft.context.identity()) else {
            self.notice =
                Some("The shell directory is unavailable; the request remains unsent.".into());
            self.smart.draft = Some(draft);
            return;
        };
        let thread = self.smart.threads.get(&draft.pane).cloned();
        let request = Request {
            thread: thread.clone().unwrap_or_else(id),
            request: id(),
            new: thread.is_none(),
            text: draft.text.clone(),
            context: draft.context.clone(),
            binding,
        };
        if let Err(why) = request.message() {
            self.notice = Some(why.into());
            self.smart.draft = Some(draft);
            return;
        }
        match Worker::start(self.sessions().0.as_ref(), draft.pane, request) {
            Ok(worker) => {
                self.smart.workers.push(worker);
                self.notice = Some("Request submitted once; waiting for the shared client.".into());
            }
            Err(why) => {
                self.notice = Some(why);
                self.smart.draft = Some(draft);
            }
        }
    }

    pub fn smart_tick(&mut self) {
        let mut answered = Vec::new();
        self.smart
            .agent_asks
            .retain(|receiver| match receiver.try_recv() {
                Ok(answer) => {
                    answered.push(answer);
                    false
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => true,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
            });
        for answer in answered {
            self.notice = Some(match answer {
                Ok(said) => said,
                Err(why) => format!("The agent request was not sent: {why}"),
            });
        }
        if let Some((pane_id, directory, summary)) = self
            .smart
            .git
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok())
            && let Some(draft) = &mut self.smart.draft
            && draft.pane == pane_id
            && draft.context.directory.as_deref() == Some(&directory)
        {
            draft.context.git = Some(summary);
        }
        let mut messages = Vec::new();
        for worker in &mut self.smart.workers {
            for _ in 0..32 {
                match worker.events.try_recv() {
                    Ok(message) => messages.push((worker.pane, worker.request.clone(), message)),
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        worker.eof = true;
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        let mut failed = 0;
        self.smart.workers.retain_mut(|worker| {
            let ended = worker.ended();
            if worker.eof && ended == Some(false) {
                failed += 1;
            }
            !(worker.eof && ended.is_some())
        });
        if failed > 0 && self.paper.on {
            self.paper_failed();
        }
        for (pane_id, request, message) in messages {
            match message {
                Message::Attached(thread) if thread == request.thread => {
                    if let Some((key, approval)) = self.smart.results.remove(&request.request) {
                        let _ = self.smart.book.acknowledge(&key, &approval);
                        continue;
                    }
                    // The thread stays with this pane; nothing opens beside it.
                    self.smart.threads.insert(pane_id, thread);
                }
                Message::Proposal(proposal, effect)
                    if proposal.thread == request.thread
                        && proposal.id == request.request
                        && proposal.binding == request.binding =>
                {
                    self.smart.policy.0.insert(proposal.command.clone(), effect);
                    let command = proposal.command.clone();
                    if let Ok(key) = self.smart.book.offer(proposal) {
                        self.smart.proposal_scroll = 0;
                        if self.paper.on {
                            self.paper_offered(&key, &command);
                        }
                        if self.auto_run(pane_id, &key) {
                            continue;
                        }
                        self.smart.pending = Some((pane_id, key));
                        self.notice =
                            Some("Pending command: Enter approves; Esc dismisses.".into());
                    }
                }
                message @ (Message::Answer(_) | Message::Door(_)) => {
                    if self.paper.on {
                        self.paper_message(&message);
                    }
                }
                _ => {}
            }
        }
        self.shell_result();
        let request = self.focus_id().and_then(|pane| {
            self.panes
                .get_mut(&pane)
                .and_then(|pane| pane.session.blocks.request.take())
        });
        if let Some(request) = request {
            self.ask(request);
        }
        self.offer_after_missing_command();
    }

    /// Runs a just-offered proposal without Enter when the auto-run
    /// opt-in admits it, and answers whether it did. The pane must sit at
    /// an empty prompt; anything [`Book::auto`] refuses stays pending.
    fn auto_run(&mut self, pane_id: PaneId, key: &str) -> bool {
        self.smart.autorun.reload();
        if self.smart.autorun.workspaces().is_empty() {
            return false;
        }
        let Some(entry) = self.smart.book.entries.get(key) else {
            return false;
        };
        let Some(pane) = self.panes.get(&pane_id) else {
            return false;
        };
        let blocks = &pane.session.blocks;
        if !blocks.at_prompt || blocks.buffer.as_deref().is_some_and(|b| !b.is_empty()) {
            return false;
        }
        let Some(binding) = pane
            .session
            .binding(entry.proposal.binding.context_digest.clone())
        else {
            return false;
        };
        let after = blocks.records.back().map_or(0, |block| block.id);
        let Ok(crate::proposals::Approval::Input { identity, bytes }) = self.smart.book.auto(
            key,
            &binding,
            &id(),
            &self.smart.policy,
            &self.smart.autorun,
        ) else {
            return false;
        };
        self.smart.execution = Some((pane_id, key.to_owned(), identity, after));
        self.notice =
            Some("Ran a read-only command: auto-run is on here (prefix R turns it off).".into());
        self.send_to(pane_id, &bytes);
        true
    }

    /// The prefix's `R`: turns read-only auto-run on for the focused
    /// pane's directory, or off when it is on there.
    pub fn toggle_autorun(&mut self) {
        let Some(cwd) = self
            .focus_id()
            .and_then(|pane| self.panes.get(&pane))
            .and_then(|pane| pane.session.binding(String::new()))
            .map(|binding| binding.cwd)
        else {
            self.notice = Some("Auto-run needs a shell pane with a known directory.".into());
            return;
        };
        self.smart.autorun.reload();
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_millis() as u64);
        self.notice = Some(match self.smart.autorun.revoke(&cwd) {
            Ok(true) => "Auto-run is off: every proposal waits for Enter.".into(),
            Ok(false) => match self.smart.autorun.admit(&cwd, "local-user", at) {
                Ok(()) => format!(
                    "Auto-run is on for {cwd}: exact read-only proposals run without Enter. Prefix R turns it off."
                ),
                Err(why) => why,
            },
            Err(why) => why,
        });
    }

    pub fn block_move(&mut self, previous: bool) {
        let Some(pane_id) = self.focus_id() else {
            return;
        };
        let Some(pane) = self.panes.get_mut(&pane_id) else {
            return;
        };
        let blocks = &pane.session.blocks.records;
        if blocks.is_empty() {
            return;
        }
        let current = self
            .smart
            .selected
            .filter(|(selected_pane, _)| *selected_pane == pane_id)
            .and_then(|(_, selected)| blocks.iter().position(|block| block.id == selected))
            .unwrap_or(blocks.len() - 1);
        let index = if previous {
            current.saturating_sub(1)
        } else {
            (current + 1).min(blocks.len() - 1)
        };
        let id = blocks[index].id;
        self.show_block(pane_id, id);
    }

    /// Selects block `id` in `pane_id` and scrolls the pane to its start.
    pub fn show_block(&mut self, pane_id: PaneId, id: u64) {
        let Some(pane) = self.panes.get_mut(&pane_id) else {
            return;
        };
        let Some(block) = pane.session.blocks.get(id) else {
            return;
        };
        self.smart.selected = Some((pane_id, block.id));
        let top = pane.session.vt.history_dropped() + pane.session.vt.scrollback_len() as u64;
        pane.scroll = top.saturating_sub(block.start.line) as usize;
        self.notice = Some(format!(
            "Block {} · exit {:?} · {} ms",
            block.id,
            block.status,
            block.elapsed_ms.unwrap_or_default()
        ));
        pane.render_revision = pane.render_revision.wrapping_add(1);
    }

    pub fn copy_block(&mut self) {
        let block = self
            .focus_id()
            .and_then(|pane| self.panes.get(&pane))
            .and_then(|pane| {
                self.smart
                    .selected
                    .filter(|(selected_pane, _)| Some(*selected_pane) == self.focus_id())
                    .and_then(|(_, id)| pane.session.blocks.get(id))
                    .or_else(|| pane.session.blocks.records.back())
            })
            .cloned();
        if let Some(block) = block {
            self.set_clipboard(format!("$ {}\n{}", block.command, block.output));
        }
    }

    pub fn collapse_block(&mut self) {
        let selected = self
            .smart
            .selected
            .filter(|(pane, _)| Some(*pane) == self.focus_id())
            .map(|(_, id)| id);
        if let Some(pane) = self.focused_pane()
            && let Some(id) =
                selected.or_else(|| pane.session.blocks.records.back().map(|block| block.id))
        {
            pane.session.blocks.collapse(id);
            pane.render_revision = pane.render_revision.wrapping_add(1);
        }
    }

    pub fn rerun_block(&mut self) {
        let block = self
            .focus_id()
            .and_then(|pane| self.panes.get(&pane))
            .and_then(|pane| {
                self.smart
                    .selected
                    .filter(|(selected_pane, _)| Some(*selected_pane) == self.focus_id())
                    .and_then(|(_, id)| pane.session.blocks.get(id))
                    .or_else(|| pane.session.blocks.records.back())
            })
            .cloned();
        if let Some(block) = block {
            self.paste(&block.command);
            self.notice = Some("Command copied to the shell. Enter runs a new block.".into());
        }
    }
}

impl super::Overlay {
    fn shell_result(&mut self) {
        let Some((pane_id, key, approval, after)) = self.smart.execution.clone() else {
            return;
        };
        let Some(pane) = self.panes.get(&pane_id) else {
            self.smart.book.recover();
            self.smart.execution = None;
            return;
        };
        let Some(block) = pane
            .session
            .blocks
            .records
            .iter()
            .find(|block| block.id > after && block.end.is_some())
            .cloned()
        else {
            return;
        };
        if let Err(why) = self.smart.book.complete(&key, &approval, block.clone()) {
            self.notice = Some(format!(
                "Command result is uncertain: {why}. It will not be replayed."
            ));
            self.smart.book.recover();
            self.smart.execution = None;
            return;
        }
        let mut context = Context::default();
        context.attach(&block, &scrub);
        let proposal = &self.smart.book.entries[&key].proposal;
        let mut binding = proposal.binding.clone();
        binding.context_digest = context.identity();
        let request = Request {
            thread: proposal.thread.clone(),
            request: approval[..32].to_owned(),
            new: false,
            // The request identity carries the approval; the visible turn
            // reads as the person's own words.
            text: format!(
                "I ran `{}` as you proposed; its output is attached. Tell me briefly what it shows.",
                proposal.command
            ),
            context,
            binding,
        };
        match Worker::start(self.sessions().0.as_ref(), pane_id, request.clone()) {
            Ok(worker) => {
                self.smart.results.insert(request.request, (key, approval));
                self.smart.workers.push(worker);
                self.smart.execution = None;
                self.notice = Some(
                    "Approved command completed; its block is returning to the same thread.".into(),
                );
            }
            Err(why) => {
                self.notice = Some(format!(
                    "Result delivery is pending: {why}. The command will not be repeated."
                ))
            }
        }
    }
}

fn is_modifier_key(code: crate::input::KeyCode) -> bool {
    use crate::input::KeyCode;
    matches!(
        code,
        KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::AltLeft
            | KeyCode::AltRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight
            | KeyCode::CapsLock
            | KeyCode::Fn
    )
}

/// How many more equally close names there are than shown, as a clause.
fn more(choices: &crate::correct::Choices) -> String {
    match choices.total {
        1 => String::new(),
        total => format!(" (again for the next of {total} equally close names)"),
    }
}
