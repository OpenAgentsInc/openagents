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

#[derive(Default)]
pub struct Smart {
    pub draft: Option<Draft>,
    pub selected: Option<(PaneId, u64)>,
    pub git: Option<Receiver<(PaneId, String, String)>>,
    pub book: Book,
    pub policy: Policies,
    pub pending: Option<(PaneId, String)>,
    pub workers: Vec<Worker>,
    pub threads: std::collections::BTreeMap<PaneId, String>,
    pub results: std::collections::BTreeMap<String, (String, String)>,
    pub execution: Option<(PaneId, String, String, u64)>,
    pub proposal_scroll: usize,
    pub enter_down: bool,
}

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
        false
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
        self.smart.workers.retain_mut(|worker| {
            let ended = worker.ended().is_some();
            !(worker.eof && ended)
        });
        for (pane_id, request, message) in messages {
            match message {
                Message::Attached(thread) if thread == request.thread => {
                    if let Some((key, approval)) = self.smart.results.remove(&request.request) {
                        let _ = self.smart.book.acknowledge(&key, &approval);
                        continue;
                    }
                    if self.smart.threads.insert(pane_id, thread.clone()).is_none()
                        && let Some(super::pty::Program::Command {
                            program,
                            mut args,
                            label,
                        }) = self.sessions().0.thread_program()
                    {
                        args.extend(["--thread".into(), thread, "--observe".into()]);
                        let active = self.active;
                        let focus = self.focus_id();
                        let Some(source_tab) = self
                            .tabs
                            .iter()
                            .position(|tab| tab.layout.panes().contains(&pane_id))
                        else {
                            continue;
                        };
                        self.active = source_tab;
                        self.tabs[source_tab].layout.set_focus(pane_id);
                        self.split(
                            super::layout::Axis::Columns,
                            &super::pty::Program::Command {
                                program,
                                args,
                                label,
                            },
                        );
                        self.active = active;
                        if let Some(focus) = focus
                            && let Some(tab) = self.tabs.get_mut(self.active)
                        {
                            tab.layout.set_focus(focus);
                        }
                    }
                }
                Message::Proposal(proposal, effect)
                    if proposal.thread == request.thread
                        && proposal.id == request.request
                        && proposal.binding == request.binding =>
                {
                    self.smart.policy.0.insert(proposal.command.clone(), effect);
                    if let Ok(key) = self.smart.book.offer(proposal) {
                        self.smart.proposal_scroll = 0;
                        self.smart.pending = Some((pane_id, key));
                        self.notice =
                            Some("Pending command: Enter approves; Esc dismisses.".into());
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
        let block = &blocks[index];
        self.smart.selected = Some((pane_id, block.id));
        let top = pane.session.vt.history_dropped() + pane.session.vt.scrollback_len() as u64;
        pane.scroll = top.saturating_sub(block.start.line) as usize;
        self.notice = Some(format!(
            "Block {} · exit {:?} · {} ms",
            block.id,
            block.status,
            block.elapsed_ms.unwrap_or_default()
        ));
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
            text: format!(
                "Result of approved shell proposal {} revision {} (approval {}). Continue this same thread from its attached output; do not repeat the command.",
                proposal.id, proposal.revision, approval
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
